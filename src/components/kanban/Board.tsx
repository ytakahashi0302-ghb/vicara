import { useMemo, useCallback } from 'react';
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
import toast from 'react-hot-toast';

export function Board() {
    const { stories, tasks, sprints, updateTaskStatus, loading, isTaskBlocked, getTaskBlockers } = useScrum();
    
    const activeSprint = useMemo(() => {
        return sprints.find(s => s.status === 'Active');
    }, [sprints]);
    
    const activeStories = useMemo(() => {
        if (!activeSprint) return [];
        return stories.filter(s => s.sprint_id === activeSprint.id);
    }, [stories, activeSprint]);

    const activeTasks = useMemo(() => {
        if (!activeSprint) return [];
        return tasks.filter(t => t.sprint_id === activeSprint.id);
    }, [tasks, activeSprint]);

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
    
    if (activeStories.length === 0) {
        return (
            <div className="p-6 bg-gray-100 h-full flex flex-col">
                <div className="flex-1 flex flex-col items-center justify-center p-12 text-center bg-gray-50 rounded-lg border-2 border-dashed border-gray-300">
                    <h3 className="text-lg font-medium text-gray-900 mb-2">タスクがありません</h3>
                    <p className="text-sm text-gray-500 max-w-sm mb-6">
                        このスプリントにはタスクが割り当てられていません。バックログから追加してください。
                    </p>
                </div>
            </div>
        );
    }

    return (
        <div className="p-6 bg-gray-100 h-full">
            <div className="mb-6 flex justify-between items-center">
                <h1 className="text-2xl font-bold text-gray-900">スプリントボード</h1>
            </div>

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
                        />
                    ))}
                </div>
            </DndContext>
        </div>
    );
}
