import { useState, useEffect, useRef } from 'react';
import { useWorkspace } from '../../context/WorkspaceContext';
import { invoke } from '@tauri-apps/api/core';
import { load } from '@tauri-apps/plugin-store';
import toast from 'react-hot-toast';
import { ScaffoldingPanel } from './ScaffoldingPanel';

interface ChatMessage {
    role: 'user' | 'assistant';
    content: string;
}

export function InceptionDeck() {
    const { projects, currentProjectId } = useWorkspace();
    const currentProject = projects.find(p => p.id === currentProjectId);
    
    // Phase Management
    const [currentPhase, setCurrentPhase] = useState<number>(1);
    
    // Tab Management
    const [activeTab, setActiveTab] = useState<'CONTEXT' | 'ARCHITECTURE' | 'RULE'>('CONTEXT');
    
    // Chat and File State
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [inputText, setInputText] = useState('');
    const [isProcessing, setIsProcessing] = useState(false);
    const [fileContents, setFileContents] = useState({
        CONTEXT: '',
        ARCHITECTURE: '',
        RULE: ''
    });

    const messagesEndRef = useRef<HTMLDivElement>(null);

    // Initial Load & Base Rule Generation, plus Store Hydration
    useEffect(() => {
        const initDeck = async () => {
            if (!currentProject?.local_path) return;
            try {
                // Read files first to check for existing context
                const context = await invoke<string | null>('read_inception_file', { localPath: currentProject.local_path, filename: 'PRODUCT_CONTEXT.md' });
                const arch = await invoke<string | null>('read_inception_file', { localPath: currentProject.local_path, filename: 'ARCHITECTURE.md' });
                let rule = await invoke<string | null>('read_inception_file', { localPath: currentProject.local_path, filename: 'Rule.md' });

                const hasExistingFiles = !!(context || arch || rule);
                
                // If completely new (no files exist), generate the base rule
                if (!hasExistingFiles) {
                    await invoke('generate_base_rule', { localPath: currentProject.local_path });
                    // Read the newly generated rule file
                    rule = await invoke<string | null>('read_inception_file', { localPath: currentProject.local_path, filename: 'Rule.md' });
                }

                setFileContents({
                    CONTEXT: context || '',
                    ARCHITECTURE: arch || '',
                    RULE: rule || ''
                });

                let initialMessage = "Phase 1 を開始します。\nプロダクトのコア価値とターゲット (Why) について教えてください。";
                if (hasExistingFiles) {
                    initialMessage = "既存のファイルが見つかりました。\n右のプレビューを確認し、この内容をベースに修正を加えますか？それとも既存のまま次へ進みますか？\n" + initialMessage;
                }
                
                // Load saved chat state from store
                const store = await load('settings.json');
                const savedState = await store.get<{ messages: ChatMessage[], currentPhase: number }>(`inception-chat-${currentProject.id}`);
                
                if (savedState && savedState.messages && savedState.messages.length > 0) {
                    setMessages(savedState.messages);
                    if (savedState.currentPhase) setCurrentPhase(savedState.currentPhase);
                } else {
                    setMessages([{ role: 'assistant', content: initialMessage }]);
                }
            } catch (error) {
                console.error('Failed to init inception files:', error);
                toast.error('初期化に失敗しました');
            }
        };
        initDeck();
    }, [currentProject?.local_path]);

    // Phase sync tab logic
    useEffect(() => {
        // eslint-disable-next-line react-hooks/set-state-in-effect
        if (currentPhase === 1 || currentPhase === 2) {
            setActiveTab('CONTEXT');
        // eslint-disable-next-line react-hooks/set-state-in-effect
        } else if (currentPhase === 3) {
            setActiveTab('ARCHITECTURE');
        // eslint-disable-next-line react-hooks/set-state-in-effect
        } else if (currentPhase === 4) {
            setActiveTab('RULE');
        }
    }, [currentPhase]);

    // Save chat state and phase to store
    useEffect(() => {
        if (!currentProject || messages.length === 0) return;
        const saveState = async () => {
            try {
                const store = await load('settings.json');
                await store.set(`inception-chat-${currentProject.id}`, { messages, currentPhase });
                await store.save();
            } catch (error) {
                console.error('Failed to save inception state:', error);
            }
        };
        saveState();
    }, [messages, currentPhase, currentProject?.id]);

    // Auto-scroll chat
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    const handleSendMessage = async () => {
        if (!inputText.trim() || !currentProject?.local_path) return;

        const userMsg: ChatMessage = { role: 'user', content: inputText };
        const newMessages = [...messages, userMsg];
        setMessages(newMessages);
        setInputText('');
        setIsProcessing(true);

        try {
            const response = await invoke<{
                reply: string;
                is_finished: boolean;
                patch_target: string | null;
                patch_content: string | null;
            }>('chat_inception', {
                projectId: currentProject.id,
                phase: currentPhase,
                messagesHistory: newMessages,
            });

            setMessages([...newMessages, { role: 'assistant', content: response.reply }]);

            // パッチがある場合はファイルに書き込み
            if (response.is_finished && response.patch_target && response.patch_content) {
                const filename = response.patch_target; // AI が指定したファイル名を使用

                // Phase 2 (PRODUCT_CONTEXT.md追記) / Phase 4 (Rule.md追記) は append=true
                const append = currentPhase === 2 || currentPhase === 4;

                await invoke('write_inception_file', {
                    localPath: currentProject.local_path,
                    filename,
                    content: response.patch_content,
                    append,
                });

                toast.success(`${filename} を更新しました`);

                // fileContentsのStateを更新（追記 or 上書き）
                const tabKey = filename === 'PRODUCT_CONTEXT.md' ? 'CONTEXT'
                             : filename === 'ARCHITECTURE.md' ? 'ARCHITECTURE'
                             : 'RULE';

                if (append) {
                    // 追記: 既存内容の末尾に patch_content を結合
                    setFileContents(prev => ({
                        ...prev,
                        [tabKey]: prev[tabKey] + '\n' + response.patch_content,
                    }));
                } else {
                    // 上書き: ファイルから最新内容を再読み込み
                    const updatedContent = await invoke<string | null>('read_inception_file', {
                        localPath: currentProject.local_path,
                        filename,
                    });
                    if (updatedContent) {
                        setFileContents(prev => ({
                            ...prev,
                            [tabKey]: updatedContent,
                        }));
                    }
                }
            }

            // フェーズ遷移: is_finished なら次へ（パッチ有無に関わらず）
            if (response.is_finished && currentPhase < 5) {
                const nextPhase = currentPhase + 1;
                setCurrentPhase(nextPhase);

                const phaseStartMsg = "次のフェーズへ進みました。\n" +
                    (nextPhase === 2 ? "Phase 2 (Not List): やらないことリストについて決めていきましょう。" :
                     nextPhase === 3 ? "Phase 3 (What): 技術スタックとアーキテクチャの制約について教えてください。" :
                     nextPhase === 4 ? "Phase 4 (How): プロジェクト固有の開発ルールやAIへの追加ルールはありますか？" :
                     "Phase 5: 初期足場構築（Scaffolding）を開始します。");

                setMessages(prev => [...prev, { role: 'assistant', content: phaseStartMsg }]);
            }
        } catch (error) {
            console.error('Chat failed:', error);
            toast.error('AIとの通信に失敗しました');
        } finally {
            setIsProcessing(false);
        }
    };

    if (!currentProject) {
        return <div className="p-8 text-center">ワークスペースを選択してください。</div>;
    }

    if (!currentProject.local_path) {
        return (
            <div className="p-8 text-center flex items-center justify-center flex-col h-[calc(100vh-64px)]">
                <h2 className="text-xl font-bold mb-4 text-gray-800">Inception Deck</h2>
                <p className="text-gray-600 mb-4 bg-white p-6 rounded-lg shadow-sm border border-gray-200">
                    AIと対話を始める前に、ヘッダーの「フォルダ」アイコンから<br/>
                    このプロジェクトの<b>ローカルディレクトリ</b>を設定してください。
                </p>
            </div>
        );
    }

    return (
        <div className="flex h-[calc(100vh-64px)] w-full overflow-hidden bg-white">
            {/* Left Pane: Chat / Wizard or Scaffolding Panel */}
            <div className="w-1/2 flex flex-col border-r border-gray-200">
                {currentPhase === 5 ? (
                    /* Phase 5: Scaffolding Panel */
                    <>
                        <ScaffoldingPanel
                            localPath={currentProject.local_path}
                            projectName={currentProject.name}
                        />
                        <div className="p-3 border-t border-gray-200 bg-white">
                            <button
                                onClick={() => setCurrentPhase(4)}
                                className="text-sm text-gray-600 hover:text-gray-900 px-3 py-1 rounded hover:bg-gray-100"
                            >
                                ← 前のフェーズに戻る
                            </button>
                        </div>
                    </>
                ) : (
                    /* Phase 1-4: Chat Wizard */
                    <>
                        <div className="p-4 border-b border-gray-200 bg-gray-50 flex items-center justify-between">
                            <div>
                                <h2 className="text-lg font-bold text-gray-800">AI Inception Deck</h2>
                                <p className="text-sm text-gray-500">スプリント0: プロジェクトの方向性をすり合わせる</p>
                            </div>
                            <div className="text-sm font-medium px-3 py-1 bg-blue-100 text-blue-800 rounded-full">
                                Phase {currentPhase} / 5
                            </div>
                        </div>

                        <div className="flex-1 overflow-y-auto p-4 space-y-4">
                            {messages.map((msg, idx) => (
                                <div key={idx} className={`p-3 rounded-lg border max-w-[85%] ${
                                    msg.role === 'user'
                                        ? 'bg-white text-gray-800 border-gray-200 self-end ml-auto'
                                        : 'bg-blue-50 text-blue-900 border-blue-100 self-start'
                                }`}>
                                    <p className="text-sm whitespace-pre-wrap">{msg.content}</p>
                                </div>
                            ))}
                            {isProcessing && (
                                <div className="bg-blue-50 text-blue-900 p-3 rounded-lg border border-blue-100 max-w-[85%] self-start flex items-center gap-2">
                                    <span className="w-2 h-2 bg-blue-500 rounded-full animate-bounce"></span>
                                    <span className="w-2 h-2 bg-blue-500 rounded-full animate-bounce" style={{ animationDelay: '0.2s' }}></span>
                                    <span className="w-2 h-2 bg-blue-500 rounded-full animate-bounce" style={{ animationDelay: '0.4s' }}></span>
                                </div>
                            )}
                            <div ref={messagesEndRef} />
                        </div>

                        <div className="p-4 border-t border-gray-200 bg-white">
                            <div className="flex flex-col gap-2">
                                <textarea
                                    value={inputText}
                                    onChange={(e) => setInputText(e.target.value)}
                                    onKeyDown={(e) => {
                                        if (e.key === 'Enter' && e.metaKey) handleSendMessage();
                                    }}
                                    disabled={isProcessing}
                                    className="w-full border border-gray-300 rounded-md p-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none resize-none h-20"
                                    placeholder="AIへの指示を入力... (Cmd+Enterで送信)"
                                />
                                <div className="flex justify-between items-center">
                                    <span className="text-xs text-gray-400">Cmd+Enterで送信</span>
                                    <button
                                        onClick={handleSendMessage}
                                        disabled={isProcessing || !inputText.trim()}
                                        className="px-6 py-2 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700 transition disabled:opacity-50 disabled:cursor-not-allowed"
                                    >
                                        {isProcessing ? '処理中...' : '送信'}
                                    </button>
                                </div>
                            </div>
                            <div className="mt-4 flex justify-between items-center border-t border-gray-100 pt-3">
                                <button
                                    disabled={currentPhase === 1}
                                    onClick={() => setCurrentPhase(p => Math.max(1, p - 1))}
                                    className="text-sm text-gray-600 hover:text-gray-900 px-3 py-1 rounded hover:bg-gray-100 disabled:opacity-50"
                                >
                                    ← 前のフェーズ
                                </button>
                                <button
                                    onClick={() => setCurrentPhase(p => Math.min(5, p + 1))}
                                    className="text-sm text-gray-600 hover:text-gray-900 px-3 py-1 rounded hover:bg-gray-100"
                                >
                                    次のフェーズへスキップ →
                                </button>
                            </div>
                        </div>
                    </>
                )}
            </div>

            {/* Right Pane: Live Document / Tabs */}
            <div className="w-1/2 flex flex-col bg-gray-50">
                <div className="flex border-b border-gray-200 bg-white px-2 pt-2 gap-1 overflow-x-auto">
                    {(['CONTEXT', 'ARCHITECTURE', 'RULE'] as const).map((tab, idx) => {
                        const labels = ['PRODUCT_CONTEXT.md', 'ARCHITECTURE.md', 'Rule.md'];
                        return (
                            <button 
                                key={tab}
                                onClick={() => setActiveTab(tab)}
                                className={`px-4 py-2 text-sm font-medium rounded-t-md border-b-2 transition-colors ${
                                    activeTab === tab 
                                    ? 'border-blue-600 text-blue-600 bg-blue-50' 
                                    : 'border-transparent text-gray-500 hover:text-gray-700 hover:bg-gray-100'
                                }`}
                            >
                                {labels[idx]}
                            </button>
                        );
                    })}
                </div>
                
                <div className="flex-1 p-6 overflow-y-auto">
                    <div className="bg-white border border-gray-200 shadow-sm rounded-lg p-6 min-h-full font-mono text-sm text-gray-800 whitespace-pre-wrap">
                        {fileContents[activeTab] || (
                            <div className="text-gray-400 italic">
                                {activeTab === 'CONTEXT' ? 'PRODUCT_CONTEXT.md' : activeTab === 'ARCHITECTURE' ? 'ARCHITECTURE.md' : 'Rule.md'} 
                                の内容がここにプレビューされます...
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}
