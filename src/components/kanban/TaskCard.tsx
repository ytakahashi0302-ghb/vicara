import { useState, memo, useCallback, useMemo } from 'react';
import { useSortable } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Task } from '../../types';
import { MoreVertical, TerminalSquare, Lock } from 'lucide-react';
import { TaskFormModal, TaskFormData } from '../board/TaskFormModal';
import { useScrum } from '../../context/ScrumContext';
import { useWorkspace } from '../../context/WorkspaceContext';
import { invoke } from '@tauri-apps/api/core';
import toast from 'react-hot-toast';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

interface TaskCardProps {
    task: Task;
    availableTasks?: Task[];
}

type TaskWithRoleAssignment = Task & {
    assigned_role_id?: string | null;
};

function getPriorityBadgeClass(priority: number): string {
    if (priority <= 1) return 'bg-red-100 text-red-700 border-red-200';
    if (priority === 2) return 'bg-orange-100 text-orange-700 border-orange-200';
    if (priority === 3) return 'bg-yellow-100 text-yellow-700 border-yellow-200';
    if (priority === 4) return 'bg-blue-100 text-blue-600 border-blue-200';
    return 'bg-gray-100 text-gray-500 border-gray-200';
}

export const TaskCard = memo(function TaskCard({ task, availableTasks = [] }: TaskCardProps) {
    const { updateTaskStatus, refresh, deleteTask, setTaskDependencies, isTaskBlocked, getTaskBlockers, getBlockerIds } = useScrum();
    const { projects, currentProjectId } = useWorkspace();
    const [isEditModalOpen, setIsEditModalOpen] = useState(false);
    const blocked = isTaskBlocked(task.id);
    const blockers = getTaskBlockers(task.id);
    const blockerIds = getBlockerIds(task.id);
    const assignedRoleId = (task as TaskWithRoleAssignment).assigned_role_id ?? '';
    const isLaunchDisabled = task.status === 'In Progress' || task.status === 'Done';
    const {
        attributes,
        listeners,
        setNodeRef,
        transform,
        transition,
        isDragging
    } = useSortable({
        id: task.id,
        data: {
            type: 'Task',
            task
        }
    });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
    };

    const handleLaunchClaude = async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (isLaunchDisabled) return;
        const currentProject = projects.find(p => p.id === currentProjectId);
        if (!currentProject?.local_path) {
            toast.error("ワークスペースのローカルパスが設定されていません。Settingsから設定してください。");
            return;
        }
        if (!assignedRoleId) {
            toast.error("Claude 実行前に担当ロールを設定してください。");
            return;
        }

        try {
            await invoke('execute_claude_task', {
                taskId: task.id,
                cwd: currentProject.local_path
            });
            await updateTaskStatus(task.id, 'In Progress');
            toast.success("Claudeでの開発を開始しました (ターミナルをご確認ください)");
        } catch (err: any) {
            toast.error(`プロセス起動失敗: ${err}`);
            window.dispatchEvent(new CustomEvent('claude_error', { detail: String(err) }));
        }
    };

    return (
        <div
            ref={setNodeRef}
            style={style}
            {...attributes}
            {...listeners}
            onClick={() => setIsEditModalOpen(true)}
            className={`bg-white p-3 rounded-md shadow-sm border cursor-grab active:cursor-grabbing ${
                isDragging ? 'border-blue-500 opacity-50'
                : blocked ? 'border-gray-200 hover:border-gray-300 opacity-60'
                : 'border-gray-200 hover:border-blue-300'
            } flex flex-col gap-1 mb-2 group relative transition-colors`}
        >
            <div className="flex-1 min-w-0 pr-6">
                <div className="flex items-center gap-1.5 mb-1">
                    <span className={`text-xs px-1.5 py-0.5 rounded border font-medium ${getPriorityBadgeClass(task.priority)}`}>
                        P{task.priority}
                    </span>
                    {blocked && (
                        <span
                            className="flex items-center gap-0.5 text-xs text-amber-600 bg-amber-50 border border-amber-200 px-1.5 py-0.5 rounded"
                            title={`ブロック中: ${blockers.map(b => b.title).join(', ')}`}
                        >
                            <Lock size={10} />
                            Blocked
                        </span>
                    )}
                </div>
                <h4 className="text-sm font-medium text-gray-900 truncate" title={task.title}>{task.title}</h4>
                {task.description && (
                    <div
                        className="text-xs text-gray-500 mt-1 prose prose-sm prose-slate max-w-none prose-p:leading-snug prose-li:my-0 max-h-64 overflow-hidden relative"
                        title="Click to edit and see full description"
                    >
                        <ReactMarkdown remarkPlugins={[remarkGfm]}>
                            {task.description}
                        </ReactMarkdown>
                        {/* Optional: Add a faded bottom edge to indicate truncation if it gets too long */}
                        <div className="absolute bottom-0 left-0 right-0 h-6 bg-gradient-to-t from-white to-transparent pointer-events-none" />
                    </div>
                )}
            </div>

            <div className="absolute top-2 right-2 flex gap-1 opacity-100 sm:opacity-0 sm:group-hover:opacity-100 transition-all z-10 bg-white/80 rounded backdrop-blur-sm p-0.5 shadow-sm">
                <button
                    onClick={handleLaunchClaude}
                    disabled={isLaunchDisabled}
                    className="p-1 text-blue-500 hover:text-white hover:bg-blue-500 rounded transition-colors disabled:text-gray-300 disabled:hover:bg-transparent disabled:hover:text-gray-300 disabled:cursor-not-allowed"
                    title={
                        task.status === 'In Progress'
                            ? '進行中のタスクは再実行できません'
                            : task.status === 'Done'
                                ? '完了済みタスクは再実行できません'
                                : '開発を実行 (Launch Claude)'
                    }
                >
                    <TerminalSquare size={16} />
                </button>
                <button
                    onClick={(e) => { e.stopPropagation(); setIsEditModalOpen(true); }}
                    className="p-1 text-gray-400 hover:text-gray-700 hover:bg-gray-100 rounded transition-colors"
                >
                    <MoreVertical size={16} />
                </button>
            </div>

            <TaskFormModal
                isOpen={isEditModalOpen}
                onClose={() => setIsEditModalOpen(false)}
                onSave={useCallback(async (data) => {
                    const statusMap: Record<TaskFormData['status'], Task['status']> = {
                        'TODO': 'To Do',
                        'IN_PROGRESS': 'In Progress',
                        'DONE': 'Done'
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
                }, [task, refresh, setTaskDependencies])}
                onDelete={useCallback(async () => {
                    await deleteTask(task.id);
                }, [task.id, deleteTask])}
                initialData={useMemo(() => ({
                    title: task.title,
                    description: task.description || '',
                    status: Object.entries({
                        'TODO': 'To Do',
                        'IN_PROGRESS': 'In Progress',
                        'DONE': 'Done'
                    }).find(([_, v]) => v === task.status)?.[0] as TaskFormData['status'] || 'TODO',
                    priority: task.priority ?? 3,
                    assigned_role_id: assignedRoleId,
                    blocked_by_task_ids: blockerIds,
                }), [task.title, task.description, task.status, task.priority, assignedRoleId, blockerIds])}
                title="タスクを編集"
                availableTasks={availableTasks.filter(t => t.id !== task.id)}
            />
        </div>
    );
});
