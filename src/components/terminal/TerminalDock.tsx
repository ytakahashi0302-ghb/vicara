import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Terminal as XTerm } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import 'xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useScrum } from '../../context/ScrumContext';
import { TeamConfiguration, TeamRoleSetting } from '../../types';
import {
    AlertTriangle,
    ChevronDown,
    ChevronUp,
    CheckCircle2,
    Loader2,
    SquareTerminal,
    StopCircle,
    X,
} from 'lucide-react';
import toast from 'react-hot-toast';
import { Avatar } from '../ai/Avatar';
import { resolveAvatarForRoleName } from '../ai/avatarRegistry';
import { VICARA_SETTINGS_UPDATED_EVENT } from '../../hooks/usePoAssistantAvatarImage';

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
    new_status?: string;
}

interface ClaudeStreamRenderState {
    buffer: string;
    hasThinking: boolean;
}

interface FrontendAgentErrorDetail {
    taskId?: string;
    taskTitle?: string;
    roleName?: string;
    model?: string;
    message: string;
}

const WELCOME_MESSAGE = '\x1b[38;5;12m[vicara]\x1b[0m Dev Agent Terminal Ready.\r\n';

function createClaudeStreamRenderState(): ClaudeStreamRenderState {
    return {
        buffer: '',
        hasThinking: false,
    };
}

function asRecord(value: unknown): Record<string, unknown> | null {
    return value && typeof value === 'object' ? (value as Record<string, unknown>) : null;
}

function getStringValue(value: unknown): string | null {
    return typeof value === 'string' ? value : null;
}

function renderClaudeAssistantText(messageValue: unknown, hasThinking: boolean): string {
    if (hasThinking) {
        return '';
    }

    const message = asRecord(messageValue);
    const content = Array.isArray(message?.content) ? message.content : [];
    return content
        .map((block) => {
            const blockRecord = asRecord(block);
            if (blockRecord?.type !== 'text') {
                return '';
            }
            return getStringValue(blockRecord.text) ?? '';
        })
        .join('');
}

function renderClaudeStreamJsonLine(line: string, hasThinking: boolean): {
    output: string;
    nextHasThinking: boolean;
} {
    const trimmed = line.trim();
    if (!trimmed) {
        return { output: '', nextHasThinking: hasThinking };
    }

    try {
        const payload = JSON.parse(trimmed) as unknown;
        const root = asRecord(payload);
        if (!root) {
            return { output: '', nextHasThinking: hasThinking };
        }

        if (root.type === 'stream_event') {
            const event = asRecord(root.event);
            if (!event) {
                return { output: '', nextHasThinking: hasThinking };
            }

            if (event.type === 'content_block_delta') {
                const delta = asRecord(event.delta);
                if (!delta) {
                    return { output: '', nextHasThinking: hasThinking };
                }

                if (delta.type === 'thinking_delta') {
                    return {
                        output: getStringValue(delta.thinking) ?? '',
                        nextHasThinking: true,
                    };
                }

                if (delta.type === 'text_delta' && !hasThinking) {
                    return {
                        output: getStringValue(delta.text) ?? '',
                        nextHasThinking: hasThinking,
                    };
                }
            }

            return { output: '', nextHasThinking: hasThinking };
        }

        if (root.type === 'assistant') {
            return {
                output: renderClaudeAssistantText(root.message, hasThinking),
                nextHasThinking: hasThinking,
            };
        }

        return { output: '', nextHasThinking: hasThinking };
    } catch {
        return {
            output: line.replace(/\n/g, '\r\n') + '\r\n',
            nextHasThinking: hasThinking,
        };
    }
}

function consumeClaudeStreamChunk(
    chunk: string,
    previousState: ClaudeStreamRenderState,
    flush = false,
): {
    output: string;
    nextState: ClaudeStreamRenderState;
} {
    const normalizedChunk = chunk.replace(/\r\n/g, '\n');
    const combined = previousState.buffer + normalizedChunk;
    const lines = combined.split('\n');
    const trailingBuffer = lines.pop() ?? '';
    const nextBuffer = flush ? '' : trailingBuffer;

    const rendered: string[] = [];
    let nextHasThinking = previousState.hasThinking;

    for (const line of lines) {
        const result = renderClaudeStreamJsonLine(line, nextHasThinking);
        nextHasThinking = result.nextHasThinking;
        if (result.output) {
            rendered.push(result.output);
        }
    }

    if (flush && trailingBuffer) {
        const result = renderClaudeStreamJsonLine(trailingBuffer, nextHasThinking);
        nextHasThinking = result.nextHasThinking;
        if (result.output) {
            rendered.push(result.output);
        }
    }

    return {
        output: rendered.join(''),
        nextState: {
            buffer: nextBuffer,
            hasThinking: nextHasThinking,
        },
    };
}

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

