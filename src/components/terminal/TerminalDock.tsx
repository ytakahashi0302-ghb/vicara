import React, { useEffect, useRef, useState } from 'react';
import { Terminal as XTerm } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import 'xterm/css/xterm.css';
import { usePtySession } from '../../hooks/usePtySession';

export const TerminalDock: React.FC = () => {
    const terminalRef = useRef<HTMLDivElement>(null);
    const xtermRef = useRef<XTerm | null>(null);
    const fitAddonRef = useRef<FitAddon | null>(null);
    const { sessionId, executeCommand } = usePtySession();
    const [hasExecutedDummy, setHasExecutedDummy] = useState(false);

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

    // React to Session Ready and Exec Dummy Command
    useEffect(() => {
        if (sessionId && xtermRef.current && !hasExecutedDummy) {
            setHasExecutedDummy(true);
            const term = xtermRef.current;
            
            term.writeln(`\x1b[32m✔ PTY Session established\x1b[0m (ID: ${sessionId.substring(0, 8)}...)`);
            term.writeln('\x1b[38;5;12m[MicroScrum AI]\x1b[0m Executing integration test: \x1b[33mecho Hello PTY\x1b[0m');
            
            // Allow stdout from Windows to interpret properly sometimes needing explicit shell depending on backend execution.
            // Backend pty_execute runs `cmd.exe /C command` on Windows
            executeCommand('echo Hello PTY')
                .then(result => {
                    term.write(result.stdout);
                    if (result.stderr) {
                        term.write('\x1b[31m' + result.stderr + '\x1b[0m');
                    }
                    if (result.exit_code !== 0) {
                        term.writeln(`\n\x1b[31m[Process exited with code ${result.exit_code}]\x1b[0m`);
                    } else {
                        term.writeln('\n\x1b[32m✔ Integration test completed successfully.\x1b[0m');
                    }
                })
                .catch(err => {
                    term.writeln(`\n\x1b[31mCommand Execution Error: ${err}\x1b[0m`);
                });
        }
    }, [sessionId, executeCommand, hasExecutedDummy]);

    return (
        <div ref={terminalRef} className="w-full h-full rounded overflow-hidden" />
    );
};
