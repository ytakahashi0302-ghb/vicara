import { useMemo, useCallback, useEffect, useState } from 'react';
import {
    DndContext,
    closestCorners,
    KeyboardSensor,
    PointerSensor,
    useSensor,
    useSensors,
    DragEndEvent,
} from '@dnd-kit/core';
import { sortableKeyboardCoordinates } from '@dnd-kit/sortable';
import { invoke } from '@tauri-apps/api/core';
import { useScrum } from '../../context/ScrumContext';
import { useWorkspace } from '../../context/WorkspaceContext';
import { TeamConfiguration, TeamRoleSetting } from '../../types';
import { StorySwimlane } from './StorySwimlane';
import toast from 'react-hot-toast';
import { VICARA_SETTINGS_UPDATED_EVENT } from '../../hooks/usePoAssistantAvatarImage';
import { useProjectLabels } from '../../hooks/useProjectLabels';
import { Button } from '../ui/Button';
import { Eye, Loader2, Square } from 'lucide-react';
import {
    detectPreviewPresetForProject,
    PreviewPreset,
    PROJECT_ROOT_PREVIEW_INVALIDATED_EVENT,
} from './projectPreview';

interface PreviewServerInfo {
    task_id: string;
    port: number;
    pid: number;
    worktree_path: string;
    command: string;
    url: string;
}

