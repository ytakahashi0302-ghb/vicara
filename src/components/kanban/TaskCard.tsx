import { useState, memo, useCallback, useMemo, useEffect } from 'react';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Task, TeamRoleSetting, WorktreeRecord } from '../../types';
import {
    AlertTriangle,
    ExternalLink,
    Eye,
    GitMerge,
    Loader2,
    Lock,
    MoreVertical,
    RotateCcw,
    Square,
    TerminalSquare,
    Trash2,
} from 'lucide-react';
import { TaskFormModal, TaskFormData } from '../board/TaskFormModal';
import { useScrum } from '../../context/ScrumContext';
import { useWorkspace } from '../../context/WorkspaceContext';
import { useSprintTimer } from '../../context/SprintTimerContext';
import { invoke } from '@tauri-apps/api/core';
import { confirm } from '@tauri-apps/plugin-dialog';
import toast from 'react-hot-toast';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Button } from '../ui/Button';
import { Modal } from '../ui/Modal';
import { Avatar } from '../ai/Avatar';
import { resolveAvatarForRoleName } from '../ai/avatarRegistry';

interface TaskCardProps {
    task: Task;
    availableTasks?: Task[];
    roleLookup: Record<string, TeamRoleSetting>;
}

interface PreviewServerInfo {
    task_id: string;
    port: number;
    pid: number;
    worktree_path: string;
    command: string;
    url: string;
}

type PreviewPreset = {
    kind: 'command' | 'static';
    command?: 'npm run dev' | 'npm run serve';
    label: string;
};

type MergeResult =
    | { type: 'success' }
    | { type: 'conflict'; conflicting_files: string[] }
    | { type: 'error'; message: string };

function getPriorityBadgeClass(priority: number): string {
    if (priority <= 1) return 'bg-red-100 text-red-700 border-red-200';
    if (priority === 2) return 'bg-orange-100 text-orange-700 border-orange-200';
    if (priority === 3) return 'bg-yellow-100 text-yellow-700 border-yellow-200';
    if (priority === 4) return 'bg-blue-100 text-blue-600 border-blue-200';
    return 'bg-gray-100 text-gray-500 border-gray-200';
}

function stopInteractiveEvent(event: React.SyntheticEvent) {
    event.stopPropagation();
}

function readPackageScripts(packageJsonContent: string | null): { dev: boolean; serve: boolean } {
    if (!packageJsonContent) {
        return { dev: false, serve: false };
    }

    try {
        const parsed = JSON.parse(packageJsonContent) as { scripts?: Record<string, unknown> };
        return {
            dev: typeof parsed.scripts?.dev === 'string',
            serve: typeof parsed.scripts?.serve === 'string',
        };
    } catch (error) {
        console.error('Failed to parse package.json for preview detection', error);
        return { dev: false, serve: false };
    }
}

function resolvePreviewPreset(
    architectureContent: string | null,
    packageJsonContent: string | null,
    hasIndexHtml: boolean,
): PreviewPreset | null {
    if (!architectureContent) {
        return null;
    }

    const content = architectureContent.toLowerCase();
    const scripts = readPackageScripts(packageJsonContent);

    const staticKeywords = [
        'vanilla js',
        'vanilla javascript',
        'plain javascript',
        'static site',
        'static html',
        'html/css/javascript',
        'html, css, javascript',
        '静的サイト',
        '静的 html',
        'バニラjs',
        'バニラ javascript',
        'vanilla',
    ];

    const devKeywords = [
        'react',
        'vite',
        'vue',
        'svelte',
        'astro',
        'next.js',
        'nextjs',
        'nuxt',
        'frontend framework',
    ];

    if (devKeywords.some((keyword) => content.includes(keyword)) && scripts.dev) {
        return {
            kind: 'command',
            command: 'npm run dev',
            label: '開発サーバープレビュー',
        };
    }

    if (staticKeywords.some((keyword) => content.includes(keyword))) {
        if (scripts.serve) {
            return {
                kind: 'command',
                command: 'npm run serve',
                label: '静的サイト向けプレビュー',
            };
        }
        if (scripts.dev) {
            return {
                kind: 'command',
                command: 'npm run dev',
                label: '開発サーバープレビュー',
            };
        }
        if (hasIndexHtml) {
            return {
                kind: 'static',
                label: '静的ファイルプレビュー',
            };
        }
    }

    return null;
}

