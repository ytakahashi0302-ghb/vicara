import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import toast from 'react-hot-toast';
import { Hammer, Play, CheckCircle2, AlertTriangle, FolderTree, FileText, RefreshCw } from 'lucide-react';

// ---------------------------------------------------------------------------
// Types (mirrors Rust scaffolding.rs)
// ---------------------------------------------------------------------------

interface TechStackInfo {
    language: string | null;
    framework: string | null;
    meta_framework: string | null;
    raw_content: string;
}

interface CliScaffold {
    type: 'CliScaffold';
    command: string;
    args: string[];
}

interface AiGenerated {
    type: 'AiGenerated';
    prompt: string;
}

type ScaffoldStrategy = CliScaffold | AiGenerated;

interface TechStackDetection {
    tech_stack: TechStackInfo;
    strategy: ScaffoldStrategy;
}

interface ScaffoldStatus {
    has_agent_md: boolean;
    has_claude_settings: boolean;
    has_extra_files: boolean;
    extra_files: string[];
}

type ScaffoldingState = 'loading' | 'idle' | 'executing' | 'generating' | 'completed' | 'error';

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface ScaffoldingPanelProps {
    localPath: string;
    projectName: string;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function ScaffoldingPanel({ localPath, projectName }: ScaffoldingPanelProps) {
    const [state, setState] = useState<ScaffoldingState>('loading');
    const [detection, setDetection] = useState<TechStackDetection | null>(null);
    const [scaffoldStatus, setScaffoldStatus] = useState<ScaffoldStatus | null>(null);
    const [output, setOutput] = useState<string[]>([]);
    const [agentMdContent, setAgentMdContent] = useState<string>('');
    const [errorMessage, setErrorMessage] = useState<string>('');

    // 技術スタック検出 + 既存状態チェック
    const detectAndCheck = useCallback(async () => {
        setState('loading');
        setErrorMessage('');
        try {
            const [det, status] = await Promise.all([
                invoke<TechStackDetection>('detect_tech_stack', { localPath }),
                invoke<ScaffoldStatus>('check_scaffold_status', { localPath }),
            ]);
            setDetection(det);
            setScaffoldStatus(status);

            if (status.has_agent_md && status.has_claude_settings) {
                // 既にスキャフォールド済み
                const content = await invoke<string | null>('read_inception_file', {
                    localPath,
                    filename: 'AGENT.md',
                });
                if (content) setAgentMdContent(content);
                setState('completed');
            } else {
                setState('idle');
            }
        } catch (error) {
            setErrorMessage(String(error));
            setState('error');
        }
    }, [localPath]);

    useEffect(() => {
        const timeoutId = window.setTimeout(() => {
            void detectAndCheck();
        }, 0);

        return () => {
            window.clearTimeout(timeoutId);
        };
    }, [detectAndCheck]);

    // スキャフォールド完了後: AGENT.md + .claude/settings.json 生成
    const handlePostScaffold = useCallback(async () => {
        setState('generating');
        try {
            const [content] = await Promise.all([
                invoke<string>('generate_agent_md', { localPath, projectName }),
                invoke<void>('generate_claude_settings', { localPath }),
            ]);
            setAgentMdContent(content);
            setState('completed');
            toast.success('スキャフォールド完了！');
        } catch (error) {
            setErrorMessage(String(error));
            setState('error');
            toast.error('AGENT.md 生成に失敗しました');
        }
    }, [localPath, projectName]);

    // scaffold_output / scaffold_exit イベントリスナー
    useEffect(() => {
        let isDisposed = false;

        const setupListeners = async () => {
            const nextUnlisteners: UnlistenFn[] = [];
            const register = async <T,>(
                eventName: string,
                handler: (event: { payload: T }) => void | Promise<void>,
            ) => {
                const unlisten = await listen<T>(eventName, handler);
                if (isDisposed) {
                    unlisten();
                    return;
                }
                nextUnlisteners.push(unlisten);
            };

            await register<{ output: string }>('scaffold_output', (event) => {
                setOutput((prev) => [...prev, event.payload.output]);
            });

            await register<{ success: boolean; reason: string }>('scaffold_exit', (event) => {
                if (event.payload.success) {
                    // スキャフォールド成功 → AGENT.md + .claude/settings.json 生成へ
                    void handlePostScaffold();
                } else {
                    setErrorMessage(event.payload.reason);
                    setState('error');
                    toast.error(`スキャフォールド失敗: ${event.payload.reason}`);
                }
            });

            // CLI ベース AI スキャフォールド用（共通の agent_cli_* イベントを利用）
            await register<{ task_id: string; output: string }>('agent_cli_output', (event) => {
                if (event.payload.task_id.startsWith('scaffold-ai-')) {
                    setOutput((prev) => [...prev, event.payload.output]);
                }
            });

            await register<{ task_id: string; success: boolean; reason: string }>('agent_cli_exit', (event) => {
                if (event.payload.task_id.startsWith('scaffold-ai-')) {
                    if (event.payload.success) {
                        void handlePostScaffold();
                    } else {
                        setErrorMessage(event.payload.reason);
                        setState('error');
                        toast.error(`AI スキャフォールド失敗: ${event.payload.reason}`);
                    }
                }
            });

            if (isDisposed) {
                nextUnlisteners.forEach((unlisten) => unlisten());
                return;
            }

            return nextUnlisteners;
        };

        const unlistenPromise = setupListeners();
        return () => {
            isDisposed = true;
            void unlistenPromise.then((unlisteners) => {
                unlisteners?.forEach((unlisten) => unlisten());
            });
        };
    }, [handlePostScaffold]);

    // スキャフォールド実行
    const handleExecuteScaffold = async () => {
        if (!detection) return;

        // 既存ファイル警告
        if (scaffoldStatus?.has_extra_files) {
            const confirmed = window.confirm(
                `プロジェクトディレクトリに既存のファイルがあります:\n${scaffoldStatus.extra_files.join(', ')}\n\nスキャフォールドを続行しますか？`
            );
            if (!confirmed) return;
        }

        setState('executing');
        setOutput([]);
        setErrorMessage('');

        try {
            if (detection.strategy.type === 'CliScaffold') {
                await invoke<boolean>('execute_scaffold_cli', {
                    localPath,
                    command: detection.strategy.command,
                    args: detection.strategy.args,
                });
            } else {
                await invoke<void>('execute_scaffold_ai', {
                    localPath,
                    techStackInfo: detection.strategy.prompt,
                });
            }
        } catch (error) {
            setErrorMessage(String(error));
            setState('error');
            toast.error(`実行エラー: ${error}`);
        }
    };

    // AGENT.md + .claude/settings.json のみ生成（スキャフォールドなし）
    const handleGenerateContextOnly = async () => {
        setState('generating');
        setErrorMessage('');
        try {
            const [content] = await Promise.all([
                invoke<string>('generate_agent_md', { localPath, projectName }),
                invoke<void>('generate_claude_settings', { localPath }),
            ]);
            setAgentMdContent(content);
            setState('completed');
            toast.success('AGENT.md と .claude/settings.json を生成しました');
        } catch (error) {
            setErrorMessage(String(error));
            setState('error');
        }
    };

    // ---------------------------------------------------------------------------
    // Render
    // ---------------------------------------------------------------------------

    if (state === 'loading') {
        return (
            <div className="flex items-center justify-center h-full text-gray-500">
                <RefreshCw size={20} className="animate-spin mr-2" />
                技術スタックを検出中...
            </div>
        );
    }

    return (
        <div className="flex flex-col h-full">
            {/* ヘッダー */}
            <div className="p-4 border-b border-gray-200 bg-gray-50">
                <div className="flex items-center gap-2 mb-1">
                    <Hammer size={20} className="text-indigo-600" />
                    <h3 className="text-lg font-bold text-gray-800">Scaffolding</h3>
                    {state === 'completed' && (
                        <CheckCircle2 size={18} className="text-green-500" />
                    )}
                </div>
                <p className="text-sm text-gray-500">プロジェクトの初期構造とAIコンテキストを構築</p>
            </div>

            <div className="flex-1 overflow-y-auto p-4 space-y-4">
                {/* 技術スタック検出結果 */}
                {detection && (
                    <div className="bg-white border border-gray-200 rounded-lg p-4">
                        <h4 className="text-sm font-semibold text-gray-700 mb-2 flex items-center gap-1.5">
                            <FolderTree size={16} />
                            検出された技術スタック
                        </h4>
                        <div className="grid grid-cols-3 gap-2 text-sm">
                            <div>
                                <span className="text-gray-500">言語:</span>{' '}
                                <span className="font-medium">{detection.tech_stack.language ?? '未検出'}</span>
                            </div>
                            <div>
                                <span className="text-gray-500">FW:</span>{' '}
                                <span className="font-medium">{detection.tech_stack.framework ?? '未検出'}</span>
                            </div>
                            <div>
                                <span className="text-gray-500">Meta:</span>{' '}
                                <span className="font-medium">{detection.tech_stack.meta_framework ?? '未検出'}</span>
                            </div>
                        </div>

                        <div className="mt-3 p-2 bg-gray-50 rounded text-sm">
                            <span className="text-gray-500">戦略: </span>
                            {detection.strategy.type === 'CliScaffold' ? (
                                <code className="text-indigo-700 bg-indigo-50 px-1.5 py-0.5 rounded">
                                    {detection.strategy.command} {detection.strategy.args.join(' ')}
                                </code>
                            ) : (
                                <span className="text-amber-700">AI自動生成（POアシスタント設定を使用）</span>
                            )}
                        </div>
                    </div>
                )}

                {/* 既存ファイル警告 */}
                {scaffoldStatus?.has_extra_files && state === 'idle' && (
                    <div className="bg-amber-50 border border-amber-200 rounded-lg p-3 flex items-start gap-2">
                        <AlertTriangle size={18} className="text-amber-500 shrink-0 mt-0.5" />
                        <div className="text-sm text-amber-800">
                            <p className="font-medium">既存ファイルが検出されました</p>
                            <p className="text-amber-600 mt-1">
                                {scaffoldStatus.extra_files.slice(0, 5).join(', ')}
                                {scaffoldStatus.extra_files.length > 5 && ` 他 ${scaffoldStatus.extra_files.length - 5} 件`}
                            </p>
                        </div>
                    </div>
                )}

                {/* アクションボタン */}
                {state === 'idle' && (
                    <div className="flex gap-2">
                        <button
                            onClick={handleExecuteScaffold}
                            className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-indigo-600 text-white rounded-lg text-sm font-medium hover:bg-indigo-700 transition"
                        >
                            <Play size={16} />
                            足場を構築する
                        </button>
                        <button
                            onClick={handleGenerateContextOnly}
                            className="flex items-center gap-2 px-4 py-2.5 border border-gray-300 text-gray-700 rounded-lg text-sm font-medium hover:bg-gray-50 transition"
                            title="スキャフォールドをスキップし、AGENT.md と .claude/settings.json のみ生成"
                        >
                            <FileText size={16} />
                            コンテキストのみ
                        </button>
                    </div>
                )}

                {/* 実行中 / 出力表示 */}
                {(state === 'executing' || state === 'error' || (state === 'generating' && output.length > 0)) && output.length > 0 && (
                    <div className="bg-gray-900 text-green-400 rounded-lg p-4 font-mono text-xs max-h-60 overflow-y-auto">
                        {output.map((line, idx) => (
                            <div key={idx} className="whitespace-pre-wrap">{line}</div>
                        ))}
                        {state === 'executing' && (
                            <span className="inline-block w-2 h-4 bg-green-400 animate-pulse ml-0.5" />
                        )}
                    </div>
                )}

                {/* 生成中 */}
                {state === 'generating' && (
                    <div className="flex items-center gap-2 text-indigo-600 text-sm">
                        <RefreshCw size={16} className="animate-spin" />
                        AGENT.md と .claude/settings.json を生成中...
                    </div>
                )}

                {/* エラー */}
                {state === 'error' && (
                    <div className="bg-red-50 border border-red-200 rounded-lg p-4">
                        <p className="text-sm text-red-800 font-medium">エラーが発生しました</p>
                        <p className="text-sm text-red-600 mt-1">{errorMessage}</p>
                        <button
                            onClick={detectAndCheck}
                            className="mt-2 text-sm text-red-700 hover:text-red-900 underline"
                        >
                            再試行
                        </button>
                    </div>
                )}

                {/* 完了: AGENT.md プレビュー */}
                {state === 'completed' && agentMdContent && (
                    <div className="space-y-3">
                        <div className="bg-green-50 border border-green-200 rounded-lg p-3">
                            <p className="text-sm text-green-800 font-medium flex items-center gap-1.5">
                                <CheckCircle2 size={16} />
                                スキャフォールド完了
                            </p>
                            <p className="text-sm text-green-600 mt-1">
                                AGENT.md と .claude/settings.json が生成されました。
                            </p>
                        </div>

                        <div className="bg-white border border-gray-200 rounded-lg">
                            <div className="px-4 py-2 border-b border-gray-100 bg-gray-50 rounded-t-lg">
                                <span className="text-sm font-medium text-gray-700">AGENT.md プレビュー</span>
                            </div>
                            <pre className="p-4 text-xs text-gray-700 font-mono whitespace-pre-wrap overflow-y-auto max-h-80">
                                {agentMdContent}
                            </pre>
                        </div>

                        <button
                            onClick={() => {
                                setState('idle');
                                setAgentMdContent('');
                                setOutput([]);
                            }}
                            className="text-sm text-indigo-600 hover:text-indigo-800 flex items-center gap-1"
                        >
                            <RefreshCw size={14} />
                            再実行
                        </button>
                    </div>
                )}
            </div>
        </div>
    );
}
