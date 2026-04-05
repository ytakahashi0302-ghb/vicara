import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useWorkspace } from '../context/WorkspaceContext';

export interface ExecutionResult {
  exit_code: number;
  stdout: string;
  stderr: string;
}

export function usePtySession() {
    const { currentProjectId, projects } = useWorkspace();
    const [sessionId, setSessionId] = useState<string | null>(null);
    const isInitializing = useRef(false);

    useEffect(() => {
        const project = projects.find(p => p.id === currentProjectId);
        // Fallback to current working directory if no project path is set for dummy testing
        const cwd = project?.local_path || '.';

        let activeSessionId: string | null = null;
        let isMounted = true;

        const initPty = async () => {
            if (isInitializing.current) return;
            isInitializing.current = true;
            try {
                const sid = await invoke<string>('pty_spawn', { cwd });
                if (isMounted) {
                    setSessionId(sid);
                    activeSessionId = sid;
                } else {
                    // React Strict Mode double-render safety:
                    // If unmounted before we could set it, kill it immediately
                    await invoke('pty_kill', { sessionId: sid }).catch(console.error);
                }
            } catch (error) {
                console.error('Failed to spawn PTY session:', error);
            } finally {
                isInitializing.current = false;
            }
        };

        // Delay slightly to handle React Strict Mode synchronous mount/unmount/mount
        const timeoutId = setTimeout(initPty, 50);

        return () => {
            isMounted = false;
            clearTimeout(timeoutId);
            if (activeSessionId) {
                invoke('pty_kill', { sessionId: activeSessionId }).catch(console.error);
            }
        };
    }, [currentProjectId, projects]);

    const executeCommand = async (command: string): Promise<ExecutionResult> => {
        if (!sessionId) {
            throw new Error('PTY session is not initialized');
        }
        return await invoke<ExecutionResult>('pty_execute', {
            sessionId: sessionId,
            command
        });
    };

    return { sessionId, executeCommand };
}
