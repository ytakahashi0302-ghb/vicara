import { useState, useEffect, useRef } from 'react';
import { useWorkspace } from '../../context/WorkspaceContext';
import { invoke } from '@tauri-apps/api/core';
import { load } from '@tauri-apps/plugin-store';
import toast from 'react-hot-toast';
import { ScaffoldingPanel } from './ScaffoldingPanel';
import { Avatar } from '../ai/Avatar';
import { usePoAssistantAvatarImage } from '../../hooks/usePoAssistantAvatarImage';
import { getAvatarDefinition, resolveAvatarImageSource } from '../ai/avatarRegistry';
import { AiQuickSwitcher } from '../ui/settings/AiQuickSwitcher';

interface ChatMessage {
    role: 'user' | 'assistant';
    content: string;
}

type InceptionTab = 'CONTEXT' | 'ARCHITECTURE' | 'RULE';
type InceptionFilename = 'PRODUCT_CONTEXT.md' | 'ARCHITECTURE.md' | 'Rule.md';

const PHASE_GUIDE_MESSAGES: Record<number, string> = {
    1: "Phase 1 を始めます。\nこのアプリで、誰のどんな困りごとを解決したいですか？ まだ曖昧なら、思い浮かんでいる利用者像からで大丈夫です。",
    2: "次のフェーズへ進みます。\n今回は『やること』ではなく『今回はやらないこと』を決めます。最初のリリースから外したい機能や、避けたい方向性はありますか？",
    3: "次のフェーズへ進みます。\nこのアプリは、まずどんな環境で使えればよさそうですか？ たとえば PC ブラウザ中心か、スマホでも使いたいか、最初はローカルだけでよいか、などを教えてください。",
    4: "次のフェーズへ進みます。\n開発を進めるときに守りたい約束ごとはありますか？ たとえば品質、セキュリティ、AI への指示などです。",
    5: "次のフェーズへ進みます。\nここからは最初の土台づくりを始めます。",
};

function getPhaseGuideMessage(phase: number, mode: 'advance' | 'resume' = 'advance') {
    if (mode === 'advance') {
        return PHASE_GUIDE_MESSAGES[phase] ?? PHASE_GUIDE_MESSAGES[1];
    }

    if (phase === 1) {
        return "Phase 1 に戻りました。\nこのアプリで、誰のどんな困りごとを解決したいかを整理し直しましょう。";
    }
    if (phase === 2) {
        return "Phase 2 に戻りました。\n今回はやらないことや、最初のリリースから外す内容を見直しましょう。";
    }
    if (phase === 3) {
        return "Phase 3 に戻りました。\nまずどんな環境で使えればよいか、ローカル中心でよいか、あとでクラウド対応したいかを見直しましょう。";
    }
    if (phase === 4) {
        return "Phase 4 に戻りました。\n開発で守りたいルールや AI への追加指示を見直しましょう。";
    }
    return "Phase 5 に移動しました。\n最初の土台づくりを進めます。";
}

function detectPhaseMarker(content: string): number | null {
    const match = content.match(/Phase\s*([1-5])(?:\s*\/\s*5)?/i);
    if (!match) {
        return null;
    }

    const phase = Number.parseInt(match[1], 10);
    return Number.isFinite(phase) ? phase : null;
}

function getMessagesForPhase(messages: ChatMessage[], targetPhase: number) {
    let phaseCursor = 1;

    return messages.filter((message) => {
        if (message.role === 'assistant') {
            const detectedPhase = detectPhaseMarker(message.content);
            if (detectedPhase !== null) {
                phaseCursor = detectedPhase;
            }
        }

        return phaseCursor === targetPhase;
    });
}

function normalizeInceptionFilename(filename: string): InceptionFilename | null {
    const normalized = filename.trim().toLowerCase();

    if (normalized === 'product_context.md') {
        return 'PRODUCT_CONTEXT.md';
    }
    if (normalized === 'architecture.md') {
        return 'ARCHITECTURE.md';
    }
    if (normalized === 'rule.md') {
        return 'Rule.md';
    }
    return null;
}

function getTabForFilename(filename: InceptionFilename): InceptionTab {
    if (filename === 'PRODUCT_CONTEXT.md') {
        return 'CONTEXT';
    }
    if (filename === 'ARCHITECTURE.md') {
        return 'ARCHITECTURE';
    }
    return 'RULE';
}

function looksLikeDocumentCompletionClaim(reply: string) {
    return /(PRODUCT_CONTEXT|ARCHITECTURE|Rule)\.md.+(生成|更新)しました/.test(reply);
}

