import { useState, useMemo, useCallback } from 'react';
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
import { useScrum } from '../../context/ScrumContext';
import { StorySwimlane } from './StorySwimlane';
import { Plus, Settings } from 'lucide-react';
import { Button } from '../ui/Button';
import { StoryFormModal, StoryFormData } from '../board/StoryFormModal';
import { SettingsModal } from '../SettingsModal';
import { v4 as uuidv4 } from 'uuid';

export function Board() {
    const { stories, tasks, updateTaskStatus, addStory, loading } = useScrum();
    const [isAddStoryModalOpen, setIsAddStoryModalOpen] = useState(false);
    const [isSettingsModalOpen, setIsSettingsModalOpen] = useState(false);

    const sensors = useSensors(
        useSensor(PointerSensor),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );

    const handleAddStory = useCallback(async (data: StoryFormData) => {
        await addStory({
            id: uuidv4(),
            title: data.title,
            description: data.description,
            acceptance_criteria: data.acceptance_criteria,
            status: 'Ready'
        });
    }, [addStory]);

    const handleDragEnd = useCallback((event: DragEndEvent) => {
        const { active, over } = event;

        // ドロップ領域が存在しない、または移動元と同じ場合は何もしない
        if (!over) return;

        // active.id はタスクの ID
        const activeTaskId = active.id as string;

        // over.id の形式によって処理を分ける
        // 1. Column の上にドロップされた場合: '{storyId}-{status}' 形式
        // 2. 他の TaskCard の上にドロップされた場合: Task の ID (現在はSortableContextでソートは考慮していないため簡易な処理)

        const activeTask = tasks.find(t => t.id === activeTaskId);
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
            // 楽観的UI更新（非同期待ちを排除してフリッカーを防止）
            updateTaskStatus(activeTaskId, targetStatus as 'To Do' | 'In Progress' | 'Done');
        }
    }, [tasks, updateTaskStatus]);

    const groupedTasks = useMemo(() => {
        const groups: Record<string, typeof tasks> = {};
        for (const t of tasks) {
            if (!groups[t.story_id]) groups[t.story_id] = [];
            groups[t.story_id].push(t);
        }
        return groups;
    }, [tasks]);

    if (loading) {
        return (
            <div className="flex items-center justify-center p-8 h-full min-h-[50vh]">
                <div className="text-gray-500">Loading board data...</div>
            </div>
        );
    }

    if (stories.length === 0) {
        return (
            <div className="p-6 bg-gray-100 min-h-screen flex flex-col">
                <div className="flex justify-end mb-4">
                    <Button variant="secondary" onClick={() => setIsSettingsModalOpen(true)}>
                        <Settings size={20} className="mr-2" />
                        Settings
                    </Button>
                </div>
                <div className="flex-1 flex flex-col items-center justify-center p-12 text-center bg-gray-50 rounded-lg border-2 border-dashed border-gray-300">
                    <h3 className="text-lg font-medium text-gray-900 mb-2">No Stories Yet</h3>
                    <p className="text-sm text-gray-500 max-w-sm mb-6">
                        Create a story to start organizing your work and see the Kanban board.
                    </p>
                    <Button onClick={() => setIsAddStoryModalOpen(true)}>
                        <Plus size={20} className="mr-2" />
                        Add Story
                    </Button>

                    <StoryFormModal
                        isOpen={isAddStoryModalOpen}
                        onClose={() => setIsAddStoryModalOpen(false)}
                        onSave={handleAddStory}
                        title="Add New Story"
                    />

                    <SettingsModal
                        isOpen={isSettingsModalOpen}
                        onClose={() => setIsSettingsModalOpen(false)}
                    />
                </div>
            </div>
        );
    }

    return (
        <div className="p-6 bg-gray-100 min-h-screen">
            <div className="mb-6 flex justify-between items-center">
                <h1 className="text-2xl font-bold text-gray-900">Sprint Board</h1>
                <div className="flex gap-2">
                    <Button variant="secondary" onClick={() => setIsSettingsModalOpen(true)}>
                        <Settings size={20} className="mr-2" />
                        Settings
                    </Button>
                    <Button onClick={() => setIsAddStoryModalOpen(true)}>
                        <Plus size={20} className="mr-2" />
                        Add Story
                    </Button>
                </div>
            </div>

            <DndContext
                sensors={sensors}
                collisionDetection={closestCorners}
                onDragEnd={handleDragEnd}
            >
                <div className="space-y-6">
                    {stories.map(story => (
                        <StorySwimlane
                            key={story.id}
                            story={story}
                            tasks={groupedTasks[story.id] || []}
                        />
                    ))}
                </div>
            </DndContext>

            <StoryFormModal
                isOpen={isAddStoryModalOpen}
                onClose={() => setIsAddStoryModalOpen(false)}
                onSave={handleAddStory}
                title="Add New Story"
            />

            <SettingsModal
                isOpen={isSettingsModalOpen}
                onClose={() => setIsSettingsModalOpen(false)}
            />
        </div>
    );
}
