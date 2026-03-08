import { useEffect, useState } from 'react';
import { useSprintTimer } from '../hooks/useSprintTimer';
import { Play, Pause, RotateCcw, CheckCircle, AlertTriangle, BellRing } from 'lucide-react';
import { Button } from './ui/Button';

const formatTime = (ms: number) => {
    const totalSeconds = Math.floor(ms / 1000);
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;

    return [
        hours.toString().padStart(2, '0'),
        minutes.toString().padStart(2, '0'),
        seconds.toString().padStart(2, '0')
    ].join(':');
};

export function SprintTimer() {
    const {
        status,
        remainingTimeMs,
        durationMs,
        isLoaded,
        startSprint,
        pauseSprint,
        resumeSprint,
        completeSprint,
        resetSprint
    } = useSprintTimer();

    const [notification, setNotification] = useState<string | null>(null);
    const [dismissedTimeUp, setDismissedTimeUp] = useState(false);

    useEffect(() => {
        const handleNotification = () => {
            setNotification(`折り返し地点です。現在のタスクの進捗は順調ですか？`);

            // 15秒後にフェードアウト
            setTimeout(() => {
                setNotification(null);
            }, 15000);
        };

        window.addEventListener('sprint-halfway-notification', handleNotification);
        return () => window.removeEventListener('sprint-halfway-notification', handleNotification);
    }, []);

    const showTimeUpModal = status === 'TIME_UP' && !dismissedTimeUp;

    const handleStart = () => {
        setDismissedTimeUp(false);
        startSprint();
    };

    const handleReset = () => {
        setDismissedTimeUp(false);
        resetSprint();
    };

    if (!isLoaded) return null;

    const progressPercent = Math.max(0, Math.min(100, ((durationMs - Math.max(0, remainingTimeMs)) / durationMs) * 100));

    // 時間に応じて色を変えるアフォーダンス (割合ベース)
    const isLate = progressPercent >= 90; // 残り10%未満 (90%経過)
    const isWarning = progressPercent >= 50 && !isLate; // 残り50%未満
    const progressColor = isLate ? 'bg-red-500' : isWarning ? 'bg-amber-500' : 'bg-emerald-500';

    return (
        <div className="w-full bg-white border-b border-gray-200">
            {/* ProgressBar */}
            <div className="w-full h-1 bg-gray-100">
                <div
                    className={`h-full transition-all duration-1000 ease-linear ${progressColor}`}
                    style={{ width: `${progressPercent}%` }}
                />
            </div>

            <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-2.5 flex items-center justify-between">
                <div className="flex items-center gap-6">
                    <div className="flex flex-col">
                        <span className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-0.5">Sprint Timer</span>
                        <div className={`font-mono text-xl font-bold tracking-tight tabular-nums ${isLate ? 'text-red-600' : 'text-gray-900'}`}>
                            {formatTime(remainingTimeMs)}
                        </div>
                    </div>

                    <div className="flex items-center gap-2 h-full pt-3">
                        {status === 'NOT_STARTED' && (
                            <Button onClick={handleStart} size="sm" className="bg-blue-600 hover:bg-blue-700 h-8">
                                <Play size={14} className="mr-1.5" />
                                Start Sprint
                            </Button>
                        )}

                        {status === 'RUNNING' && (
                            <Button onClick={pauseSprint} size="sm" variant="secondary" className="bg-orange-50 text-orange-700 hover:bg-orange-100 border-orange-200 h-8">
                                <Pause size={14} className="mr-1.5" />
                                Pause
                            </Button>
                        )}

                        {status === 'PAUSED' && (
                            <Button onClick={resumeSprint} size="sm" className="bg-blue-600 hover:bg-blue-700 h-8">
                                <Play size={14} className="mr-1.5" />
                                Resume
                            </Button>
                        )}

                        {(status === 'RUNNING' || status === 'PAUSED') && (
                            <Button onClick={completeSprint} size="sm" className="bg-emerald-600 hover:bg-emerald-700 border-emerald-600 text-white h-8">
                                <CheckCircle size={14} className="mr-1.5" />
                                Complete
                            </Button>
                        )}

                        {(status === 'COMPLETED' || status === 'TIME_UP') && (
                            <Button onClick={handleReset} size="sm" variant="secondary" className="h-8">
                                <RotateCcw size={14} className="mr-1.5" />
                                Reset Timer
                            </Button>
                        )}
                    </div>
                </div>

                <div className="flex items-center">
                    {status === 'TIME_UP' && (
                        <div className="flex items-center text-red-600 text-sm font-semibold bg-red-50 px-3 py-1.5 rounded-md border border-red-100">
                            <AlertTriangle size={16} className="mr-1.5" />
                            Time is up! Sprint ended.
                        </div>
                    )}
                    {status === 'COMPLETED' && (
                        <div className="flex items-center text-emerald-600 text-sm font-semibold bg-emerald-50 px-3 py-1.5 rounded-md border border-emerald-100">
                            <CheckCircle size={16} className="mr-1.5" />
                            Sprint completed early!
                        </div>
                    )}
                </div>
            </div>

            {/* Notification Toast for Daily Scrum */}
            {notification && (
                <div className="fixed bottom-6 right-6 bg-white border border-blue-200 shadow-2xl rounded-xl p-5 flex items-start gap-4 z-50 animate-in slide-in-from-bottom-5 fade-in duration-300">
                    <div className="bg-blue-100 text-blue-600 p-2.5 rounded-full flex-shrink-0 mt-0.5">
                        <BellRing size={20} className="animate-pulse" />
                    </div>
                    <div>
                        <h4 className="font-bold text-gray-900 text-sm">Daily Scrum (Mini-Retro)</h4>
                        <p className="text-gray-600 text-sm mt-1">{notification}</p>
                    </div>
                </div>
            )}

            {/* Time's Up Modal */}
            {showTimeUpModal && (
                <div className="fixed inset-0 bg-gray-900/60 backdrop-blur-sm flex items-center justify-center z-[100] animate-in fade-in">
                    <div className="bg-white rounded-2xl shadow-2xl p-8 max-w-sm w-full text-center animate-in zoom-in-95 duration-200">
                        <div className="mx-auto bg-red-100 w-20 h-20 rounded-full flex items-center justify-center mb-6">
                            <AlertTriangle size={40} className="text-red-600" />
                        </div>
                        <h2 className="text-3xl font-black text-gray-900 mb-3 tracking-tight">Time's Up!</h2>
                        <p className="text-gray-600 mb-8 leading-relaxed">
                            The 8-hour sprint has automatically ended. Please complete any closing tasks and review your sprint progress.
                        </p>
                        <Button onClick={() => setDismissedTimeUp(true)} className="w-full h-11 text-base relative overflow-hidden group">
                            <span className="relative z-10">Acknowledge</span>
                            <div className="absolute inset-0 bg-white/20 translate-y-full group-hover:translate-y-0 transition-transform" />
                        </Button>
                    </div>
                </div>
            )}
        </div>
    );
}
