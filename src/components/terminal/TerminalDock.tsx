import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Terminal as XTerm } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import 'xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useScrum } from '../../context/ScrumContext';
import {
    AlertTriangle,
    CheckCircle2,
    Loader2,
    SquareTerminal,
    StopCircle,
} from 'lucide-react';
import toast from 'react-hot-toast';

type TerminalSessionStatus = 'Starting' | 'Running' | 'Completed' | 'Failed' | 'Killed';

interface ActiveClaudeSession {
    task_id: string;
    task_title: string;
    role_name: string;
    model: string;
    started_at: number;
    status: 'Starting' | 'Running';
}

interface TerminalTabSession {
    taskId: string;
    taskTitle: string;
    roleName: string;
    model: string;
    startedAt: number;
    status: TerminalSessionStatus;
    logs: string;
    exitReason?: string;
}

interface ClaudeOutputPayload {
    task_id: string;
    output: string;
}

interface ClaudeExitPayload {
    task_id: string;
    success: boolean;
    reason: string;
}

const WELCOME_MESSAGE = '\x1b[38;5;12m[MicroScrum AI]\x1b[0m Dev Agent Terminal Ready.\r\n';

function buildSessionHeader(session: Pick<TerminalTabSession, 'roleName' | 'taskTitle' | 'model'>): string {
    return `\x1b[38;5;12m[${session.roleName}]\x1b[0m ${session.taskTitle}\r\n\x1b[38;5;8mModel: ${session.model || 'unknown'}\x1b[0m\r\n\r\n`;
}

function createSessionFromActiveSession(payload: ActiveClaudeSession): TerminalTabSession {
    const baseSession: TerminalTabSession = {
        taskId: payload.task_id,
        taskTitle: payload.task_title,
        roleName: payload.role_name,
        model: payload.model,
        startedAt: payload.started_at,
        status: payload.status,
        logs: '',
    };

    return {
        ...baseSession,
        logs: buildSessionHeader(baseSession),
    };
}

function createPlaceholderSession(taskId: string): TerminalTabSession {
    return {
        taskId,
        taskTitle: taskId,
        roleName: 'Unknown Role',
        model: '',
        startedAt: Date.now(),
        status: 'Running',
        logs: '',
    };
}

function createExitLine(success: boolean, reason: string): string {
    const color = success ? '\x1b[32m' : '\x1b[31m';
    return `\r\n${color}✔ Process Exited: ${reason}\x1b[0m\r\n`;
}

function mapExitStatus(success: boolean, reason: string): TerminalSessionStatus {
    if (success) return 'Completed';
    if (reason.includes('Manually killed')) return 'Killed';
    return 'Failed';
}

function isSessionRunning(status: TerminalSessionStatus): boolean {
    return status === 'Starting' || status === 'Running';
}

function StatusIndicator({ status }: { status: TerminalSessionStatus }) {
    if (status === 'Starting' || status === 'Running') {
        return <Loader2 size={14} className="shrink-0 animate-spin text-emerald-500" />;
    }
    if (status === 'Completed') {
        return <CheckCircle2 size={14} className="shrink-0 text-emerald-500" />;
    }
    if (status === 'Killed') {
        return <StopCircle size={14} className="shrink-0 text-amber-500" />;
    }
    return <AlertTriangle size={14} className="shrink-0 text-red-500" />;
}