function createSessionFromFrontendError(detail: FrontendAgentErrorDetail): TerminalTabSession {
    const baseSession: TerminalTabSession = {
        taskId: detail.taskId ?? `frontend-error-${Date.now()}`,
        taskTitle: detail.taskTitle?.trim() || detail.taskId || 'Frontend Error',
        roleName: detail.roleName?.trim() || 'Unknown Role',
        model: detail.model?.trim() || '',
        startedAt: Date.now(),
        status: 'Failed',
        logs: '',
        exitReason: detail.message,
    };

    return {
        ...baseSession,
        logs: buildSessionHeader(baseSession),
    };
}

function normalizeFrontendErrorDetail(
    detail: unknown,
    fallbackTaskId: string | null,
): FrontendAgentErrorDetail {
    if (typeof detail === 'string') {
        return {
            taskId: fallbackTaskId ?? undefined,
            message: detail,
        };
    }

    if (typeof detail === 'object' && detail !== null) {
        const raw = detail as Record<string, unknown>;
        return {
            taskId: typeof raw.taskId === 'string' ? raw.taskId : fallbackTaskId ?? undefined,
            taskTitle: typeof raw.taskTitle === 'string' ? raw.taskTitle : undefined,
            roleName: typeof raw.roleName === 'string' ? raw.roleName : undefined,
            model: typeof raw.model === 'string' ? raw.model : undefined,
            message:
                typeof raw.message === 'string'
                    ? raw.message
                    : typeof raw.detail === 'string'
                      ? raw.detail
                      : String(detail),
        };
    }

    return {
        taskId: fallbackTaskId ?? undefined,
        message: String(detail),
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

function getNextActiveTaskId(
    sessions: Record<string, TerminalTabSession>,
    removedTaskId: string,
    currentActiveTaskId: string | null,
) {
    if (currentActiveTaskId !== removedTaskId) {
        return currentActiveTaskId;
    }

    const remainingSessions = Object.values(sessions)
        .filter((session) => session.taskId !== removedTaskId)
        .sort((a, b) => a.startedAt - b.startedAt);

    return remainingSessions.length > 0
        ? remainingSessions[remainingSessions.length - 1].taskId
        : null;
}

function StatusIndicator({ status }: { status: TerminalSessionStatus }) {
    if (status === 'Starting' || status === 'Running') {
        return <Loader2 size={14} className="shrink-0 animate-spin text-sky-400" />;
    }
    if (status === 'Completed') {
        return <CheckCircle2 size={14} className="shrink-0 text-emerald-400" />;
    }
    if (status === 'Killed') {
        return <StopCircle size={14} className="shrink-0 text-amber-400" />;
    }
    return <AlertTriangle size={14} className="shrink-0 text-rose-400" />;
}

interface TerminalDockProps {
    isMinimized: boolean;
    onToggleMinimize: () => void;
}

export const TerminalDock: React.FC<TerminalDockProps> = ({ isMinimized, onToggleMinimize }) => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<XTerm | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const fitTimeoutRef = useRef<number | null>(null);
    const fitRafRef = useRef<number | null>(null);
    const safeFitRef = useRef<() => void>(() => undefined);
    const activeTaskIdRef = useRef<string | null>(null);
    const sessionsRef = useRef<Record<string, TerminalTabSession>>({});
    const claudeStreamStateRef = useRef<Record<string, ClaudeStreamRenderState>>({});
    const { updateTaskStatus } = useScrum();
    const [sessions, setSessions] = useState<Record<string, TerminalTabSession>>({});
    const [activeTaskId, setActiveTaskId] = useState<string | null>(null);
    const [teamRoles, setTeamRoles] = useState<TeamRoleSetting[]>([]);

    const sortedSessions = useMemo(
        () => Object.values(sessions).sort((a, b) => a.startedAt - b.startedAt),
        [sessions]
    );

    const activeSession = activeTaskId ? sessions[activeTaskId] ?? null : null;
    const roleLookupByName = useMemo(
        () =>
            new Map(
                teamRoles
                    .map((role) => [role.name.trim(), role] as const)
                    .filter(([roleName]) => roleName.length > 0),
            ),
        [teamRoles],
    );
    const activeSessionRole = activeSession
        ? roleLookupByName.get(activeSession.roleName.trim()) ?? null
        : null;
    const canKillActiveSession = activeSession ? isSessionRunning(activeSession.status) : false;
    const activeSessionAvatar = activeSession
        ? resolveAvatarForRoleName(activeSession.roleName)
        : null;
    const completedSessionCount = useMemo(
        () => sortedSessions.filter((session) => !isSessionRunning(session.status)).length,
        [sortedSessions],
    );

    const cancelPendingFit = () => {
        if (fitTimeoutRef.current !== null) {
            window.clearTimeout(fitTimeoutRef.current);
            fitTimeoutRef.current = null;
        }

        if (fitRafRef.current !== null) {
            window.cancelAnimationFrame(fitRafRef.current);
            fitRafRef.current = null;
        }
    };

    useEffect(() => {
        activeTaskIdRef.current = activeTaskId;
    }, [activeTaskId]);

    useEffect(() => {
        sessionsRef.current = sessions;
    }, [sessions]);

    useEffect(() => {
        let cancelled = false;

        const loadTeamRoles = async () => {
            try {
                const config = await invoke<TeamConfiguration>('get_team_configuration');
                if (!cancelled) {
                    setTeamRoles(config.roles);
                }
            } catch (error) {
                console.error('Failed to load team roles for terminal avatars', error);
                if (!cancelled) {
                    setTeamRoles([]);
                }
            }
        };

        const handleSettingsUpdated = () => {
            void loadTeamRoles();
        };

        void loadTeamRoles();
        window.addEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);

        return () => {
            cancelled = true;
            window.removeEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
        };
    }, []);

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
            cancelPendingFit();
            if (
                terminalRef.current &&
                xtermRef.current &&
                fitAddonRef.current &&
                terminalRef.current.offsetWidth > 0 &&
                terminalRef.current.offsetHeight > 0
            ) {
                fitRafRef.current = window.requestAnimationFrame(() => {
                    fitRafRef.current = null;
                    if (
                        !terminalRef.current ||
                        !xtermRef.current ||
                        !fitAddonRef.current ||
                        !terminalRef.current.isConnected ||
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

        fitTimeoutRef.current = window.setTimeout(() => {
            fitTimeoutRef.current = null;
            safeFit();
        }, 50);

        const resizeObserver = new ResizeObserver(() => {
            safeFit();
        });
        resizeObserver.observe(terminalRef.current);

        return () => {
            resizeObserver.disconnect();
            safeFitRef.current = () => undefined;
            cancelPendingFit();
            xtermRef.current = null;
            fitAddonRef.current = null;
            term.dispose();
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
        if (!isMinimized) {
            safeFitRef.current();
        }
    }, [isMinimized]);

    useEffect(() => {
        let cancelled = false;

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
                                    '\x1b[38;5;12m[vicara]\x1b[0m 進行中セッションを復元しました。\r\n',
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
            const nextUnlisteners: Array<() => void> = [];
            const register = async <T,>(
                eventName: string,
                handler: (event: { payload: T }) => void | Promise<void>,
            ) => {
                const unlisten = await listen<T>(eventName, handler);
                if (cancelled) {
                    unlisten();
                    return;
                }
                nextUnlisteners.push(unlisten);
            };

            await register<ActiveClaudeSession>('claude_cli_started', (event) => {
                claudeStreamStateRef.current[event.payload.task_id] = createClaudeStreamRenderState();
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

            await register<ClaudeOutputPayload>('claude_cli_output', (event) => {
                const currentState =
                    claudeStreamStateRef.current[event.payload.task_id] ?? createClaudeStreamRenderState();
                const rendered = consumeClaudeStreamChunk(event.payload.output, currentState);
                claudeStreamStateRef.current[event.payload.task_id] = rendered.nextState;
                if (!rendered.output) {
                    return;
                }

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
                            logs: withHeader + rendered.output,
                        },
                    };
                });

                if (activeTaskIdRef.current === event.payload.task_id && xtermRef.current) {
                    xtermRef.current.write(rendered.output);
                }
            });

            await register<ClaudeExitPayload>('claude_cli_exit', async (event) => {
                const currentState =
                    claudeStreamStateRef.current[event.payload.task_id] ?? createClaudeStreamRenderState();
                const flushed = consumeClaudeStreamChunk('', currentState, true);
                delete claudeStreamStateRef.current[event.payload.task_id];
                const exitLine = createExitLine(event.payload.success, event.payload.reason);
                const nextStatus = mapExitStatus(event.payload.success, event.payload.reason);
                const appendedOutput = flushed.output + exitLine;

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
                            logs: withHeader + appendedOutput,
                        },
                    };
                });

                if (activeTaskIdRef.current === event.payload.task_id && xtermRef.current) {
                    xtermRef.current.write(appendedOutput);
                }

                if (event.payload.success) {
                    if (event.payload.new_status) {
                        await updateTaskStatus(
                            event.payload.task_id,
                            event.payload.new_status as Parameters<typeof updateTaskStatus>[1],
                        );
                        toast.success(
                            event.payload.new_status === 'Review'
                                ? '開発が完了しました。タスクをレビュー待ちに移動しました。'
                                : `開発が完了しました。ステータスを ${event.payload.new_status} に更新しました。`,
                        );
                    }
                } else {
                    toast.error(`プロセス終了: ${event.payload.reason}`);
                }
            });

            if (cancelled) {
                nextUnlisteners.forEach((unlisten) => unlisten());
                return;
            }

            return nextUnlisteners;
        };

        const handleFrontendError = (e: Event) => {
            const ce = e as CustomEvent;
            const errorDetail = normalizeFrontendErrorDetail(ce.detail, activeTaskIdRef.current);
            const message = `\r\n\x1b[31m[Invoke Error] ${errorDetail.message}\x1b[0m\r\n`;
            const targetTaskId = errorDetail.taskId;

            if (targetTaskId) {
                setSessions((prev) => {
                    const existing = prev[targetTaskId] ?? createSessionFromFrontendError(errorDetail);
                    const withHeader = existing.logs ? existing.logs : buildSessionHeader(existing);
                    return {
                        ...prev,
                        [targetTaskId]: {
                            ...existing,
                            taskTitle: errorDetail.taskTitle?.trim() || existing.taskTitle,
                            roleName: errorDetail.roleName?.trim() || existing.roleName,
                            model: errorDetail.model?.trim() || existing.model,
                            status: 'Failed',
                            exitReason: errorDetail.message,
                            logs: withHeader + message,
                        },
                    };
                });
                setActiveTaskId(targetTaskId);
            }

            if (xtermRef.current && targetTaskId && activeTaskIdRef.current === targetTaskId) {
                xtermRef.current.write(message);
                return;
            }

            if (xtermRef.current && !targetTaskId) {
                xtermRef.current.write(message);
            }
        };

        window.addEventListener('claude_error', handleFrontendError);
        restoreSessions();
        const unlistenPromise = setupListeners();

        return () => {
            cancelled = true;
            void unlistenPromise.then((unlisteners) => {
                unlisteners?.forEach((unlisten) => unlisten());
            });
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

    const handleDismissSession = (taskId: string) => {
        setSessions((prev) => {
            const nextActiveTaskId = getNextActiveTaskId(prev, taskId, activeTaskIdRef.current);
            const next = { ...prev };
            delete next[taskId];
            setActiveTaskId(nextActiveTaskId);
            return next;
        });
    };

    const handleClearCompletedSessions = () => {
        setSessions((prev) => {
            const remainingEntries = Object.entries(prev).filter(([, session]) =>
                isSessionRunning(session.status),
            );
            const next = Object.fromEntries(remainingEntries);
            const nextActiveTaskId =
                activeTaskIdRef.current && next[activeTaskIdRef.current]
                    ? activeTaskIdRef.current
                    : remainingEntries.length > 0
                      ? remainingEntries[remainingEntries.length - 1][0]
                      : null;
            setActiveTaskId(nextActiveTaskId);
            return next;
        });
    };

    return (
        <div className="relative flex h-full min-h-0 w-full flex-col">
            <div className="flex h-10 items-stretch border-b border-zinc-950 bg-[#18181b]">
                <div className="min-w-0 flex-1 overflow-hidden">
                    {sortedSessions.length === 0 ? (
                        <div className="flex h-full items-center px-2 text-xs text-zinc-500">
                            <span className="inline-flex min-w-0 items-center gap-2 truncate rounded-sm px-2 py-1">
                                <SquareTerminal size={13} className="shrink-0 text-zinc-500" />
                                <span className="truncate">実行中または履歴表示中のエージェントはありません</span>
                            </span>
                        </div>
                    ) : (
                        <div
                            className="flex h-full items-end gap-px overflow-x-auto overflow-y-hidden px-1 pb-1"
                            style={{ scrollbarGutter: 'stable both-edges' }}
                        >
                            {sortedSessions.map((session) => {
                                const isActive = session.taskId === activeTaskId;
                                const showInlineKill = isActive && canKillActiveSession;
                                const showDismiss = !isSessionRunning(session.status);
                                return (
                                    <div key={session.taskId} className="group relative min-w-[180px] max-w-[300px] shrink-0">
                                        <button
                                            type="button"
                                            onClick={() => handleSelectTab(session.taskId)}
                                            className={`flex h-8 w-full items-center gap-2 rounded-t-sm border border-b-0 px-2 py-1 text-left text-xs transition-colors ${
                                                isActive
                                                    ? 'border-zinc-700 bg-[#1e1e1e] text-zinc-100'
                                                    : 'border-transparent bg-[#23232a] text-zinc-400 hover:bg-[#2a2a33] hover:text-zinc-200'
                                            } ${showInlineKill || showDismiss ? 'pr-7' : ''}`}
                                            title={`${session.roleName} / ${session.taskTitle}`}
                                        >
                                            <StatusIndicator status={session.status} />
                                            <span className="min-w-0 flex-1 truncate font-medium">
                                                [{session.roleName}] {session.taskTitle}
                                            </span>
                                        </button>

                                        {showInlineKill && (
                                            <button
                                                type="button"
                                                onClick={(e) => {
                                                    e.stopPropagation();
                                                    void handleKill();
                                                }}
                                                className="absolute right-1 top-1/2 inline-flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded-sm text-red-400 opacity-50 transition-all hover:bg-red-500/10 hover:text-red-300 hover:opacity-100 focus:opacity-100 focus:outline-none group-hover:opacity-100"
                                                title="現在表示中の Claude プロセスを強制停止します"
                                            >
                                                <StopCircle size={12} />
                                            </button>
                                        )}

                                        {showDismiss && (
                                            <button
                                                type="button"
                                                onClick={(e) => {
                                                    e.stopPropagation();
                                                    handleDismissSession(session.taskId);
                                                }}
                                                className="absolute right-1 top-1/2 inline-flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded-sm text-zinc-400 opacity-50 transition-all hover:bg-white/10 hover:text-zinc-100 hover:opacity-100 focus:opacity-100 focus:outline-none group-hover:opacity-100"
                                                title="このセッション履歴を閉じる"
                                            >
                                                <X size={12} />
                                            </button>
                                        )}
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </div>

                {completedSessionCount > 0 && (
                    <button
                        type="button"
                        onClick={handleClearCompletedSessions}
                        className="inline-flex h-full shrink-0 items-center gap-1 border-l border-zinc-800 px-3 text-xs text-zinc-400 transition-colors hover:bg-white/5 hover:text-zinc-100"
                        title="完了・失敗・停止済みのセッション履歴をまとめて閉じます"
                    >
                        <X size={13} />
                        <span className="hidden sm:inline">完了分を閉じる</span>
                    </button>
                )}

                <button
                    type="button"
                    onClick={onToggleMinimize}
                    className="inline-flex h-full w-9 shrink-0 items-center justify-center border-l border-zinc-800 text-zinc-500 transition-colors hover:bg-white/5 hover:text-zinc-200"
                    title={isMinimized ? 'ターミナルを展開' : 'ターミナルを折りたたむ'}
                >
                    {isMinimized ? <ChevronUp size={15} /> : <ChevronDown size={15} />}
                </button>
            </div>

            <div className={`relative min-h-0 flex-1 overflow-hidden bg-[#1e1e1e] ${isMinimized ? 'hidden' : 'block'}`}>
                <div ref={terminalRef} className="h-full w-full overflow-hidden" />
                {activeSession && activeSessionAvatar && (
                    <div className="pointer-events-none absolute bottom-3 right-3 flex items-end gap-3">
                        <div className="rounded-2xl border border-sky-400/20 bg-zinc-950/74 px-4 py-2.5 text-right shadow-[0_18px_45px_-25px_rgba(56,189,248,0.45)] backdrop-blur-sm">
                            <div className="text-sm font-semibold leading-none text-sky-100">
                                {activeSession.roleName}
                            </div>
                            <div className="mt-1 max-w-[300px] truncate text-xs text-zinc-400">
                                {activeSession.taskTitle}
                            </div>
                        </div>
                        <div className="relative">
                            <div className="absolute inset-0 rounded-full bg-sky-400/20 blur-2xl" />
                            <Avatar
                                kind={activeSessionAvatar.kind}
                                size="xl"
                                alt={activeSession.roleName}
                                imageSrc={activeSessionRole?.avatar_image ?? null}
                                className="relative h-28 w-28 shadow-[0_18px_42px_-20px_rgba(56,189,248,0.75)]"
                            />
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
};

