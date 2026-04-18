import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { useSprintTimer } from '../context/SprintTimerContext';
import { Play, Pause, RotateCcw, CheckCircle, AlertTriangle, BellRing, Loader2 } from 'lucide-react';
import { Button } from './ui/Button';
import { useScrum } from '../context/ScrumContext';

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

const formatDurationLabel = (ms: number) => {
    const totalMinutes = Math.max(1, Math.round(ms / 60000));
    if (totalMinutes % 60 === 0) {
        return `${totalMinutes / 60}時間`;
    }
    if (totalMinutes > 60) {
        return `${(totalMinutes / 60).toFixed(1)}時間`;
    }
    return `${totalMinutes}分`;
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
        completeSprint: storeCompleteSprint,
        resetSprint
    } = useSprintTimer();

    const { refresh, sprints, completeSprint: completeScrumSprint } = useScrum();
    const [isArchiving, setIsArchiving] = useState(false);

    const [notificationMsg, setNotification] = useState<string | null>(null);
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
    const portalRoot = typeof document !== 'undefined' ? document.body : null;

    const handleStart = () => {
        setDismissedTimeUp(false);
        const activeSprint = sprints.find(s => s.status === 'Active');
        void startSprint({
            linkedSprintId: activeSprint?.id ?? null,
            reason: 'MANUAL',
        });
    };

    const handleComplete = async () => {
        if (status === 'RUNNING' || status === 'PAUSED' || status === 'TIME_UP') {
            const activeSprint = sprints.find(s => s.status === 'Active');
            if (!activeSprint) {
                alert('アクティブなスプリントが見つかりません。');
                return;
            }
            
            setIsArchiving(true);
            try {
                await completeScrumSprint(activeSprint.id, Date.now());
                await storeCompleteSprint();
                await refresh();
            } catch (error) {
                console.error('Failed to complete sprint', error);
            } finally {
                setIsArchiving(false);
            }
        }
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

            <div className="w-full mx-auto px-4 sm:px-6 lg:px-8 py-2.5 flex items-center justify-between">
                <div className="flex items-center gap-6">
                    <div className="flex flex-col">
                        <span className="text-xs font-semibold text-gray-500 uppercase tracking-wider mb-0.5">スプリントタイマー</span>
                        <div className={`font-mono text-xl font-bold tracking-tight tabular-nums ${isLate ? 'text-red-600' : 'text-gray-900'}`}>
                            {formatTime(remainingTimeMs)}
                        </div>
                    </div>

                    <div className="flex items-center gap-2 h-full pt-3">
                        {status === 'NOT_STARTED' && (
                            <Button onClick={handleStart} size="sm" className="bg-blue-600 hover:bg-blue-700 h-8">
                                <Play size={14} className="mr-1.5" />
                                スプリント開始
                            </Button>
                        )}

                        {status === 'RUNNING' && (
                            <Button onClick={pauseSprint} size="sm" variant="secondary" className="bg-orange-50 text-orange-700 hover:bg-orange-100 border-orange-200 h-8">
                                <Pause size={14} className="mr-1.5" />
                                一時停止
                            </Button>
                        )}

                        {status === 'PAUSED' && (
                            <Button onClick={resumeSprint} size="sm" className="bg-blue-600 hover:bg-blue-700 h-8">
                                <Play size={14} className="mr-1.5" />
                                再開
                            </Button>
                        )}

                        {(status === 'RUNNING' || status === 'PAUSED' || status === 'TIME_UP') && (
                            <Button onClick={handleComplete} disabled={isArchiving} size="sm" className="bg-emerald-600 hover:bg-emerald-700 border-emerald-600 text-white h-8">
                                {isArchiving ? <Loader2 size={14} className="mr-1.5 animate-spin" /> : <CheckCircle size={14} className="mr-1.5" />}
                                完了にする
                            </Button>
                        )}

                        {(status === 'COMPLETED' || status === 'TIME_UP') && (
                            <Button onClick={handleReset} size="sm" variant="secondary" className="h-8">
                                <RotateCcw size={14} className="mr-1.5" />
                                タイマーリセット
                            </Button>
                        )}
                    </div>
                </div>

                <div className="flex items-center">
                    {status === 'TIME_UP' && (
                        <div className="flex items-center text-red-600 text-sm font-semibold bg-red-50 px-3 py-1.5 rounded-md border border-red-100">
                            <AlertTriangle size={16} className="mr-1.5" />
                            時間は終了しました！スプリント完了処理を行ってください。
                        </div>
                    )}
                    {status === 'COMPLETED' && (
                        <div className="flex items-center text-emerald-600 text-sm font-semibold bg-emerald-50 px-3 py-1.5 rounded-md border border-emerald-100">
                            <CheckCircle size={16} className="mr-1.5" />
                            スプリントが完了しました！
                        </div>
                    )}
                </div>
            </div>

            {/* Notification Toast for Daily Scrum */}
            {notificationMsg && portalRoot && createPortal(
                <div className="fixed bottom-6 right-6 bg-white border border-blue-200 shadow-2xl rounded-xl p-5 flex items-start gap-4 z-[120] animate-in slide-in-from-bottom-5 fade-in duration-300">
                    <div className="bg-blue-100 text-blue-600 p-2.5 rounded-full flex-shrink-0 mt-0.5">
                        <BellRing size={20} className="animate-pulse" />
                    </div>
                    <div>
                        <h4 className="font-bold text-gray-900 text-sm">デイリースクラム (Mini-Retro)</h4>
                        <p className="text-gray-600 text-sm mt-1">{notificationMsg}</p>
                    </div>
                </div>,
                portalRoot
            )}

            {/* Time's Up Modal */}
            {showTimeUpModal && portalRoot && createPortal(
                <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-sm flex items-center justify-center z-[130] animate-in fade-in">
                    <div className="bg-white rounded-2xl shadow-2xl p-8 max-w-sm w-full text-center animate-in zoom-in-95 duration-200">
                        <div className="mx-auto bg-red-100 w-20 h-20 rounded-full flex items-center justify-center mb-6">
                            <AlertTriangle size={40} className="text-red-600" />
                        </div>
                        <h2 className="text-3xl font-black text-gray-900 mb-3 tracking-tight">時間終了！</h2>
                        <p className="text-gray-600 mb-8 leading-relaxed">
                            {formatDurationLabel(durationMs)}のスプリントが終了しました。残りのタスクを整理し、スプリントを完了してください。
                        </p>
                        <div className="flex gap-3">
                            <Button onClick={() => setDismissedTimeUp(true)} variant="secondary" className="flex-1 h-11 text-base">
                                閉じる
                            </Button>
                            <Button onClick={() => { setDismissedTimeUp(true); handleComplete(); }} disabled={isArchiving} className="flex-1 h-11 text-base bg-emerald-600 hover:bg-emerald-700 border-emerald-600 focus:ring-emerald-500">
                                {isArchiving ? <Loader2 size={16} className="mr-2 animate-spin" /> : <CheckCircle size={16} className="mr-2" />}
                                スプリント完了にする
                            </Button>
                        </div>
                    </div>
                </div>,
                portalRoot
            )}
        </div>
    );
}
