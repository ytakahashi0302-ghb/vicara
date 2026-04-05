import { useState, useEffect } from 'react';
import { Modal } from './ui/Modal';
import { useSprintHistory, SprintHistoryData } from '../hooks/useSprintHistory';
import { Clock, CheckCircle, ChevronDown, ChevronRight, Activity } from 'lucide-react';

interface HistoryModalProps {
    isOpen: boolean;
    onClose: () => void;
}

export function HistoryModal({ isOpen, onClose }: HistoryModalProps) {
    const { historyData, loading, fetchHistory } = useSprintHistory();
    const [expandedSprints, setExpandedSprints] = useState<Record<string, boolean>>({});

    useEffect(() => {
        if (isOpen) {
            fetchHistory();
        }
    }, [isOpen, fetchHistory]);

    const toggleSprint = (sprintId: string) => {
        setExpandedSprints(prev => ({
            ...prev,
            [sprintId]: !prev[sprintId]
        }));
    };

    const formatDate = (ms: number) => {
        return new Date(ms).toLocaleString(undefined, {
            month: 'numeric',
            day: 'numeric',
            hour: '2-digit',
            minute: '2-digit'
        });
    };

    const formatDuration = (start: number, end: number) => {
        const diffMs = end - start;
        const hours = Math.floor(diffMs / (1000 * 60 * 60));
        const minutes = Math.floor((diffMs % (1000 * 60 * 60)) / (1000 * 60));
        return `${hours}h ${minutes}m`;
    };

    return (
        <Modal
            isOpen={isOpen}
            onClose={onClose}
            width="xl"
            title={
                <div className="flex items-center gap-2 text-gray-900">
                    <Activity size={20} className="text-gray-500" />
                    <span>スプリント履歴</span>
                </div>
            }
        >
            <div className="space-y-4 min-h-[300px] max-h-[70vh] overflow-y-auto pr-2 custom-scrollbar">
                {loading ? (
                    <div className="flex items-center justify-center h-40 text-gray-500">
                        <span className="animate-pulse">履歴を読み込み中...</span>
                    </div>
                ) : historyData.length === 0 ? (
                    <div className="flex flex-col items-center justify-center p-8 text-center bg-gray-50 rounded-lg border border-gray-100 border-dashed">
                        <Clock size={32} className="text-gray-300 mb-3" />
                        <h3 className="text-base font-medium text-gray-900 mb-1">完了したスプリントはありません</h3>
                        <p className="text-sm text-gray-500 max-w-xs">
                            8時間のスプリントを完了すると、これまでのスプリントの履歴がここに表示されます。
                        </p>
                    </div>
                ) : (
                    <div className="space-y-3">
                        {historyData.map((data: SprintHistoryData) => {
                            const isExpanded = !!expandedSprints[data.sprint.id];

                            return (
                                <div key={data.sprint.id} className="border border-gray-200 rounded-lg bg-white overflow-hidden shadow-sm transition-all">
                                    {/* Sprint Header */}
                                    <div
                                        className="flex items-center justify-between p-4 cursor-pointer hover:bg-gray-50 transition-colors"
                                        onClick={() => toggleSprint(data.sprint.id)}
                                    >
                                        <div className="flex flex-col gap-1 w-full sm:flex-row sm:items-center sm:justify-between pr-4">
                                            <div className="flex items-center gap-2">
                                                {isExpanded ?
                                                    <ChevronDown size={18} className="text-gray-400" /> :
                                                    <ChevronRight size={18} className="text-gray-400" />
                                                }
                                                <span className="font-semibold text-gray-900">
                                                    {data.sprint.completed_at ? formatDate(data.sprint.completed_at as number) : '未完了'}
                                                </span>
                                            </div>

                                            <div className="flex items-center gap-4 text-sm text-gray-500 pl-6 sm:pl-0">
                                                <div className="flex items-center gap-1.5 bg-gray-100 px-2.5 py-1 rounded-md">
                                                    <Clock size={14} className="text-gray-400" />
                                                    <span>{data.sprint.completed_at ? formatDuration(data.sprint.started_at || 0, data.sprint.completed_at || 0) : '測定中...'}</span>
                                                </div>
                                                <div className="flex items-center gap-1.5 font-medium">
                                                    <CheckCircle size={14} className="text-emerald-500" />
                                                    <span className="text-gray-700">{data.tasks.length} タスク</span>
                                                    {(data.stories.length > 0) && (
                                                        <span className="text-gray-400 ml-1">({data.stories.length} ストーリー完了)</span>
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                    </div>

                                    {/* Expanded Content */}
                                    {isExpanded && (
                                        <div className="px-5 py-4 bg-gray-50 border-t border-gray-100 divide-y divide-gray-100 animate-in slide-in-from-top-2 duration-200">
                                            {/* Stories Section (if any closed) */}
                                            {data.stories.length > 0 && (
                                                <div className="pb-4">
                                                    <h4 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">完了したストーリー</h4>
                                                    <ul className="space-y-2">
                                                        {data.stories.map(story => (
                                                            <li key={story.id} className="flex flex-col bg-white p-3 rounded shadow-sm border border-gray-100">
                                                                <span className="text-sm font-semibold text-gray-900">{story.title}</span>
                                                            </li>
                                                        ))}
                                                    </ul>
                                                </div>
                                            )}

                                            {/* Tasks Section */}
                                            <div className={data.stories.length > 0 ? "pt-4" : ""}>
                                                <h4 className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-2">完了したタスク</h4>
                                                {data.tasks.length > 0 ? (
                                                    <ul className="space-y-2">
                                                        {data.tasks.map(task => (
                                                            <li key={task.id} className="flex items-start gap-2.5 bg-white p-3 rounded shadow-sm border border-gray-100 group">
                                                                <CheckCircle size={16} className="text-emerald-500 mt-0.5 flex-shrink-0" />
                                                                <div className="flex flex-col">
                                                                    <span className="text-sm font-medium text-gray-900 group-hover:text-blue-600 transition-colors">{task.title}</span>
                                                                    {data.stories.some(s => s.id === task.story_id) === false && (
                                                                        <span className="text-xs text-gray-500 mt-0.5 flex items-center gap-1">
                                                                            <span className="w-1.5 h-1.5 rounded-full bg-blue-300"></span>
                                                                            アクティブなストーリーの一部
                                                                        </span>
                                                                    )}
                                                                </div>
                                                            </li>
                                                        ))}
                                                    </ul>
                                                ) : (
                                                    <p className="text-sm text-gray-500 italic px-2">このスプリントで完了したタスクはありません。</p>
                                                )}
                                            </div>
                                        </div>
                                    )}
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>

            <div className="mt-6 flex justify-end">
                <button
                    onClick={onClose}
                    className="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-emerald-500 transition-colors"
                >
                    閉じる
                </button>
            </div>
        </Modal>
    );
}