export const TerminalDock: React.FC = () => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<XTerm | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const safeFitRef = useRef<() => void>(() => undefined);
    const activeTaskIdRef = useRef<string | null>(null);
    const sessionsRef = useRef<Record<string, TerminalTabSession>>({});
    const { updateTaskStatus } = useScrum();
    const [sessions, setSessions] = useState<Record<string, TerminalTabSession>>({});
    const [activeTaskId, setActiveTaskId] = useState<string | null>(null);

    const sortedSessions = useMemo(
        () => Object.values(sessions).sort((a, b) => a.startedAt - b.startedAt),
        [sessions]
    );

    const activeSession = activeTaskId ? sessions[activeTaskId] ?? null : null;
    const canKillActiveSession = activeSession ? isSessionRunning(activeSession.status) : false;

    useEffect(() => {
        activeTaskIdRef.current = activeTaskId;
    }, [activeTaskId]);

    useEffect(() => {
        sessionsRef.current = sessions;
    }, [sessions]);

    useEffect(() => {
        if (!terminalRef.current) return;

        const fitAddon = new FitAddon();
        const term = new XTerm({
            theme: {
                background: '#1e1e1e',
                foreground: '#cccccc',
                cursor: '#ffffff',
                selectionBackground: '#434c5e',
                black: '#000000',
                red: '#cd3131',
                green: '#0dbc79',
                yellow: '#e5e510',
                blue: '#2472c8',
                magenta: '#bc3fbc',
                cyan: '#11a8cd',
                white: '#e5e5e5',
                brightBlack: '#666666',
                brightRed: '#f14c4c',
                brightGreen: '#23d18b',
                brightYellow: '#f5f543',
                brightBlue: '#3b8eea',
                brightMagenta: '#d670d6',
                brightCyan: '#29b8db',
                brightWhite: '#e5e5e5',
            },
            fontFamily: 'Consolas, "Courier New", monospace',
            fontSize: 13,
            cursorBlink: true,
            convertEol: true,
            disableStdin: true,
        });

        const safeFit = () => {
            if (
                terminalRef.current &&
                xtermRef.current &&
                fitAddonRef.current &&
                terminalRef.current.offsetWidth > 0 &&
                terminalRef.current.offsetHeight > 0
            ) {
                requestAnimationFrame(() => {
                    if (
                        !terminalRef.current ||
                        !xtermRef.current ||
                        !fitAddonRef.current ||
                        terminalRef.current.offsetWidth === 0 ||
                        terminalRef.current.offsetHeight === 0
                    ) {
                        return;
                    }

                    try {
                        fitAddonRef.current.fit();
                    } catch (error) {
                        console.warn('xterm fit error ignored:', error);
                    }
                });
            }
        };

        safeFitRef.current = safeFit;

        term.loadAddon(fitAddon);
        term.open(terminalRef.current);
        term.write(WELCOME_MESSAGE);

        xtermRef.current = term;
        fitAddonRef.current = fitAddon;

        setTimeout(safeFit, 50);

        const resizeObserver = new ResizeObserver(() => {
            safeFit();
        });
        resizeObserver.observe(terminalRef.current);

        return () => {
            resizeObserver.disconnect();
            term.dispose();
            xtermRef.current = null;
            fitAddonRef.current = null;
            safeFitRef.current = () => undefined;
        };
    }, []);

    useEffect(() => {
        const term = xtermRef.current;
        if (!term) return;

        const session = activeTaskId ? sessionsRef.current[activeTaskId] : null;
        term.reset();
        term.write(session?.logs || WELCOME_MESSAGE);
        safeFitRef.current();
    }, [activeTaskId]);

    useEffect(() => {
        let cancelled = false;
        let unlistenStarted: (() => void) | null = null;
        let unlistenOutput: (() => void) | null = null;
        let unlistenExit: (() => void) | null = null;

        const restoreSessions = async () => {
            try {
                const activeSessions = await invoke<ActiveClaudeSession[]>('get_active_claude_sessions');
                if (cancelled || activeSessions.length === 0) return;

                setSessions((prev) => {
                    const next = { ...prev };
                    for (const restored of activeSessions) {
                        const existing = next[restored.task_id];
                        next[restored.task_id] = existing
                            ? {
                                ...existing,
                                taskTitle: restored.task_title,
                                roleName: restored.role_name,
                                model: restored.model,
                                startedAt: restored.started_at,
                                status: restored.status,
                            }
                            : {
                                ...createSessionFromActiveSession(restored),
                                logs:
                                    createSessionFromActiveSession(restored).logs +
                                    '\x1b[38;5;12m[MicroScrum AI]\x1b[0m 進行中セッションを復元しました。\r\n',
                            };
                    }
                    return next;
                });

                setActiveTaskId((prev) => prev ?? activeSessions[0].task_id);
            } catch (error) {
                console.error('Failed to restore active Claude sessions', error);
                toast.error(`実行中セッションの復元に失敗しました: ${error}`);
            }
        };

        const setupListeners = async () => {
            const us = await listen<ActiveClaudeSession>('claude_cli_started', (event) => {
                setSessions((prev) => {
                    const existing = prev[event.payload.task_id];
                    const created = createSessionFromActiveSession(event.payload);
                    return {
                        ...prev,
                        [event.payload.task_id]: existing
                            ? {
                                ...existing,
                                taskTitle: event.payload.task_title,
                                roleName: event.payload.role_name,
                                model: event.payload.model,
                                startedAt: event.payload.started_at,
                                status: 'Running',
                                logs: existing.logs || created.logs,
                            }
                            : created,
                    };
                });
                setActiveTaskId(event.payload.task_id);
            });
            if (cancelled) {
                us();
                return;
            }
            unlistenStarted = us;

            const uo = await listen<ClaudeOutputPayload>('claude_cli_output', (event) => {
                setSessions((prev) => {
                    const existing = prev[event.payload.task_id] ?? createPlaceholderSession(event.payload.task_id);
                    const withHeader = existing.logs
                        ? existing.logs
                        : buildSessionHeader(existing);
                    return {
                        ...prev,
                        [event.payload.task_id]: {
                            ...existing,
                            status: existing.status === 'Starting' ? 'Running' : existing.status,
                            logs: withHeader + event.payload.output,
                        },
                    };
                });

                if (activeTaskIdRef.current === event.payload.task_id && xtermRef.current) {
                    xtermRef.current.write(event.payload.output);
                }
            });
            if (cancelled) {
                uo();
                return;
            }
            unlistenOutput = uo;

            const ue = await listen<ClaudeExitPayload>('claude_cli_exit', async (event) => {
                const exitLine = createExitLine(event.payload.success, event.payload.reason);
                const nextStatus = mapExitStatus(event.payload.success, event.payload.reason);

                setSessions((prev) => {
                    const existing = prev[event.payload.task_id] ?? createPlaceholderSession(event.payload.task_id);
                    const withHeader = existing.logs
                        ? existing.logs
                        : buildSessionHeader(existing);
                    return {
                        ...prev,
                        [event.payload.task_id]: {
                            ...existing,
                            status: nextStatus,
                            exitReason: event.payload.reason,
                            logs: withHeader + exitLine,
                        },
                    };
                });

                if (activeTaskIdRef.current === event.payload.task_id && xtermRef.current) {
                    xtermRef.current.write(exitLine);
                }

                if (event.payload.success) {
                    await updateTaskStatus(event.payload.task_id, 'Done');
                    toast.success('開発が完了しました。レビューをお願いします。');
                } else {
                    toast.error(`プロセス終了: ${event.payload.reason}`);
                }
            });
            if (cancelled) {
                ue();
                return;
            }
            unlistenExit = ue;
        };

        const handleFrontendError = (e: Event) => {
            const ce = e as CustomEvent;
            const message = `\r\n\x1b[31m[Invoke Error] ${String(ce.detail)}\x1b[0m\r\n`;
            const currentActiveTaskId = activeTaskIdRef.current;

            if (currentActiveTaskId) {
                setSessions((prev) => {
                    const existing = prev[currentActiveTaskId];
                    if (!existing) return prev;
                    return {
                        ...prev,
                        [currentActiveTaskId]: {
                            ...existing,
                            logs: existing.logs + message,
                        },
                    };
                });
            }

            if (xtermRef.current && (!currentActiveTaskId || sessionsRef.current[currentActiveTaskId])) {
                xtermRef.current.write(message);
            }
        };

        window.addEventListener('claude_error', handleFrontendError);
        restoreSessions();
        setupListeners();

        return () => {
            cancelled = true;
            if (unlistenStarted) unlistenStarted();
            if (unlistenOutput) unlistenOutput();
            if (unlistenExit) unlistenExit();
            window.removeEventListener('claude_error', handleFrontendError);
        };
    }, [updateTaskStatus]);

    const handleSelectTab = (taskId: string) => {
        setActiveTaskId(taskId);
        setTimeout(() => {
            safeFitRef.current();
        }, 0);
    };

    const handleKill = async () => {
        if (!activeSession || !canKillActiveSession) return;
        try {
            await invoke('kill_claude_process', { taskId: activeSession.taskId });
            toast.success('強制終了シグナルを送信しました');
        } catch (e: any) {
            toast.error(`Kill Error: ${e}`);
        }
    };

    return (
        <div className="relative flex h-full min-h-0 w-full flex-col">
            <div className="border-b border-gray-200 bg-gray-50 px-2 py-2">
                {sortedSessions.length === 0 ? (
                    <div className="flex items-center gap-2 rounded-md border border-dashed border-gray-300 bg-white px-3 py-2 text-sm text-gray-500">
                        <SquareTerminal size={16} />
                        実行中または履歴表示中のエージェントはありません
                    </div>
                ) : (
                    <div className="flex gap-2 overflow-x-auto pb-1">
                        {sortedSessions.map((session) => {
                            const isActive = session.taskId === activeTaskId;
                            return (
                                <button
                                    key={session.taskId}
                                    type="button"
                                    onClick={() => handleSelectTab(session.taskId)}
                                    className={`min-w-[220px] max-w-[280px] rounded-lg border px-3 py-2 text-left transition-colors ${
                                        isActive
                                            ? 'border-blue-300 bg-white shadow-sm'
                                            : 'border-gray-200 bg-white/70 hover:border-gray-300 hover:bg-white'
                                    }`}
                                    title={`${session.roleName} / ${session.taskTitle}`}
                                >
                                    <div className="flex items-start gap-2">
                                        <StatusIndicator status={session.status} />
                                        <div className="min-w-0 flex-1">
                                            <div className="truncate text-xs font-semibold uppercase tracking-wide text-gray-500">
                                                {session.roleName}
                                            </div>
                                            <div className="truncate text-sm font-medium text-gray-900">
                                                {session.taskTitle}
                                            </div>
                                            <div className="truncate text-xs text-gray-500">
                                                {session.status}
                                            </div>
                                        </div>
                                    </div>
                                </button>
                            );
                        })}
                    </div>
                )}
            </div>

            <div className="relative min-h-0 flex-1 bg-[#1e1e1e]">
                <div ref={terminalRef} className="h-full w-full rounded-b overflow-hidden" />
                {canKillActiveSession && (
                    <button
                        onClick={handleKill}
                        className="absolute right-4 top-2 z-10 flex items-center gap-2 rounded-md bg-red-600 px-3 py-1.5 text-sm font-medium text-white shadow-lg transition-colors hover:bg-red-500"
                        title="現在表示中の Claude プロセスを強制停止します"
                    >
                        <StopCircle size={16} />
                        このタブを強制停止
                    </button>
                )}
            </div>
        </div>
    );
};