export function InceptionDeck() {
    const { projects, currentProjectId } = useWorkspace();
    const currentProject = projects.find(p => p.id === currentProjectId);
    const poAssistantAvatarImage = usePoAssistantAvatarImage();
    const poAssistantFigure = getAvatarDefinition('po-assistant');
    const poAssistantFigureSrc = resolveAvatarImageSource(poAssistantAvatarImage) ?? poAssistantFigure.src;
    
    // Phase Management
    const [currentPhase, setCurrentPhase] = useState<number>(1);
    
    // Tab Management
    const [activeTab, setActiveTab] = useState<InceptionTab>('CONTEXT');
    
    // Chat and File State
    const [messages, setMessages] = useState<ChatMessage[]>([]);
    const [inputText, setInputText] = useState('');
    const [isProcessing, setIsProcessing] = useState(false);
    const [isFigureHidden, setIsFigureHidden] = useState(false);
    const [fileContents, setFileContents] = useState({
        CONTEXT: '',
        ARCHITECTURE: '',
        RULE: ''
    });

    const messagesEndRef = useRef<HTMLDivElement>(null);
    const sendLockRef = useRef(false);

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

                let initialMessage = getPhaseGuideMessage(1);
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

    useEffect(() => {
        setIsFigureHidden(false);
    }, [poAssistantFigureSrc]);

    const navigateToPhase = (targetPhase: number) => {
        setCurrentPhase(targetPhase);
        setMessages((prev) => [...prev, { role: 'assistant', content: getPhaseGuideMessage(targetPhase, 'resume') }]);
    };

    const handleSendMessage = async () => {
        if (sendLockRef.current || isProcessing || !inputText.trim() || !currentProject?.local_path) {
            return;
        }

        sendLockRef.current = true;

        const userMsg: ChatMessage = { role: 'user', content: inputText };
        const requestMessages = [...getMessagesForPhase(messages, currentPhase), userMsg];
        setMessages((prev) => [...prev, userMsg]);
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
                messagesHistory: requestMessages,
            });

            setMessages((prev) => [...prev, { role: 'assistant', content: response.reply }]);

            let didWritePatch = false;

            // パッチがある場合はファイルに書き込み
            if (response.is_finished && response.patch_target && response.patch_content) {
                const filename = normalizeInceptionFilename(response.patch_target);

                if (!filename) {
                    toast.error(`AI が未知のファイル名を返しました: ${response.patch_target}`);
                } else {
                    // Phase 2 (PRODUCT_CONTEXT.md追記) / Phase 4 (Rule.md追記) は append=true
                    const append = currentPhase === 2 || currentPhase === 4;

                    await invoke('write_inception_file', {
                        localPath: currentProject.local_path,
                        filename,
                        content: response.patch_content,
                        append,
                    });

                    didWritePatch = true;
                    toast.success(`${filename} を更新しました`);

                    // fileContentsのStateを更新（追記 or 上書き）
                    const tabKey = getTabForFilename(filename);

                    if (append) {
                        // 追記: 既存内容の末尾に patch_content を結合
                        setFileContents(prev => ({
                            ...prev,
                            [tabKey]: prev[tabKey] ? `${prev[tabKey]}\n${response.patch_content}` : response.patch_content,
                        }));
                    } else {
                        // 上書き: ファイルから最新内容を再読み込み
                        const updatedContent = await invoke<string | null>('read_inception_file', {
                            localPath: currentProject.local_path,
                            filename,
                        });
                        if (updatedContent !== null) {
                            setFileContents(prev => ({
                                ...prev,
                                [tabKey]: updatedContent,
                            }));
                        }
                    }

                    setActiveTab(tabKey);
                }
            }

            // フェーズ遷移: is_finished なら次へ（パッチ有無に関わらず）
            if (response.is_finished && currentPhase < 5) {
                if (didWritePatch) {
                    const nextPhase = currentPhase + 1;
                    setCurrentPhase(nextPhase);
                    setMessages(prev => [...prev, { role: 'assistant', content: getPhaseGuideMessage(nextPhase, 'advance') }]);
                } else {
                    toast.error('AI は完了を宣言しましたが、更新ファイルの内容が返ってこなかったためフェーズを進めませんでした。');
                }
            } else if (!response.is_finished && looksLikeDocumentCompletionClaim(response.reply)) {
                toast.error('応答文では更新完了とされていますが、実際のファイル更新データを受け取れていません。もう一度送信してください。');
            }
        } catch (error) {
            console.error('Chat failed:', error);
            toast.error('AIとの通信に失敗しました');
        } finally {
            sendLockRef.current = false;
            setIsProcessing(false);
        }
    };

    if (!currentProject) {
        return <div className="p-8 text-center">ワークスペースを選択してください。</div>;
    }

    if (!currentProject.local_path) {
        return (
            <div className="flex h-full flex-col items-center justify-center p-8 text-center">
                <h2 className="text-xl font-bold mb-4 text-gray-800">Inception Deck</h2>
                <p className="text-gray-600 mb-4 bg-white p-6 rounded-lg shadow-sm border border-gray-200">
                    AIと対話を始める前に、ヘッダーのワークスペース設定から<br/>
                    このプロジェクトの<b>ローカルディレクトリ</b>を設定してください。
                </p>
            </div>
        );
    }

    return (
        <div className="flex h-full w-full overflow-hidden bg-white">
            {/* Left Pane: Chat / Wizard or Scaffolding Panel */}
            <div className="w-1/2 flex flex-col border-r border-gray-200">
                {currentPhase === 5 ? (
                    /* Phase 5: Scaffolding Panel */
                    <>
                        <div className="flex items-center justify-end border-b border-gray-200 bg-gradient-to-r from-white via-blue-50 to-emerald-50 px-4 py-3">
                            <div className="flex items-center gap-3 rounded-full border border-blue-100 bg-white/90 px-3 py-1.5 shadow-sm">
                                <div className="text-right">
                                    <div className="text-xs font-semibold uppercase tracking-[0.16em] text-blue-700">PO Assistant</div>
                                    <div className="text-sm font-medium text-slate-700">インセプションを伴走中</div>
                                </div>
                                <Avatar kind="po-assistant" size="sm" imageSrc={poAssistantAvatarImage} />
                            </div>
                        </div>
                        <div className="border-b border-gray-200 bg-white px-4 py-3">
                            <AiQuickSwitcher compact />
                        </div>
                        <ScaffoldingPanel
                            localPath={currentProject.local_path}
                            projectName={currentProject.name}
                        />
                        <div className="p-3 border-t border-gray-200 bg-white">
                            <button
                                onClick={() => navigateToPhase(4)}
                                className="text-sm text-gray-600 hover:text-gray-900 px-3 py-1 rounded hover:bg-gray-100"
                            >
                                ← 前のフェーズに戻る
                            </button>
                        </div>
                    </>
                ) : (
                    /* Phase 1-4: Chat Wizard */
                    <div className="relative flex min-h-0 flex-1 flex-col overflow-hidden">
                        <div className="p-4 border-b border-gray-200 bg-gray-50 flex items-center justify-between">
                            <div>
                                <h2 className="text-lg font-bold text-gray-800">AI Inception Deck</h2>
                                <p className="text-sm text-gray-500">スプリント0: プロジェクトの方向性をすり合わせる</p>
                            </div>
                            <div className="flex items-center gap-3">
                                <div className="flex items-center gap-3 rounded-full border border-blue-100 bg-white px-3 py-1.5 shadow-sm">
                                    <div className="text-right">
                                        <div className="text-[11px] font-semibold uppercase tracking-[0.16em] text-blue-700">PO Assistant</div>
                                        <div className="text-xs text-slate-500">価値の言語化をサポート</div>
                                    </div>
                                    <Avatar kind="po-assistant" size="sm" imageSrc={poAssistantAvatarImage} />
                                </div>
                                <div className="text-sm font-medium px-3 py-1 bg-blue-100 text-blue-800 rounded-full">
                                    Phase {currentPhase} / 5
                                </div>
                            </div>
                        </div>

                        <div className="border-b border-gray-200 bg-white px-4 py-3">
                            <AiQuickSwitcher compact />
                        </div>

                        <div className="relative min-h-0 flex-1 overflow-hidden bg-gradient-to-b from-white via-slate-50/60 to-blue-50/40">
                            <div className="relative z-10 flex h-full flex-col overflow-y-auto px-4 py-4 pr-6 xl:pr-[7.5rem]">
                                <div className="space-y-4">
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
                                </div>
                                <div ref={messagesEndRef} />
                            </div>
                        </div>

                        {!isFigureHidden && (
                            <div className="pointer-events-none absolute bottom-[104px] right-[-34px] z-[1] hidden xl:block">
                                <div className="absolute inset-x-6 bottom-10 top-14 rounded-full bg-emerald-300/14 blur-3xl" />
                                <img
                                    src={poAssistantFigureSrc}
                                    alt={poAssistantFigure.label}
                                    className="relative h-[365px] w-[210px] origin-bottom-right object-contain opacity-95 drop-shadow-[0_24px_30px_rgba(16,185,129,0.16)]"
                                    onError={() => setIsFigureHidden(true)}
                                />
                            </div>
                        )}

                        <div className="p-4 border-t border-gray-200 bg-white">
                            <div className="flex flex-col gap-2">
                                <textarea
                                    value={inputText}
                                    onChange={(e) => setInputText(e.target.value)}
                                    onKeyDown={(e) => {
                                        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                                            e.preventDefault();
                                            if (!sendLockRef.current && !isProcessing) {
                                                void handleSendMessage();
                                            }
                                        }
                                    }}
                                    disabled={isProcessing}
                                    className="w-full border border-gray-300 rounded-md p-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none resize-none h-20"
                                    placeholder="AIへの指示を入力... (Ctrl+Enterで送信)"
                                />
                                <div className="flex justify-between items-center">
                                    <span className="text-xs text-gray-400">Ctrl+Enterで送信</span>
                                    <button
                                        onClick={() => void handleSendMessage()}
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
                                    onClick={() => navigateToPhase(Math.max(1, currentPhase - 1))}
                                    className="text-sm text-gray-600 hover:text-gray-900 px-3 py-1 rounded hover:bg-gray-100 disabled:opacity-50"
                                >
                                    ← 前のフェーズ
                                </button>
                                <button
                                    onClick={() => navigateToPhase(Math.min(5, currentPhase + 1))}
                                    className="text-sm text-gray-600 hover:text-gray-900 px-3 py-1 rounded hover:bg-gray-100"
                                >
                                    次のフェーズへスキップ →
                                </button>
                            </div>
                        </div>
                    </div>
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
