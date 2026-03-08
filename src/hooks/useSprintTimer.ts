import { useState, useEffect, useCallback, useRef } from 'react';
import { load, Store } from '@tauri-apps/plugin-store';

export type SprintStatus = 'NOT_STARTED' | 'RUNNING' | 'PAUSED' | 'COMPLETED' | 'TIME_UP';

export interface SprintState {
    status: SprintStatus;
    remainingTimeMs: number;
    durationMs: number; // 動的に設定されたスプリントの総時間
    startedAt: number | null;
    hasNotifiedHalfway: boolean; // 50%経過通知のフラグ
}

const DEFAULT_SPRINT_TIME_MS = 1 * 60 * 60 * 1000; // 1 hour default

export function useSprintTimer() {
    const [state, setState] = useState<SprintState>({
        status: 'NOT_STARTED',
        remainingTimeMs: DEFAULT_SPRINT_TIME_MS,
        durationMs: DEFAULT_SPRINT_TIME_MS,
        startedAt: null,
        hasNotifiedHalfway: false
    });
    const [isLoaded, setIsLoaded] = useState(false);
    const [actualRemainingTime, setActualRemainingTime] = useState(DEFAULT_SPRINT_TIME_MS);
    const storeRef = useRef<Store | null>(null);

    // 共通の最新設定読み込み処理
    const getLatestDurationMs = useCallback(async (): Promise<number> => {
        let durationHours = 1;
        try {
            const settingsStore = await load('settings.json');
            const savedDuration = await settingsStore.get<{ value: number }>('sprint-duration-hours');
            if (savedDuration && typeof savedDuration === 'object' && 'value' in savedDuration) {
                durationHours = Number(savedDuration.value);
            } else if (typeof savedDuration === 'number') {
                durationHours = savedDuration;
            }
        } catch (e) {
            console.error('Failed to read sprint duration from store', e);
        }
        return durationHours * 60 * 60 * 1000;
    }, []);

    const saveState = useCallback(async (newState: SprintState) => {
        if (storeRef.current) {
            await storeRef.current.set('sprintState', newState);
            await storeRef.current.save();
        }
        setState(newState);
    }, []);

    // storeからの初期ロード
    useEffect(() => {
        let mounted = true;
        async function initStore() {
            try {
                const store = await load('sprint.json');
                storeRef.current = store;
                const savedState = await store.get<SprintState>('sprintState');
                if (savedState && mounted) {
                    if (savedState.status === 'RUNNING' && savedState.startedAt) {
                        const elapsed = Date.now() - savedState.startedAt;
                        let newRemaining = savedState.remainingTimeMs - elapsed;
                        if (newRemaining <= 0) {
                            savedState.status = 'TIME_UP';
                            savedState.remainingTimeMs = 0;
                            savedState.startedAt = null;
                            newRemaining = 0;
                        }
                        setState(savedState);
                        setActualRemainingTime(newRemaining);
                    } else {
                        // NOT_STARTED時は古いsprint.jsonの時間ではなく、最新のsettings.jsonの時間を優先する
                        if (savedState.status === 'NOT_STARTED') {
                            const latestDurationMs = await getLatestDurationMs();
                            savedState.durationMs = latestDurationMs;
                            savedState.remainingTimeMs = latestDurationMs;
                        }
                        setState(savedState);
                        setActualRemainingTime(savedState.remainingTimeMs);
                    }
                }
                if (mounted) setIsLoaded(true);
            } catch (err) {
                console.error('Failed to load sprint state:', err);
                if (mounted) setIsLoaded(true);
            }
        }
        initStore();
        return () => { mounted = false; };
    }, [getLatestDurationMs]);

    // 設定変更イベントの監視（NOT_STARTED時に即時反映する）
    useEffect(() => {
        const handleSettingsUpdated = async () => {
            if (state.status === 'NOT_STARTED') {
                const latestDurationMs = await getLatestDurationMs();
                const updatedState = {
                    ...state,
                    durationMs: latestDurationMs,
                    remainingTimeMs: latestDurationMs
                };
                setState(updatedState);
                setActualRemainingTime(latestDurationMs);
                saveState(updatedState);
            }
        };

        window.addEventListener('settings-updated', handleSettingsUpdated);
        return () => window.removeEventListener('settings-updated', handleSettingsUpdated);
    }, [state, saveState, getLatestDurationMs]);

    // タイマーの更新と通知ロジック
    useEffect(() => {
        let intervalId: number | undefined;

        if (isLoaded && state.status === 'RUNNING' && state.startedAt) {
            intervalId = window.setInterval(() => {
                const elapsed = Date.now() - state.startedAt!;
                const newRemaining = Math.max(0, state.remainingTimeMs - elapsed);
                setActualRemainingTime(newRemaining);

                if (newRemaining <= 0) {
                    saveState({
                        ...state,
                        status: 'TIME_UP',
                        remainingTimeMs: 0,
                        startedAt: null
                    });
                } else {
                    // 折り返し地点通知（50%経過）判定
                    const halfDuration = state.durationMs / 2;
                    if (newRemaining <= halfDuration && !state.hasNotifiedHalfway) {
                        // 発火（コンポーネント側で受け取れるようイベント発行）
                        window.dispatchEvent(new CustomEvent('sprint-halfway-notification'));

                        saveState({
                            ...state,
                            hasNotifiedHalfway: true
                        });
                    }
                }
            }, 1000);
        }

        return () => clearInterval(intervalId);
    }, [isLoaded, state, saveState]);

    const startSprint = async () => {
        const durationMs = await getLatestDurationMs();

        await saveState({
            status: 'RUNNING',
            remainingTimeMs: durationMs,
            durationMs: durationMs,
            startedAt: Date.now(),
            hasNotifiedHalfway: false
        });
    };

    const pauseSprint = async () => {
        if (state.status === 'RUNNING') {
            await saveState({
                ...state,
                status: 'PAUSED',
                remainingTimeMs: actualRemainingTime,
                startedAt: null
            });
        }
    };

    const resumeSprint = async () => {
        if (state.status === 'PAUSED') {
            await saveState({
                ...state,
                status: 'RUNNING',
                startedAt: Date.now()
            });
        }
    };

    const completeSprint = async () => {
        await saveState({
            ...state,
            status: 'COMPLETED',
            remainingTimeMs: actualRemainingTime,
            startedAt: null
        });
    };

    const resetSprint = async () => {
        const latestDurationMs = await getLatestDurationMs();
        setActualRemainingTime(latestDurationMs);
        await saveState({
            status: 'NOT_STARTED',
            remainingTimeMs: latestDurationMs,
            durationMs: latestDurationMs,
            startedAt: null,
            hasNotifiedHalfway: false
        });
    };

    return {
        status: state.status,
        remainingTimeMs: state.status === 'RUNNING' ? actualRemainingTime : state.remainingTimeMs,
        durationMs: state.durationMs,
        isLoaded,
        startSprint,
        pauseSprint,
        resumeSprint,
        completeSprint,
        resetSprint
    };
}
