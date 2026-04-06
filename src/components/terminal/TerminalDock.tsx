import React, { useEffect, useRef, useState } from 'react';
import { Terminal as XTerm } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import 'xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { useScrum } from '../../context/ScrumContext';
import { StopCircle } from 'lucide-react';
import toast from 'react-hot-toast';

export const TerminalDock: React.FC = () => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<XTerm | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const { updateTaskStatus } = useScrum();
    const [activeTaskId, setActiveTaskId] = useState<string | null>(null);

    useEffect(() => {
        if (!terminalRef.current) return;

        const term = new XTerm({
            theme: {
                background: '#1e1e1e', // VS Code default integration terminal background
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
            convertEol: true, // Handle \n vs \r\n properly
            disableStdin: true, // Read-Only (監視・可視化用) に強制
        });

        const safeFit = () => {
            if (
                terminalRef.current &&
                xtermRef.current &&
                terminalRef.current.offsetWidth > 0 &&
                terminalRef.current.offsetHeight > 0
            ) {
                requestAnimationFrame(() => {
                    // requestAnimationFrame後にもう一度要素の存在とサイズをチェック
                    if (!terminalRef.current || !xtermRef.current) return;
                    if (terminalRef.current.offsetWidth === 0 || terminalRef.current.offsetHeight === 0) return;
                    
                    try {
                        fitAddon.fit();
                    } catch(e) {
                        console.warn('xterm fit error ignored:', e);
                    }
                });
            }
        };

        const fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(terminalRef.current);
        
        // DOMのレイアウト完了を待ってから最初のfitを実行
        setTimeout(safeFit, 50);

        xtermRef.current = term;
        fitAddonRef.current = fitAddon;

        // Resize observer to auto-fit terminal on dock resize
        const resizeObserver = new ResizeObserver(() => {
            safeFit();
        });
        resizeObserver.observe(terminalRef.current);

        term.writeln('\x1b[38;5;12m[MicroScrum AI]\x1b[0m Dev Agent Terminal Initialization...');

        return () => {
            resizeObserver.disconnect();
            term.dispose();
            xtermRef.current = null;
            fitAddonRef.current = null;
        };
    }, []);
    // Listen to Claude events (cancelled flag pattern for StrictMode safety)
    useEffect(() => {
        let cancelled = false;
        let unlistenOutput: (() => void) | null = null;
        let unlistenExit: (() => void) | null = null;

        const setupListeners = async () => {
            const uo = await listen<{ task_id: string; output: string }>('claude_cli_output', (event) => {
                setActiveTaskId(event.payload.task_id);
                if (xtermRef.current) {
                    xtermRef.current.write(event.payload.output);
                }
            });
            if (cancelled) { uo(); return; }
            unlistenOutput = uo;

            const ue = await listen<{ task_id: string; success: boolean; reason: string }>('claude_cli_exit', async (event) => {
                if (xtermRef.current) {
                    const color = event.payload.success ? '\x1b[32m' : '\x1b[31m';
                    xtermRef.current.writeln(`\r\n${color}✔ Process Exited: ${event.payload.reason}\x1b[0m\r\n`);
                }
                setActiveTaskId(null);
                
                if (event.payload.success) {
                    await updateTaskStatus(event.payload.task_id, 'Done');
                    toast.success('開発が完了しました。レビューをお願いします。');
                } else {
                    toast.error(`プロセス終了: ${event.payload.reason}`);
                }
            });
            if (cancelled) { ue(); return; }
            unlistenExit = ue;
        };

        const handleFrontendError = (e: Event) => {
            const ce = e as CustomEvent;
            if (xtermRef.current) {
                xtermRef.current.writeln(`\r\n\x1b[31m[Invoke Error] ${ce.detail}\x1b[0m\r\n`);
            }
        };

        window.addEventListener('claude_error', handleFrontendError);
        setupListeners();

        return () => {
            cancelled = true;
            if (unlistenOutput) unlistenOutput();
            if (unlistenExit) unlistenExit();
            window.removeEventListener('claude_error', handleFrontendError);
        };
    }, [updateTaskStatus]);

    const handleKill = async () => {
        if (!activeTaskId) return;
        try {
            await invoke('kill_claude_process', { taskId: activeTaskId });
            toast.success('強制終了シグナルを送信しました');
        } catch (e: any) {
            toast.error(`Kill Error: ${e}`);
        }
    };

    return (
        <div className="relative w-full h-full">
            <div ref={terminalRef} className="w-full h-full rounded overflow-hidden" />
            {activeTaskId && (
                <button
                    onClick={handleKill}
                    className="absolute top-2 right-4 bg-red-600 hover:bg-red-500 text-white px-3 py-1.5 rounded-md flex items-center gap-2 text-sm shadow-lg font-medium transition-colors z-10"
                    title="実行中のClaudeプロセスを強制停止します"
                >
                    <StopCircle size={16} />
                    実行を強制停止
                </button>
            )}
        </div>
    );
};
