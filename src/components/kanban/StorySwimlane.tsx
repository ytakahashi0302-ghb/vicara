import { useState, useMemo, useCallback, memo } from 'react';
import { Story, Task } from '../../types';
import { StatusColumn } from './StatusColumn';
import { Plus, MoreVertical } from 'lucide-react';
import { Button } from '../ui/Button';
import { TaskFormModal, TaskFormData } from '../board/TaskFormModal';
import { StoryFormModal, StoryFormData } from '../board/StoryFormModal';
import { useScrum } from '../../context/ScrumContext';
import { v4 as uuidv4 } from 'uuid';
import { invoke } from '@tauri-apps/api/core';
import toast from 'react-hot-toast';

interface StorySwimlaneProps {
    story: Story;
    tasks: Task[];
}

const STATUSES: Task['status'][] = ['To Do', 'In Progress', 'Review', 'Done'];

export const StorySwimlane = memo(function StorySwimlane({ story, tasks }: StorySwimlaneProps) {
    const [isAddTaskModalOpen, setIsAddTaskModalOpen] = useState(false);
    const [isEditStoryModalOpen, setIsEditStoryModalOpen] = useState(false);
    const { refresh, updateStory, deleteStory, setTaskDependencies } = useScrum();

    const handleAddTask = useCallback(async (data: TaskFormData) => {
        const statusMap: Record<TaskFormData['status'], Task['status']> = {
            'TODO': 'To Do',
            'IN_PROGRESS': 'In Progress',
            'REVIEW': 'Review',
            'DONE': 'Done'
        };
        const newId = uuidv4();
        try {
            await invoke('add_task', {
                id: newId,
                projectId: story.project_id,
                storyId: story.id,
                title: data.title,
                description: data.description,
                status: statusMap[data.status],
                assigneeType: null,
                assignedRoleId: data.assigned_role_id || null,
                priority: data.priority ?? 3
            });
            await refresh();
        } catch (error) {
            console.error('Failed to add task with role assignment', error);
            toast.error(`タスクの作成に失敗しました: ${error}`);
            throw error;
        }
        if (data.blocked_by_task_ids.length > 0) {
            await setTaskDependencies(newId, data.blocked_by_task_ids);
        }
    }, [refresh, setTaskDependencies, story.id, story.project_id]);

    const handleEditStory = useCallback(async (data: StoryFormData) => {
        await updateStory({
            ...story,
            title: data.title,
            description: data.description,
            acceptance_criteria: data.acceptance_criteria,
            priority: data.priority ?? 3
        });
    }, [updateStory, story]);

    const handleDeleteStory = useCallback(async () => {
        await deleteStory(story.id);
    }, [deleteStory, story.id]);

    const groupedTasks = useMemo(() => {
        const groups: Record<string, Task[]> = {
            'To Do': [],
            'In Progress': [],
            'Review': [],
            'Done': []
        };
        for (const t of tasks) {
            if (groups[t.status]) {
                groups[t.status].push(t);
            }
        }
        return groups;
    }, [tasks]);

    return (
        <div className="bg-white border text-left border-gray-200 rounded-lg shadow-sm mb-6 overflow-hidden">
            {/* Story Header */}
            <div className="bg-gray-50 px-4 py-3 border-b border-gray-200 flex justify-between items-start group">
                <div className="flex-1 pr-4">
                    <div className="flex items-center gap-2">
                        <h2 className="text-lg font-semibold text-gray-900">{story.title}</h2>
                        <span className={`text-xs px-1.5 py-0.5 rounded border font-medium shrink-0 ${
                            (story.priority ?? 3) <= 1 ? 'bg-red-100 text-red-700 border-red-200' :
                            story.priority === 2 ? 'bg-orange-100 text-orange-700 border-orange-200' :
                            story.priority === 3 ? 'bg-yellow-100 text-yellow-700 border-yellow-200' :
                            story.priority === 4 ? 'bg-blue-100 text-blue-600 border-blue-200' :
                            'bg-gray-100 text-gray-500 border-gray-200'
                        }`}>P{story.priority ?? 3}</span>
                    </div>
                    {story.description && (
                        <p className="text-sm text-gray-500 mt-1">{story.description}</p>
                    )}
                </div>
                <div className="flex items-center gap-2">
                    <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800">
                        {story.status === 'Ready' ? '準備完了' : story.status === 'In Progress' ? '進行中' : story.status === 'Done' ? '完了' : story.status}
                    </span>
                    <Button size="sm" onClick={() => setIsAddTaskModalOpen(true)}>
                        <Plus size={16} className="mr-1" />
                        タスクを追加
                    </Button>
                    <button
                        onClick={() => setIsEditStoryModalOpen(true)}
                        className="p-1.5 text-gray-400 opacity-0 group-hover:opacity-100 hover:text-gray-700 hover:bg-gray-200 rounded transition-all"
                    >
                        <MoreVertical size={16} />
                    </button>
                </div>
            </div>

            {/* Task Columns */}
            <div className="p-4 bg-white">
                <div className="grid grid-cols-1 gap-4 md:grid-cols-2 2xl:grid-cols-4">
                    {STATUSES.map(status => (
                        <StatusColumn
                            key={`${story.id}-${status}`}
                            storyId={story.id}
                            status={status}
                            tasks={groupedTasks[status]}
                            allStoryTasks={tasks}
                        />
                    ))}
                </div>
            </div>

            <TaskFormModal
                isOpen={isAddTaskModalOpen}
                onClose={() => setIsAddTaskModalOpen(false)}
                onSave={handleAddTask}
                title={`「${story.title}」にタスクを追加`}
                availableTasks={tasks}
            />

            <StoryFormModal
                isOpen={isEditStoryModalOpen}
                onClose={() => setIsEditStoryModalOpen(false)}
                onSave={handleEditStory}
                onDelete={handleDeleteStory}
                initialData={{
                    title: story.title,
                    description: story.description || '',
                    acceptance_criteria: story.acceptance_criteria || '',
                    priority: story.priority ?? 3
                }}
                title="ストーリーを編集"
            />
        </div>
    );
});
