import { useState, useMemo, useCallback, memo } from 'react';
import { Story, Task } from '../../types';
import { StatusColumn } from './StatusColumn';
import { Plus, MoreVertical, Sparkles, Loader2 } from 'lucide-react';
import { Button } from '../ui/Button';
import { load } from '@tauri-apps/plugin-store';
import { TaskFormModal, TaskFormData } from '../board/TaskFormModal';
import { StoryFormModal, StoryFormData } from '../board/StoryFormModal';
import { useScrum } from '../../context/ScrumContext';
import { v4 as uuidv4 } from 'uuid';
import { invoke } from '@tauri-apps/api/core';
import { useWorkspace } from '../../context/WorkspaceContext';

interface StorySwimlaneProps {
    story: Story;
    tasks: Task[];
}

const STATUSES: Task['status'][] = ['To Do', 'In Progress', 'Done'];

export const StorySwimlane = memo(function StorySwimlane({ story, tasks }: StorySwimlaneProps) {
    const { currentProjectId } = useWorkspace();
    const [isAddTaskModalOpen, setIsAddTaskModalOpen] = useState(false);
    const [isEditStoryModalOpen, setIsEditStoryModalOpen] = useState(false);
    const [isGenerating, setIsGenerating] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const { addTask, updateStory, deleteStory } = useScrum();

    const handleAddTask = useCallback(async (data: TaskFormData) => {
        // Map the TaskFormData status to the Task type status
        const statusMap: Record<TaskFormData['status'], Task['status']> = {
            'TODO': 'To Do',
            'IN_PROGRESS': 'In Progress',
            'DONE': 'Done'
        };

        await addTask({
            id: uuidv4(),
            story_id: story.id,
            title: data.title,
            description: data.description,
            status: statusMap[data.status],
            archived: false
        });
    }, [addTask, story.id]);

    const handleEditStory = useCallback(async (data: StoryFormData) => {
        await updateStory({
            ...story,
            title: data.title,
            description: data.description,
            acceptance_criteria: data.acceptance_criteria
        });
    }, [updateStory, story]);

    const handleDeleteStory = useCallback(async () => {
        await deleteStory(story.id);
    }, [deleteStory, story.id]);

    const handleGenerateTasks = useCallback(async () => {
        setIsGenerating(true);
        setError(null);
        try {
            const store = await load('settings.json');
            let provider = 'anthropic';
            const savedProvider = await store.get<{ value: string }>('default-ai-provider');
            if (savedProvider && typeof savedProvider === 'object' && 'value' in savedProvider) {
                provider = savedProvider.value;
            } else if (typeof savedProvider === 'string') {
                provider = savedProvider;
            }

            const generatedTasks = await invoke<{ title: string; description: string }[]>('generate_tasks_from_story', {
                title: story.title,
                description: story.description || '',
                acceptanceCriteria: story.acceptance_criteria || '',
                provider: provider,
                projectId: currentProjectId
            });

            for (const t of generatedTasks) {
                await addTask({
                    id: uuidv4(),
                    story_id: story.id,
                    title: t.title,
                    description: t.description,
                    status: 'To Do',
                    archived: false
                });
            }
        } catch (err: unknown) {
            console.error('Failed to generate tasks:', err);
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            // フォールバックとしてアラートも表示
            alert(`AI Task Generation Failed:\n${errorMessage}`);
        } finally {
            setIsGenerating(false);
        }
    }, [story, addTask]);

    const groupedTasks = useMemo(() => {
        const groups: Record<string, Task[]> = {
            'To Do': [],
            'In Progress': [],
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
                    <h2 className="text-lg font-semibold text-gray-900">{story.title}</h2>
                    {story.description && (
                        <p className="text-sm text-gray-500 mt-1">{story.description}</p>
                    )}
                </div>
                <div className="flex items-center gap-2">
                    <span className="inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-800">
                        {story.status === 'Ready' ? '準備完了' : story.status === 'In Progress' ? '進行中' : story.status === 'Done' ? '完了' : story.status}
                    </span>
                    <Button
                        size="sm"
                        variant="secondary"
                        onClick={handleGenerateTasks}
                        disabled={isGenerating}
                        className="bg-purple-50 text-purple-700 hover:bg-purple-100 hover:text-purple-800 border-purple-200"
                    >
                        {isGenerating ? (
                            <Loader2 size={16} className="mr-1 animate-spin" />
                        ) : (
                            <Sparkles size={16} className="mr-1" />
                        )}
                        {isGenerating ? 'AI生成中...' : 'AIで自動生成'}
                    </Button>
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

            {error && (
                <div className="px-4 py-2 bg-red-50 text-red-600 text-sm border-b border-red-100">
                    <span className="font-semibold">エラー:</span> {error}
                </div>
            )}

            {/* Task Columns */}
            <div className="p-4 bg-white">
                <div className="flex gap-4">
                    {STATUSES.map(status => (
                        <StatusColumn
                            key={`${story.id}-${status}`}
                            storyId={story.id}
                            status={status}
                            tasks={groupedTasks[status]}
                        />
                    ))}
                </div>
            </div>

            <TaskFormModal
                isOpen={isAddTaskModalOpen}
                onClose={() => setIsAddTaskModalOpen(false)}
                onSave={handleAddTask}
                title={`「${story.title}」にタスクを追加`}
            />

            <StoryFormModal
                isOpen={isEditStoryModalOpen}
                onClose={() => setIsEditStoryModalOpen(false)}
                onSave={handleEditStory}
                onDelete={handleDeleteStory}
                initialData={{
                    title: story.title,
                    description: story.description || '',
                    acceptance_criteria: story.acceptance_criteria || ''
                }}
                title="ストーリーを編集"
            />
        </div>
    );
});
