import { useState, useMemo } from 'react';
import { useScrum } from '../../context/ScrumContext';
import { Button } from '../ui/Button';
import { Lightbulb, Plus, CalendarPlus, Play, ArrowRight, ArrowLeft } from 'lucide-react';
import { StoryFormModal, StoryFormData } from '../board/StoryFormModal';
import { IdeaRefinementDrawer } from '../ai/IdeaRefinementDrawer';
import { v4 as uuidv4 } from 'uuid';
import { Story, Task } from '../../types';
import toast from 'react-hot-toast';

export function BacklogView() {
    const { stories, tasks, sprints, addStory, updateStory, deleteStory, createPlannedSprint, startSprint, assignStoryToSprint } = useScrum();
    const [isAddStoryModalOpen, setIsAddStoryModalOpen] = useState(false);
    const [isIdeaRefinementModalOpen, setIsIdeaRefinementModalOpen] = useState(false);
    const [storyFormInitialData, setStoryFormInitialData] = useState<Partial<StoryFormData> | undefined>();
    const [editingStory, setEditingStory] = useState<Story | null>(null);

    const backlogStories = useMemo(() => stories.filter(s => !s.sprint_id), [stories]);
    
    const plannedSprint = useMemo(() => sprints.find(s => s.status === 'Planned'), [sprints]);
    const activeSprint = useMemo(() => sprints.find(s => s.status === 'Active'), [sprints]);
    
    const plannedStories = useMemo(() => {
        if (!plannedSprint) return [];
        return stories.filter(s => s.sprint_id === plannedSprint.id);
    }, [stories, plannedSprint]);
    
    const plannedTasks = useMemo(() => {
        if (!plannedSprint) return [];
        return tasks.filter(t => t.sprint_id === plannedSprint.id);
    }, [tasks, plannedSprint]);

    // Handlers
    const handleAddStory = async (data: StoryFormData) => {
        await addStory({
            id: uuidv4(),
            title: data.title,
            description: data.description,
            acceptance_criteria: data.acceptance_criteria,
            status: 'Ready',
            archived: false
        });
    };

    const handleCreateSprint = async () => {
        if (plannedSprint) return;
        try {
            await createPlannedSprint();
        } catch (e) {
            console.error(e);
        }
    };

    const handleStartSprint = async () => {
        if (!plannedSprint) return;
        if (activeSprint) {
            toast.error('既にアクティブなスプリントが存在します。先にそちらを完了してください。');
            return;
        }

        if (plannedStories.length === 0 && plannedTasks.length === 0) {
            toast.error('タスクが追加されていません。先にバックログから追加してください。');
            return;
        }
        
        // Example: duration placeholder, normally set by timer or settings
        const durationMs = 7 * 24 * 60 * 60 * 1000;
        try {
            await startSprint(plannedSprint.id, durationMs);
            toast.success('スプリントを開始しました！Boardタブに移動してください。');
        } catch (e) {
            console.error(e);
        }
    };

    // Drag and Drop implementation has been removed due to WebView limitations. 
    // Button-based assignment is used exclusively.

    const renderStoryItem = (story: Story, assignedTasks: Task[], isPlanned: boolean) => {
        const totalTasks = assignedTasks.length;
        const doneTasks = assignedTasks.filter(t => t.status === 'Done').length;
        const progressText = totalTasks > 0 ? `(${doneTasks}/${totalTasks} 完了)` : '';

        return (
            <div 
                key={story.id} 
                onClick={() => {
                    setEditingStory(story);
                    setStoryFormInitialData({
                        title: story.title,
                        description: story.description || '',
                        acceptance_criteria: story.acceptance_criteria || ''
                    });
                    setIsAddStoryModalOpen(true);
                }}
                className="bg-white p-3 rounded-md shadow-sm border border-gray-200 mb-3 cursor-pointer hover:border-blue-400 hover:shadow-md transition-all group relative"
            >
                <div className="flex justify-between items-start">
                    <div className="font-semibold text-gray-800 break-words pr-2 flex items-center gap-2">
                        {story.title}
                        {totalTasks > 0 && (
                            <span className="text-xs font-normal text-gray-500 bg-gray-100 px-1.5 py-0.5 rounded">
                                {progressText}
                            </span>
                        )}
                    </div>
                    {plannedSprint && (
                        <button
                            onClick={(e) => {
                                e.stopPropagation();
                                assignStoryToSprint(story.id, isPlanned ? null : plannedSprint.id);
                            }}
                            className="opacity-0 group-hover:opacity-100 p-2 text-gray-400 hover:text-blue-600 transition-opacity bg-gray-50 hover:bg-blue-50 rounded shrink-0 mb-1"
                            title={isPlanned ? "バックログに戻す" : "スプリントに追加"}
                        >
                            {isPlanned ? <ArrowLeft size={20} /> : <ArrowRight size={20} />}
                        </button>
                    )}
                </div>
                <div className="text-xs text-gray-500 mt-1">{totalTasks} 個のタスク</div>
                {assignedTasks.length > 0 && (
                    <div className="mt-3 space-y-2 border-l-2 border-gray-100 pl-3">
                        {assignedTasks.map(t => (
                            <div 
                                key={t.id} 
                                className={`bg-gray-50 p-2 text-sm rounded border border-gray-100 cursor-default transition-colors flex justify-between items-center group/task ${t.status === 'Done' || t.archived ? 'opacity-50 grayscale hover:bg-gray-50 hover:border-gray-100' : 'hover:bg-gray-100 hover:border-blue-300'}`}
                            >
                                <div className={`flex items-center ${t.status === 'Done' || t.archived ? 'line-through text-gray-500' : ''}`}>
                                    <span className={`inline-block w-2 h-2 rounded-full mr-2 ${t.status === 'Done' || t.archived ? 'bg-green-400' : 'bg-blue-300'}`}></span>
                                    {t.title}
                                </div>
                            </div>
                        ))}
                    </div>
                )}
            </div>
        );
    };

    return (
        <div className="flex h-full gap-6 px-6 py-4 overflow-hidden">
            {/* Left: Backlog */}
            <div 
                className="flex-[1.2] flex flex-col bg-gray-50 rounded-lg border border-gray-200 shadow-inner overflow-hidden"
            >
                <div className="flex justify-between items-center p-4 border-b border-gray-200 bg-white">
                    <h2 className="text-base font-bold text-gray-800 flex items-center">
                        プロダクトバックログ
                        <span className="ml-2 bg-gray-100 text-gray-600 px-2 py-0.5 rounded-full text-xs font-medium">
                            {backlogStories.length} {backlogStories.length > 0 ? `stories` : ''}
                        </span>
                    </h2>
                    <div className="flex gap-2">
                        <Button
                            variant="secondary"
                            size="sm"
                            onClick={() => setIsIdeaRefinementModalOpen(true)}
                            className="bg-yellow-50 hover:bg-yellow-100 text-yellow-800 border-yellow-200"
                        >
                            <Lightbulb size={16} className="sm:mr-1" />
                            <span className="hidden sm:inline">アイデア</span>
                        </Button>
                        <Button size="sm" onClick={() => {
                            setStoryFormInitialData(undefined);
                            setIsAddStoryModalOpen(true);
                        }}>
                            <Plus size={16} className="sm:mr-1" />
                            <span className="hidden sm:inline">追加</span>
                        </Button>
                    </div>
                </div>
                
                <div 
                    className="overflow-y-auto flex-1 p-4 min-h-[300px] h-full"
                >
                    {backlogStories.length === 0 && tasks.filter(t => !t.sprint_id && !t.archived).length === 0 ? (
                        <div className="flex flex-col items-center justify-center h-full text-gray-400 text-sm p-4 text-center pointer-events-none">
                            <Lightbulb className="w-8 h-8 opacity-20 mb-2" />
                            <p>バックログは空です。<br/>ストーリーを作成して、プロジェクトを計画しましょう。</p>
                        </div>
                    ) : (
                        backlogStories.map(story => {
                            const tasksForStory = tasks.filter(t => t.story_id === story.id);
                            return renderStoryItem(story, tasksForStory, false);
                        })
                    )}
                </div>
            </div>

            {/* Right: Planned Sprint */}
            <div 
                className="flex-1 flex flex-col bg-blue-50/30 rounded-lg border border-blue-200 shadow-sm overflow-hidden min-h-[300px]"
            >
                <div className="flex justify-between items-center p-4 border-b border-blue-100 bg-white">
                    <h2 className="text-base font-bold text-blue-800 flex items-center">
                        {plannedSprint ? '次のスプリント (計画中)' : 'スプリント計画'}
                    </h2>
                    {!plannedSprint ? (
                        <Button size="sm" onClick={handleCreateSprint} variant="primary">
                            <CalendarPlus size={16} className="sm:mr-1" />
                            <span className="hidden sm:inline">スプリントを作成</span>
                        </Button>
                    ) : (
                        <Button size="sm" onClick={handleStartSprint} className="bg-green-600 hover:bg-green-700">
                            <Play size={16} className="sm:mr-1" />
                            <span className="hidden sm:inline">スプリントを開始</span>
                        </Button>
                    )}
                </div>

                <div 
                    className={ `overflow-y-auto flex-1 p-4 min-h-[300px] h-full ${!plannedSprint ? 'opacity-50 pointer-events-none' : ''}` }
                >
                    {!plannedSprint ? (
                        <div className="flex flex-col items-center justify-center h-full text-gray-400 text-sm p-4 text-center pointer-events-none">
                            <CalendarPlus className="w-8 h-8 opacity-20 mb-2 text-blue-400" />
                            <p>スプリントを作成すると、<br/>バックログからストーリーを割り当てられます。</p>
                        </div>
                    ) : plannedStories.length === 0 && tasks.filter(t => t.sprint_id === plannedSprint.id).length === 0 ? (
                        <div className="flex flex-col items-center justify-center h-full text-blue-400/80 text-sm p-4 border-2 border-dashed border-blue-200 rounded-lg bg-blue-50/50 pointer-events-none">
                            <p>左のバックログから<br/>矢印ボタンでストーリーを追加してください</p>
                        </div>
                    ) : (
                        plannedStories.map(story => {
                            const tasksForStory = tasks.filter(t => t.story_id === story.id);
                            return renderStoryItem(story, tasksForStory, true);
                        })
                    )}
                </div>
            </div>

            <StoryFormModal
                isOpen={isAddStoryModalOpen}
                initialData={storyFormInitialData}
                onClose={() => {
                    setIsAddStoryModalOpen(false);
                    setStoryFormInitialData(undefined);
                    setEditingStory(null);
                }}
                onSave={async (data) => {
                    if (editingStory) {
                        await updateStory({
                            ...editingStory,
                            ...data
                        });
                    } else {
                        await handleAddStory(data);
                    }
                }}
                onDelete={editingStory ? async () => {
                    await deleteStory(editingStory.id);
                } : undefined}
                title={editingStory ? "ストーリーを編集" : "ストーリーを追加"}
            />

            <IdeaRefinementDrawer
                isOpen={isIdeaRefinementModalOpen}
                onClose={() => setIsIdeaRefinementModalOpen(false)}
                onComplete={(data) => {
                    setStoryFormInitialData(data);
                    setIsIdeaRefinementModalOpen(false);
                    setIsAddStoryModalOpen(true);
                }}
            />
        </div>
    );
}