export const TaskCard = memo(function TaskCard({ task, availableTasks = [], roleLookup }: TaskCardProps) {
    const {
        updateTaskStatus,
        refresh,
        deleteTask,
        setTaskDependencies,
        isTaskBlocked,
        getTaskBlockers,
        getBlockerIds,
    } = useScrum();
    const { ensureTimerRunning } = useSprintTimer();
    const { projects, currentProjectId } = useWorkspace();
    const [isEditModalOpen, setIsEditModalOpen] = useState(false);
    const [previewInfo, setPreviewInfo] = useState<PreviewServerInfo | null>(null);
    const [previewPreset, setPreviewPreset] = useState<PreviewPreset | null>(null);
    const [hasConflict, setHasConflict] = useState(false);
    const [conflictFiles, setConflictFiles] = useState<string[]>([]);
    const [isConflictModalOpen, setIsConflictModalOpen] = useState(false);
    const [isPreviewLoading, setIsPreviewLoading] = useState(false);
    const [isStoppingPreview, setIsStoppingPreview] = useState(false);
    const [isMerging, setIsMerging] = useState(false);
    const [isRerunning, setIsRerunning] = useState(false);
    const [isDiscarding, setIsDiscarding] = useState(false);

    const blocked = isTaskBlocked(task.id);
    const blockers = getTaskBlockers(task.id);
    const blockerIds = getBlockerIds(task.id);
    const assignedRoleId = task.assigned_role_id ?? '';
    const isReviewTask = task.status === 'Review';
    const isLaunchDisabled =
        task.status === 'In Progress' || task.status === 'Done' || task.status === 'Review';

    const currentProject = useMemo(
        () => projects.find((project) => project.id === currentProjectId),
        [projects, currentProjectId],
    );
    const assignedRole = assignedRoleId ? roleLookup[assignedRoleId] : undefined;
    const assignedRoleName = assignedRole?.name?.trim() || '';
    const assignedAvatar = assignedRoleName ? resolveAvatarForRoleName(assignedRoleName) : null;
    const assignedAvatarImage = assignedRole?.avatar_image ?? null;
    const projectPath = currentProject?.local_path ?? null;

    const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
        id: task.id,
        data: {
            type: 'Task',
            task,
        },
    });

    useEffect(() => {
        let cancelled = false;

        const loadPreviewPreset = async () => {
            if (!projectPath) {
                setPreviewPreset(null);
                return;
            }

            try {
                const [architecture, packageJson, indexHtml] = await Promise.all([
                    invoke<string | null>('read_inception_file', {
                        localPath: projectPath,
                        filename: 'ARCHITECTURE.md',
                    }),
                    invoke<string | null>('read_inception_file', {
                        localPath: projectPath,
                        filename: 'package.json',
                    }),
                    invoke<string | null>('read_inception_file', {
                        localPath: projectPath,
                        filename: 'index.html',
                    }),
                ]);
                if (cancelled) return;
                setPreviewPreset(
                    resolvePreviewPreset(architecture, packageJson, Boolean(indexHtml)),
                );
            } catch (error) {
                console.error('Failed to load architecture file for preview detection', error);
                if (!cancelled) {
                    setPreviewPreset(null);
                }
            }
        };

        void loadPreviewPreset();

        return () => {
            cancelled = true;
        };
    }, [projectPath]);

    useEffect(() => {
        if (!isReviewTask) {
            setPreviewInfo(null);
            setHasConflict(false);
            setConflictFiles([]);
            setIsConflictModalOpen(false);
            return;
        }

        let cancelled = false;

        const loadWorktreeRecord = async () => {
            try {
                const record = await invoke<WorktreeRecord | null>('get_worktree_record', {
                    taskId: task.id,
                });
                if (cancelled) return;

                if (record?.preview_port) {
                    setPreviewInfo({
                        task_id: task.id,
                        port: record.preview_port,
                        pid: record.preview_pid ?? 0,
                        worktree_path: record.worktree_path,
                        command: 'npm run dev',
                        url: `http://127.0.0.1:${record.preview_port}`,
                    });
                } else {
                    setPreviewInfo(null);
                }

                setHasConflict(record?.status === 'conflict');
            } catch (error) {
                console.error('Failed to load worktree record', error);
            }
        };

        void loadWorktreeRecord();

        return () => {
            cancelled = true;
        };
    }, [isReviewTask, task.id]);

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
    };

    const handleLaunchClaude = useCallback(
        async (e: React.MouseEvent) => {
            e.stopPropagation();
            if (isLaunchDisabled) return;
            if (!projectPath) {
                toast.error('ワークスペースのローカルパスが設定されていません。Settingsから設定してください。');
                return;
            }
            if (!assignedRoleId) {
                toast.error('Claude 実行前に担当ロールを設定してください。');
                return;
            }

            try {
                await invoke('execute_claude_task', {
                    taskId: task.id,
                    cwd: projectPath,
                });
                try {
                    await ensureTimerRunning('AI_TASK_LAUNCHED', task.sprint_id ?? null);
                } catch (timerError) {
                    console.error('Failed to auto-start sprint timer after Claude launch', timerError);
                    toast.error('Claude は起動しましたが、タイマーの自動開始に失敗しました。必要に応じて手動で開始してください。');
                }
                await updateTaskStatus(task.id, 'In Progress');
                toast.success('Claudeでの開発を開始しました (ターミナルをご確認ください)');
            } catch (err) {
                toast.error(`プロセス起動失敗: ${err}`);
                window.dispatchEvent(new CustomEvent('claude_error', { detail: String(err) }));
            }
        },
        [assignedRoleId, ensureTimerRunning, isLaunchDisabled, projectPath, task.id, task.sprint_id, updateTaskStatus],
    );

    const handleStartPreview = useCallback(async () => {
        if (!projectPath) {
            toast.error('ローカルパスが未設定のためプレビューを起動できません。');
            return;
        }
        if (!previewPreset) {
            toast.error(
                'この技術スタックは現在の簡易プレビューに未対応です。対応可否は ARCHITECTURE.md の判定に依存します。',
            );
            return;
        }

        setIsPreviewLoading(true);
        try {
            if (previewPreset.kind === 'static') {
                await invoke<string>('open_static_preview', {
                    projectPath,
                    taskId: task.id,
                });
                setHasConflict(false);
                toast.success('ワークツリー内の index.html を開きました。');
                return;
            }

            const info = await invoke<PreviewServerInfo>('start_preview_server', {
                projectPath,
                taskId: task.id,
                command: previewPreset.command,
            });
            setPreviewInfo(info);
            setHasConflict(false);
            await invoke('open_preview_in_browser', { port: info.port });
            toast.success(`プレビューを起動しました (Port ${info.port})`);
        } catch (error) {
            console.error('Failed to start preview server', error);
            toast.error(`プレビュー起動に失敗しました: ${error}`);
        } finally {
            setIsPreviewLoading(false);
        }
    }, [previewPreset, projectPath, task.id]);

    const handleStopPreview = useCallback(async () => {
        setIsStoppingPreview(true);
        try {
            await invoke('stop_preview_server', { taskId: task.id });
            setPreviewInfo(null);
            toast.success('プレビューを停止しました');
        } catch (error) {
            console.error('Failed to stop preview server', error);
            toast.error(`プレビュー停止に失敗しました: ${error}`);
        } finally {
            setIsStoppingPreview(false);
        }
    }, [task.id]);

    const handleOpenPreview = useCallback(async () => {
        if (!previewInfo) return;
        try {
            await invoke('open_preview_in_browser', { port: previewInfo.port });
        } catch (error) {
            console.error('Failed to open preview', error);
            toast.error(`ブラウザ起動に失敗しました: ${error}`);
        }
    }, [previewInfo]);

    const handleMerge = useCallback(async () => {
        if (!projectPath) {
            toast.error('ローカルパスが未設定のためマージできません。');
            return;
        }
        const confirmed = await confirm(
            'このタスクの変更を main にマージします。よろしいですか？',
            {
                title: 'マージ確認',
                kind: 'warning',
            },
        );
        if (!confirmed) {
            return;
        }

        setIsMerging(true);
        try {
            const result = await invoke<MergeResult>('merge_worktree', {
                projectPath,
                taskId: task.id,
            });

            if (result.type === 'success') {
                setPreviewInfo(null);
                setHasConflict(false);
                setConflictFiles([]);
                await refresh();
                toast.success('マージが完了しました。タスクを Done に更新しました。');
                return;
            }

            if (result.type === 'conflict') {
                setHasConflict(true);
                setConflictFiles(result.conflicting_files);
                setIsConflictModalOpen(true);
                await refresh();
                toast.error('マージ時に競合が発生しました。対応方法を選択してください。');
                return;
            }

            toast.error(`マージに失敗しました: ${result.message}`);
        } catch (error) {
            console.error('Failed to merge worktree', error);
            toast.error(`マージに失敗しました: ${error}`);
        } finally {
            setIsMerging(false);
        }
    }, [projectPath, refresh, task.id]);

    const handleRerunWithConflictContext = useCallback(async () => {
        if (!projectPath) {
            toast.error('ローカルパスが未設定のため AI 再実行ができません。');
            return;
        }
        if (!assignedRoleId) {
            toast.error('Claude 実行前に担当ロールを設定してください。');
            return;
        }

        setIsRerunning(true);
        try {
            const additionalContext =
                conflictFiles.length > 0
                    ? `前回のマージで競合が発生しました。競合ファイル: ${conflictFiles.join(
                          ', ',
                      )}。既存の変更を尊重しつつ、競合を解消できるように実装・調整してください。`
                    : '前回のマージで競合が発生しました。既存の変更を尊重しつつ、競合を解消できるように実装・調整してください。';

            await invoke('execute_claude_task', {
                taskId: task.id,
                cwd: projectPath,
                additionalContext,
            });
            try {
                await ensureTimerRunning('AI_TASK_LAUNCHED', task.sprint_id ?? null);
            } catch (timerError) {
                console.error('Failed to auto-start sprint timer after Claude rerun', timerError);
                toast.error('AI は再実行しましたが、タイマーの自動開始に失敗しました。必要に応じて手動で開始してください。');
            }
            await updateTaskStatus(task.id, 'In Progress');
            setHasConflict(false);
            setIsConflictModalOpen(false);
            toast.success('競合情報を添えて AI を再実行しました。');
        } catch (error) {
            console.error('Failed to rerun claude task', error);
            toast.error(`AI 再実行に失敗しました: ${error}`);
        } finally {
            setIsRerunning(false);
        }
    }, [assignedRoleId, conflictFiles, ensureTimerRunning, projectPath, task.id, task.sprint_id, updateTaskStatus]);

    const handleDiscardWorktree = useCallback(async () => {
        if (!projectPath) {
            toast.error('ローカルパスが未設定のためワークツリーを破棄できません。');
            return;
        }
        if (!window.confirm('worktree を破棄して変更を取り消します。よろしいですか？')) {
            return;
        }

        setIsDiscarding(true);
        try {
            await invoke('remove_worktree', {
                projectPath,
                taskId: task.id,
            });
            await updateTaskStatus(task.id, 'To Do');
            setPreviewInfo(null);
            setHasConflict(false);
            setConflictFiles([]);
            setIsConflictModalOpen(false);
            await refresh();
            toast.success('ワークツリーを破棄し、タスクを To Do に戻しました。');
        } catch (error) {
            console.error('Failed to discard worktree', error);
            toast.error(`ワークツリーの破棄に失敗しました: ${error}`);
        } finally {
            setIsDiscarding(false);
        }
    }, [projectPath, refresh, task.id, updateTaskStatus]);

    const handleManualResolve = useCallback(() => {
        setIsConflictModalOpen(false);
        toast('Terminal Dock から worktree を開いて手動で競合を解消してください。', {
            duration: 5000,
            style: { background: '#fef2f2', color: '#991b1b' },
        });
    }, []);

    const handleSaveTask = useCallback(
        async (data: TaskFormData) => {
            const statusMap: Record<TaskFormData['status'], Task['status']> = {
                TODO: 'To Do',
                IN_PROGRESS: 'In Progress',
                REVIEW: 'Review',
                DONE: 'Done',
            };

            await invoke('update_task', {
                id: task.id,
                title: data.title,
                description: data.description,
                status: statusMap[data.status],
                assigneeType: task.assignee_type ?? null,
                assignedRoleId: data.assigned_role_id || null,
                priority: data.priority,
            });
            await refresh();
            await setTaskDependencies(task.id, data.blocked_by_task_ids);
        },
        [refresh, setTaskDependencies, task],
    );

    const handleDeleteTask = useCallback(async () => {
        await deleteTask(task.id);
    }, [deleteTask, task.id]);

    const initialTaskFormData = useMemo(
        () => ({
            title: task.title,
            description: task.description || '',
            status:
                (Object.entries({
                    TODO: 'To Do',
                    IN_PROGRESS: 'In Progress',
                    REVIEW: 'Review',
                    DONE: 'Done',
                }).find(([_, value]) => value === task.status)?.[0] as TaskFormData['status']) ||
                'TODO',
            priority: task.priority ?? 3,
            assigned_role_id: assignedRoleId,
            blocked_by_task_ids: blockerIds,
        }),
        [assignedRoleId, blockerIds, task.description, task.priority, task.status, task.title],
    );

    return (
        <>
            <div
                ref={setNodeRef}
                style={style}
                {...attributes}
                {...listeners}
                onClick={() => setIsEditModalOpen(true)}
                className={`group relative mb-2 flex cursor-grab flex-col gap-2 rounded-md border p-3 shadow-sm transition-colors active:cursor-grabbing ${
                    isDragging
                        ? 'border-blue-500 opacity-50'
                        : isReviewTask
                          ? 'border-amber-300 bg-amber-50/40 hover:border-amber-400'
                          : blocked
                            ? 'border-gray-200 opacity-60 hover:border-gray-300'
                            : 'border-gray-200 bg-white hover:border-blue-300'
                }`}
            >
                <div className="min-w-0 flex-1 pr-6">
                    <div className="mb-1 flex items-center gap-1.5">
                        <span
                            className={`rounded border px-1.5 py-0.5 text-xs font-medium ${getPriorityBadgeClass(
                                task.priority,
                            )}`}
                        >
                            P{task.priority}
                        </span>
                        {blocked && (
                            <span
                                className="flex items-center gap-0.5 rounded border border-amber-200 bg-amber-50 px-1.5 py-0.5 text-xs text-amber-600"
                                title={`ブロック中: ${blockers.map((blocker) => blocker.title).join(', ')}`}
                            >
                                <Lock size={10} />
                                Blocked
                            </span>
                        )}
                        {isReviewTask && (
                            <span className="rounded border border-amber-200 bg-amber-100 px-1.5 py-0.5 text-xs font-medium text-amber-800">
                                Review
                            </span>
                        )}
                    </div>
                    <h4 className="truncate text-sm font-medium text-gray-900" title={task.title}>
                        {task.title}
                    </h4>
                    {assignedRoleName && assignedAvatar && (
                        <div className="mt-1.5 flex items-center gap-2">
                            <Avatar kind={assignedAvatar.kind} size="xs" imageSrc={assignedAvatarImage} />
                            <span className="truncate text-xs font-medium text-slate-600" title={assignedRoleName}>
                                {assignedRoleName}
                            </span>
                        </div>
                    )}
                    {task.description && (
                        <div
                            className="relative mt-1 max-h-64 overflow-hidden prose prose-sm max-w-none prose-li:my-0 prose-p:leading-snug prose-slate text-xs text-gray-500"
                            title="Click to edit and see full description"
                        >
                            <ReactMarkdown remarkPlugins={[remarkGfm]}>{task.description}</ReactMarkdown>
                            <div className="pointer-events-none absolute bottom-0 left-0 right-0 h-6 bg-gradient-to-t from-white to-transparent" />
                        </div>
                    )}
                </div>

                {isReviewTask && (
                    <div
                        className="mt-1 space-y-2 rounded-lg border border-amber-200 bg-white/85 p-2"
                        onClick={stopInteractiveEvent}
                        onPointerDown={stopInteractiveEvent}
                    >
                        {previewInfo && (
                            <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-emerald-200 bg-emerald-50 px-2 py-2 text-xs text-emerald-800">
                                <span className="font-medium">プレビュー中: {previewInfo.url}</span>
                                <div className="flex gap-2">
                                    <Button
                                        type="button"
                                        size="sm"
                                        variant="ghost"
                                        onClick={() => void handleOpenPreview()}
                                    >
                                        <ExternalLink size={14} className="mr-1" />
                                        開く
                                    </Button>
                                    <Button
                                        type="button"
                                        size="sm"
                                        variant="ghost"
                                        disabled={isStoppingPreview}
                                        onClick={() => void handleStopPreview()}
                                    >
                                        {isStoppingPreview ? (
                                            <Loader2 size={14} className="mr-1 animate-spin" />
                                        ) : (
                                            <Square size={14} className="mr-1" />
                                        )}
                                        停止
                                    </Button>
                                </div>
                            </div>
                        )}

                        {!previewInfo && (
                            <div className="rounded-md border border-amber-100 bg-amber-50/70 px-2 py-2 text-xs text-amber-900">
                                {previewPreset ? (
                                    previewPreset.kind === 'command' ? (
                                        <span>
                                            使用コマンド:{' '}
                                            <code className="font-mono">{previewPreset.command}</code>
                                        </span>
                                    ) : (
                                        <span>
                                            静的サイトのため、ワークツリー内の
                                            <code className="mx-1 font-mono">index.html</code>
                                            を直接開きます。
                                        </span>
                                    )
                                ) : (
                                    <span>
                                        この技術スタックは簡易プレビュー未対応です。現在は
                                        <code className="mx-1 font-mono">npm run dev</code>
                                        と
                                        <code className="mx-1 font-mono">npm run serve</code>
                                        のみ対応しています。
                                    </span>
                                )}
                            </div>
                        )}

                        {hasConflict && (
                            <button
                                type="button"
                                className="flex w-full items-center justify-between rounded-md border border-red-200 bg-red-50 px-2 py-2 text-left text-xs text-red-700 transition-colors hover:bg-red-100"
                                onClick={() => setIsConflictModalOpen(true)}
                            >
                                <span className="flex items-center gap-1 font-medium">
                                    <AlertTriangle size={14} />
                                    コンフリクトあり
                                </span>
                                <span>解決オプションを表示</span>
                            </button>
                        )}

                        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                            <Button
                                type="button"
                                size="sm"
                                variant="secondary"
                                disabled={isPreviewLoading || isMerging || !previewPreset}
                                onClick={() => void handleStartPreview()}
                            >
                                {isPreviewLoading ? (
                                    <Loader2 size={14} className="mr-1 animate-spin" />
                                ) : (
                                    <Eye size={14} className="mr-1" />
                                )}
                                {previewPreset?.kind === 'static'
                                    ? 'index.html を開く'
                                    : previewInfo
                                      ? 'プレビュー再起動'
                                      : 'プレビュー起動'}
                            </Button>
                            <Button
                                type="button"
                                size="sm"
                                variant="primary"
                                disabled={isPreviewLoading || isMerging}
                                onClick={() => void handleMerge()}
                            >
                                {isMerging ? (
                                    <Loader2 size={14} className="mr-1 animate-spin" />
                                ) : (
                                    <GitMerge size={14} className="mr-1" />
                                )}
                                承認してマージ
                            </Button>
                        </div>
                    </div>
                )}

                <div className="absolute right-2 top-2 z-10 flex gap-1 rounded bg-white/80 p-0.5 opacity-100 shadow-sm backdrop-blur-sm transition-all sm:opacity-0 sm:group-hover:opacity-100">
                    <button
                        onClick={handleLaunchClaude}
                        onPointerDown={stopInteractiveEvent}
                        disabled={isLaunchDisabled}
                        className="rounded p-1 text-blue-500 transition-colors hover:bg-blue-500 hover:text-white disabled:cursor-not-allowed disabled:text-gray-300 disabled:hover:bg-transparent disabled:hover:text-gray-300"
                        title={
                            task.status === 'In Progress'
                                ? '進行中のタスクは再実行できません'
                                : task.status === 'Done'
                                  ? '完了済みタスクは再実行できません'
                                  : task.status === 'Review'
                                    ? 'Review 中のタスクは専用アクションから操作してください'
                                    : '開発を実行 (Launch Claude)'
                        }
                    >
                        <TerminalSquare size={16} />
                    </button>
                    <button
                        onClick={(event) => {
                            stopInteractiveEvent(event);
                            setIsEditModalOpen(true);
                        }}
                        onPointerDown={stopInteractiveEvent}
                        className="rounded p-1 text-gray-400 transition-colors hover:bg-gray-100 hover:text-gray-700"
                    >
                        <MoreVertical size={16} />
                    </button>
                </div>
            </div>

            <TaskFormModal
                isOpen={isEditModalOpen}
                onClose={() => setIsEditModalOpen(false)}
                onSave={handleSaveTask}
                onDelete={handleDeleteTask}
                initialData={initialTaskFormData}
                title="タスクを編集"
                availableTasks={availableTasks.filter((availableTask) => availableTask.id !== task.id)}
            />

            <Modal
                isOpen={isConflictModalOpen}
                onClose={() => setIsConflictModalOpen(false)}
                title="マージ競合が発生しました"
                width="lg"
            >
                <div className="space-y-4">
                    <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-sm text-red-800">
                        `main` へのマージ中に競合が発生しました。対応方法を選択してください。
                    </div>

                    <div>
                        <h3 className="mb-2 text-sm font-semibold text-gray-900">競合ファイル</h3>
                        {conflictFiles.length > 0 ? (
                            <div className="max-h-40 overflow-y-auto rounded-lg border border-gray-200 bg-gray-50 p-3 text-sm text-gray-700">
                                <ul className="space-y-1">
                                    {conflictFiles.map((file) => (
                                        <li key={file} className="font-mono text-xs">
                                            {file}
                                        </li>
                                    ))}
                                </ul>
                            </div>
                        ) : (
                            <p className="text-sm text-gray-500">
                                競合ファイル一覧は直前のマージ結果から取得できませんでした。必要に応じて Terminal Dock で確認してください。
                            </p>
                        )}
                    </div>

                    <div className="grid grid-cols-1 gap-2">
                        <Button
                            type="button"
                            variant="secondary"
                            onClick={handleManualResolve}
                            disabled={isRerunning || isDiscarding}
                        >
                            <TerminalSquare size={15} className="mr-2" />
                            手動解決する
                        </Button>
                        <Button
                            type="button"
                            variant="primary"
                            onClick={() => void handleRerunWithConflictContext()}
                            disabled={isRerunning || isDiscarding}
                        >
                            {isRerunning ? (
                                <Loader2 size={15} className="mr-2 animate-spin" />
                            ) : (
                                <RotateCcw size={15} className="mr-2" />
                            )}
                            AIで再実行する
                        </Button>
                        <Button
                            type="button"
                            variant="danger"
                            onClick={() => void handleDiscardWorktree()}
                            disabled={isRerunning || isDiscarding}
                        >
                            {isDiscarding ? (
                                <Loader2 size={15} className="mr-2 animate-spin" />
                            ) : (
                                <Trash2 size={15} className="mr-2" />
                            )}
                            ワークツリーを破棄する
                        </Button>
                    </div>
                </div>
            </Modal>
        </>
    );
});