export function Board() {
    const { stories, tasks, sprints, updateTaskStatus, loading, isTaskBlocked, getTaskBlockers } = useScrum();
    const { projects, currentProjectId } = useWorkspace();
    const { formatSprintLabel } = useProjectLabels();
    const [teamRoles, setTeamRoles] = useState<TeamRoleSetting[]>([]);
    const [previewPreset, setPreviewPreset] = useState<PreviewPreset | null>(null);
    const [previewInfo, setPreviewInfo] = useState<PreviewServerInfo | null>(null);
    const [isPreviewLoading, setIsPreviewLoading] = useState(false);
    const [isStoppingPreview, setIsStoppingPreview] = useState(false);
    
    const activeSprint = useMemo(() => {
        return sprints.find(s => s.status === 'Active');
    }, [sprints]);

    const currentProject = useMemo(() => {
        return projects.find((project) => project.id === currentProjectId) ?? null;
    }, [projects, currentProjectId]);

    const projectPath = currentProject?.local_path ?? null;
    
    const activeStories = useMemo(() => {
        if (!activeSprint) return [];
        return stories.filter(s => s.sprint_id === activeSprint.id);
    }, [stories, activeSprint]);

    const activeTasks = useMemo(() => {
        if (!activeSprint) return [];
        // sprint_id が明示設定されているタスク OR アクティブスプリントのストーリーに属するタスク
        // (PO経由でスプリント中に追加されたタスクは story_id のみ持つ場合がある)
        const activeStoryIds = new Set(activeStories.map(s => s.id));
        return tasks.filter(t =>
            t.sprint_id === activeSprint.id || activeStoryIds.has(t.story_id)
        );
    }, [tasks, activeSprint, activeStories]);

    useEffect(() => {
        let cancelled = false;

        const loadTeamRoles = async () => {
            try {
                const config = await invoke<TeamConfiguration>('get_team_configuration');
                if (!cancelled) {
                    setTeamRoles(config.roles);
                }
            } catch (error) {
                console.error('Failed to load team roles for avatar resolution', error);
                if (!cancelled) {
                    setTeamRoles([]);
                }
            }
        };

        void loadTeamRoles();
        const handleSettingsUpdated = () => {
            void loadTeamRoles();
        };
        window.addEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);

        return () => {
            cancelled = true;
            window.removeEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
        };
    }, []);

    useEffect(() => {
        let cancelled = false;

        const loadPreviewPreset = async () => {
            if (!projectPath) {
                setPreviewPreset(null);
                return;
            }

            try {
                const preset = await detectPreviewPresetForProject(projectPath);
                if (!cancelled) {
                    setPreviewPreset(preset);
                }
            } catch (error) {
                console.error('Failed to detect root preview preset', error);
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
        let cancelled = false;

        const loadPreviewInfo = async () => {
            if (!currentProjectId) {
                setPreviewInfo(null);
                return;
            }

            try {
                const info = await invoke<PreviewServerInfo | null>('get_project_root_preview', {
                    projectId: currentProjectId,
                });
                if (!cancelled) {
                    setPreviewInfo(info);
                }
            } catch (error) {
                console.error('Failed to load root preview info', error);
                if (!cancelled) {
                    setPreviewInfo(null);
                }
            }
        };

        void loadPreviewInfo();

        return () => {
            cancelled = true;
        };
    }, [currentProjectId]);

    useEffect(() => {
        const handlePreviewInvalidated = (event: Event) => {
            const customEvent = event as CustomEvent<{ projectId?: string }>;
            if (customEvent.detail?.projectId === currentProjectId) {
                setPreviewInfo(null);
            }
        };

        window.addEventListener(PROJECT_ROOT_PREVIEW_INVALIDATED_EVENT, handlePreviewInvalidated);
        return () => {
            window.removeEventListener(PROJECT_ROOT_PREVIEW_INVALIDATED_EVENT, handlePreviewInvalidated);
        };
    }, [currentProjectId]);

    const roleLookup = useMemo<Record<string, TeamRoleSetting>>(
        () => Object.fromEntries(teamRoles.map((role) => [role.id, role])),
        [teamRoles],
    );

    const sensors = useSensors(
        useSensor(PointerSensor, {
            activationConstraint: {
                distance: 5,
            },
        }),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );


    const handleDragEnd = useCallback((event: DragEndEvent) => {
        const { active, over } = event;

        // ドロップ領域が存在しない、または移動元と同じ場合は何もしない
        if (!over) return;

        // active.id はタスクの ID
        const activeTaskId = active.id as string;

        // over.id の形式によって処理を分ける
        // 1. Column の上にドロップされた場合: '{storyId}-{status}' 形式
        // 2. 他の TaskCard の上にドロップされた場合: Task の ID (現在はSortableContextでソートは考慮していないため簡易な処理)

        const activeTask = activeTasks.find(t => t.id === activeTaskId);
        if (!activeTask) return;

        let targetStoryId = '';
        let targetStatus = '';

        if (over.data.current?.type === 'Column') {
            targetStoryId = over.data.current.storyId;
            targetStatus = over.data.current.status;
        } else if (over.data.current?.type === 'Task') {
            const overTask = over.data.current.task;
            targetStoryId = overTask.story_id;
            targetStatus = overTask.status;
        }

        // 制約A (同一Story内のみの移動を許可)
        if (targetStoryId && targetStoryId !== activeTask.story_id) {
            console.warn('Cannot move task between different stories (Constraint Plan A)');
            return;
        }

        // ステータスが変更された場合のみ更新
        if (targetStatus && targetStatus !== activeTask.status) {
            // ブロック中タスクを In Progress に移動する場合、警告を表示（ソフト制約）
            if (targetStatus === 'In Progress' && isTaskBlocked(activeTaskId)) {
                const blockers = getTaskBlockers(activeTaskId);
                const blockerTitles = blockers.map(b => b.title).join(', ');
                toast(`⚠️ このタスクは先行タスクが未完了です: ${blockerTitles}`, {
                    duration: 4000,
                    style: { background: '#fef3c7', color: '#92400e' }
                });
            }
            updateTaskStatus(activeTaskId, targetStatus as typeof activeTask.status);
        }
    }, [activeTasks, updateTaskStatus, isTaskBlocked, getTaskBlockers]);

    const groupedTasks = useMemo(() => {
        const groups: Record<string, typeof activeTasks> = {};
        for (const t of activeTasks) {
            if (!groups[t.story_id]) groups[t.story_id] = [];
            groups[t.story_id].push(t);
        }
        return groups;
    }, [activeTasks]);

    const isRootPreviewReady = Boolean(projectPath && previewPreset);
    const hasRunningRootPreview = Boolean(previewInfo);
    const rootPreviewSubtitle = useMemo(() => {
        if (isPreviewLoading) {
            return 'ブラウザを準備しています';
        }
        if (isStoppingPreview) {
            return '動作確認を停止しています';
        }
        if (!projectPath) {
            return 'ローカルパスを設定してください';
        }
        if (previewInfo) {
            return `起動中: ${previewInfo.url}`;
        }
        if (!previewPreset) {
            return '構成を判定できません';
        }
        return previewPreset.kind === 'static' ? 'index.html を直接開きます' : '開発サーバーを起動します';
    }, [isPreviewLoading, isStoppingPreview, previewInfo, previewPreset, projectPath]);

    const handleOpenRootPreview = useCallback(async () => {
        if (!projectPath) {
            toast.error('ワークスペースのローカルパスが未設定です。Settings から設定してください。');
            return;
        }

        if (!previewPreset) {
            toast.error('このプロジェクトは現在の簡易動作確認に未対応です。ARCHITECTURE.md と package.json を確認してください。');
            return;
        }

        setIsPreviewLoading(true);
        try {
            if (previewPreset.kind === 'static') {
                await invoke<string>('open_project_root_static_preview', {
                    projectPath,
                });
                setPreviewInfo(null);
                toast.success('ルートディレクトリの index.html を開きました。');
                return;
            }

            if (previewInfo) {
                const latestInfo = await invoke<PreviewServerInfo | null>('get_project_root_preview', {
                    projectId: currentProjectId,
                });
                if (latestInfo) {
                    setPreviewInfo(latestInfo);
                    await invoke('open_preview_in_browser', { url: latestInfo.url });
                    toast.success(`ルートディレクトリの動作確認を再表示しました (${latestInfo.url})`);
                    return;
                }

                setPreviewInfo(null);
            }

            const info = await invoke<PreviewServerInfo>('start_project_root_preview', {
                projectId: currentProjectId,
                projectPath,
                command: previewPreset.command,
            });
            setPreviewInfo(info);
            await invoke('open_preview_in_browser', { url: info.url });
            toast.success(`ルートディレクトリの動作確認を開きました (${info.url})`);
        } catch (error) {
            console.error('Failed to open root preview', error);
            toast.error(`ルートディレクトリの動作確認に失敗しました: ${error}`);
        } finally {
            setIsPreviewLoading(false);
        }
    }, [currentProjectId, previewInfo, previewPreset, projectPath]);

    const handleStopRootPreview = useCallback(async () => {
        if (!previewInfo) {
            return;
        }

        setIsStoppingPreview(true);
        try {
            await invoke('stop_project_root_preview', {
                projectId: currentProjectId,
            });
            setPreviewInfo(null);
            toast.success('ルートディレクトリの動作確認を停止しました。');
        } catch (error) {
            console.error('Failed to stop root preview', error);
            toast.error(`ルートディレクトリの動作確認停止に失敗しました: ${error}`);
        } finally {
            setIsStoppingPreview(false);
        }
    }, [currentProjectId, previewInfo]);

    if (loading) {
        return (
            <div className="flex items-center justify-center p-8 h-full min-h-[50vh]">
                <div className="text-gray-500">データを読み込み中...</div>
            </div>
        );
    }

    if (!activeSprint) {
        return (
            <div className="p-6 bg-gray-100 h-full flex flex-col">
                <div className="flex-1 flex flex-col items-center justify-center p-12 text-center bg-gray-50 rounded-lg border-2 border-dashed border-gray-300">
                    <h3 className="text-lg font-medium text-gray-900 mb-2">アクティブなスプリントがありません</h3>
                    <p className="text-sm text-gray-500 max-w-sm mb-6">
                        バックログ画面から次のスプリントを計画し、開始してください。
                    </p>
                </div>
            </div>
        );
    }
    
    return (
        <div className="p-6 bg-gray-100 h-full flex flex-col">
            <div className="mb-6 flex justify-between items-center">
                <div>
                    <h1 className="text-2xl font-bold text-gray-900">スプリントボード</h1>
                    <p className="mt-1 text-sm font-semibold text-slate-500">
                        {formatSprintLabel(activeSprint)}
                    </p>
                </div>
                <div className="flex items-center gap-2">
                    <Button
                        type="button"
                        size="md"
                        variant="ghost"
                        onClick={() => void handleOpenRootPreview()}
                        disabled={isPreviewLoading}
                        title={
                            !projectPath
                                ? 'ローカルパス未設定のため動作確認を開始できません'
                                : previewPreset
                                  ? hasRunningRootPreview
                                    ? '現在起動中の動作確認を再表示します'
                                    : '現在のプロジェクトルートで動作確認を開きます'
                                  : 'このプロジェクトは現在の簡易動作確認に未対応です'
                        }
                        className={`h-auto min-h-[56px] rounded-2xl border px-3.5 py-2.5 focus:ring-sky-400 ${
                            isRootPreviewReady
                                ? 'border-sky-200 bg-white text-slate-900 shadow-sm hover:-translate-y-0.5 hover:border-sky-300 hover:bg-sky-50/70 hover:shadow-md'
                                : 'border-slate-200 bg-slate-50/85 text-slate-600 shadow-sm hover:border-slate-300 hover:bg-slate-100'
                        }`}
                    >
                        <span
                            className={`mr-3 inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-xl ${
                                isRootPreviewReady
                                    ? 'bg-sky-100 text-sky-700'
                                    : 'bg-slate-200 text-slate-500'
                            }`}
                        >
                            {isPreviewLoading ? (
                                <Loader2 size={16} className="animate-spin" />
                            ) : (
                                <Eye size={16} />
                            )}
                        </span>
                        <span className="flex min-w-0 flex-col items-start text-left leading-tight">
                            <span className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400">
                                Project Root
                            </span>
                            <span className="mt-0.5 text-sm font-semibold text-current">
                                {hasRunningRootPreview ? '動作確認を再表示' : '動作確認'}
                            </span>
                            <span className="mt-0.5 max-w-[240px] truncate text-xs text-slate-500">
                                {rootPreviewSubtitle}
                            </span>
                        </span>
                    </Button>

                    {hasRunningRootPreview && (
                        <Button
                            type="button"
                            size="sm"
                            variant="ghost"
                            onClick={() => void handleStopRootPreview()}
                            disabled={isStoppingPreview}
                            title="現在起動中のルート動作確認を停止します"
                            className="h-11 rounded-2xl border border-rose-200 bg-white px-3 text-rose-700 shadow-sm hover:border-rose-300 hover:bg-rose-50 hover:text-rose-800 focus:ring-rose-300"
                        >
                            {isStoppingPreview ? (
                                <Loader2 size={15} className="mr-1.5 animate-spin" />
                            ) : (
                                <Square size={15} className="mr-1.5" />
                            )}
                            停止
                        </Button>
                    )}
                </div>
            </div>

            {activeStories.length === 0 ? (
                <div className="flex-1 flex flex-col items-center justify-center p-12 text-center bg-gray-50 rounded-lg border-2 border-dashed border-gray-300">
                    <h3 className="text-lg font-medium text-gray-900 mb-2">タスクがありません</h3>
                    <p className="text-sm text-gray-500 max-w-sm mb-6">
                        このスプリントにはタスクが割り当てられていません。バックログから追加してください。
                    </p>
                </div>
            ) : (
                <DndContext
                    sensors={sensors}
                    collisionDetection={closestCorners}
                    onDragEnd={handleDragEnd}
                >
                    <div className="space-y-6">
                        {activeStories.map(story => (
                            <StorySwimlane
                                key={story.id}
                                story={story}
                                tasks={groupedTasks[story.id] || []}
                                roleLookup={roleLookup}
                            />
                        ))}
                    </div>
                </DndContext>
            )}
        </div>
    );
}
