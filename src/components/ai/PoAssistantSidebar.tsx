import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TeamChatMessage } from '../../types';
import { useWorkspace } from '../../context/WorkspaceContext';
import { Send, User, Loader2, X, Trash2, MessageSquare, FileText, Info } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import toast from 'react-hot-toast';
import { v4 as uuidv4 } from 'uuid';
import { Avatar } from './Avatar';
import { getAvatarDefinition, PO_ASSISTANT_ROLE_NAME, resolveAvatarImageSource } from './avatarRegistry';
import { usePoAssistantAvatarImage } from '../../hooks/usePoAssistantAvatarImage';
import { AiQuickSwitcher } from '../ui/settings/AiQuickSwitcher';
import { NotesPanel } from './NotesPanel';

interface PoAssistantSidebarProps {
    isOpen: boolean;
    onClose: () => void;
}

export const PoAssistantSidebar: React.FC<PoAssistantSidebarProps> = ({ isOpen, onClose }) => {
    const { currentProjectId, projects } = useWorkspace();
    const poAssistantAvatarImage = usePoAssistantAvatarImage();
    const [messages, setMessages] = useState<TeamChatMessage[]>([]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [isFigureHidden, setIsFigureHidden] = useState(false);
    const [activeTab, setActiveTab] = useState<'chat' | 'notes'>('chat');
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const textareaRef = useRef<HTMLTextAreaElement>(null);
    const poAssistantFigure = getAvatarDefinition('po-assistant');
    const poAssistantFigureSrc = resolveAvatarImageSource(poAssistantAvatarImage) ?? poAssistantFigure.src;

    // Load chat history when project changes or panel opens
    useEffect(() => {
        if (isOpen && currentProjectId) {
            loadMessages();
        }
    }, [isOpen, currentProjectId]);

    // Auto-scroll to bottom (with slight delay for accurate DOM height calculation after markdown render)
    useEffect(() => {
        const scrollToBottom = () => {
            messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
        };
        // DOM update delay
        setTimeout(scrollToBottom, 50);
    }, [messages, isLoading]);

    // Focus textarea when panel opens
    useEffect(() => {
        if (isOpen && activeTab === 'chat') {
            setTimeout(() => textareaRef.current?.focus(), 300);
        }
    }, [activeTab, isOpen]);

    useEffect(() => {
        setIsFigureHidden(false);
    }, [isOpen, poAssistantFigureSrc]);

    const loadMessages = async () => {
        try {
            const data = await invoke<TeamChatMessage[]>('get_team_chat_messages', {
                projectId: currentProjectId
            });
            setMessages(data);
        } catch (error) {
            console.error('Failed to load team chat messages:', error);
        }
    };

    const handleSend = async (e?: React.FormEvent) => {
        e?.preventDefault();
        if (!input.trim() || isLoading) return;

        const currentProject = projects.find(p => p.id === currentProjectId);
        if (!currentProject?.local_path) {
            toast.error('AIチャットを利用するには、設定からプロジェクトのローカルパスを設定してください。');
            return;
        }

        const userContent = input.trim();
        setInput('');

        const userMsgId = uuidv4();
        const userMsg: TeamChatMessage = {
            id: userMsgId,
            project_id: currentProjectId,
            role: 'user',
            content: userContent,
            created_at: new Date().toISOString()
        };

        // Optimistic update
        setMessages(prev => [...prev, userMsg]);
        setIsLoading(true);

        try {
            // 1. Save user message
            await invoke('add_team_chat_message', {
                id: userMsgId,
                projectId: currentProjectId,
                role: 'user',
                content: userContent,
            });

            // 2. Call PO assistant
            const messagesForAI = [
                ...messages.map(m => ({
                    role: m.role,
                    content: m.content,
                })),
                { role: 'user', content: userContent }
            ];

            const aiResponse = await invoke<{ reply: string }>('chat_with_team_leader', {
                projectId: currentProjectId,
                messagesHistory: messagesForAI,
            });

            const replyContent = aiResponse.reply;

            // 3. Save AI response
            const aiMsgId = uuidv4();
            const aiMsg: TeamChatMessage = {
                id: aiMsgId,
                project_id: currentProjectId,
                role: 'assistant',
                content: replyContent,
                created_at: new Date().toISOString()
            };

            await invoke('add_team_chat_message', {
                id: aiMsgId,
                projectId: currentProjectId,
                role: 'assistant',
                content: replyContent,
            });

            setMessages(prev => [...prev, aiMsg]);
        } catch (error) {
            console.error('PO assistant chat failed:', error);
            toast.error(`推論に失敗しました: ${error}`);
        } finally {
            setIsLoading(false);
        }
    };

    const handleClearHistory = async () => {
        if (!window.confirm('チャット履歴を全て削除してもよろしいですか？')) return;
        try {
            await invoke('clear_team_chat_messages', { projectId: currentProjectId });
            setMessages([]);
            toast.success('チャット履歴を削除しました');
        } catch (error) {
            console.error('Failed to clear chat history:', error);
            toast.error('履歴の削除に失敗しました');
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
        e.stopPropagation();
        if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            handleSend();
        }
    };

    const tabButtonClass = (tab: 'chat' | 'notes') => (
        `inline-flex items-center gap-1.5 border-b-2 px-3 py-2 text-xs font-semibold transition-colors ${
            activeTab === tab
                ? 'border-indigo-500 text-indigo-600'
                : 'border-transparent text-gray-500 hover:border-gray-300 hover:text-gray-700'
        }`
    );

    return (
        <div
            className={`relative flex flex-col h-full w-full bg-white transition-opacity duration-300 ease-in-out overflow-hidden border-none ${
                isOpen ? 'opacity-100' : 'opacity-0 hidden'
            }`}
        >
            {isOpen && (
                <>
                    {/* Header */}
                    <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 bg-gradient-to-r from-indigo-50 to-blue-50 shrink-0">
                        <div className="flex items-center gap-2.5">
                            <Avatar kind="po-assistant" size="md" imageSrc={poAssistantAvatarImage} />
                            <div>
                                <div className="flex items-center gap-1">
                                    <h2 className="text-sm font-bold text-gray-800 leading-tight">{PO_ASSISTANT_ROLE_NAME}</h2>
                                    <div className="group relative">
                                        <Info size={12} className="text-gray-400 hover:text-indigo-500 cursor-help transition-colors" />
                                        <div className="pointer-events-none absolute left-1/2 top-full z-50 mt-1.5 w-64 -translate-x-1/2 rounded-lg border border-gray-200 bg-white p-3 text-[11px] leading-relaxed text-gray-600 shadow-lg opacity-0 group-hover:opacity-100 transition-opacity">
                                            <p className="mb-1.5 font-semibold text-gray-800">POアシスタントでできること</p>
                                            <ul className="space-y-1 list-none">
                                                <li>💬 バックログへのPBI・タスク追加</li>
                                                <li>📌 気づきをふせんとしてボードに記録</li>
                                                <li>🔄 レトロボードへのKPT提案（Keep / Problem / Try）</li>
                                                <li>🎯 優先順位づけや要件整理の相談</li>
                                            </ul>
                                        </div>
                                    </div>
                                </div>
                                <p className="text-[10px] text-gray-500 leading-tight">意思決定サポート・バックログ整理・レトロスペクティブ連携を担当</p>
                            </div>
                        </div>
                        <div className="flex items-center gap-1">
                            {activeTab === 'chat' && messages.length > 0 && (
                                <button
                                    onClick={handleClearHistory}
                                    className="p-1.5 text-gray-400 hover:text-red-500 hover:bg-red-50 rounded-md transition-colors"
                                    title="チャット履歴を削除"
                                >
                                    <Trash2 size={14} />
                                </button>
                            )}
                            <button
                                onClick={onClose}
                                className="p-1.5 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-md transition-colors"
                                title="パネルを閉じる"
                            >
                                <X size={16} />
                            </button>
                        </div>
                    </div>

                    <div className="hidden" aria-hidden="true">
                        <AiQuickSwitcher compact />
                    </div>

                    <div className="flex items-center gap-1 border-b border-gray-200 bg-white px-3 shrink-0">
                        <button
                            type="button"
                            onClick={() => setActiveTab('chat')}
                            className={tabButtonClass('chat')}
                        >
                            <MessageSquare size={14} />
                            チャット
                        </button>
                        <button
                            type="button"
                            onClick={() => setActiveTab('notes')}
                            className={tabButtonClass('notes')}
                        >
                            <FileText size={14} />
                            ふせん
                            <div className="group relative ml-0.5" onClick={(e) => e.stopPropagation()}>
                                <Info size={11} className="text-gray-400 hover:text-indigo-500 cursor-help transition-colors" />
                                <div className="pointer-events-none absolute left-1/2 top-full z-50 mt-1.5 w-56 -translate-x-1/2 rounded-lg border border-gray-200 bg-white p-3 text-[11px] leading-relaxed text-gray-600 shadow-lg opacity-0 group-hover:opacity-100 transition-opacity">
                                    <p className="mb-1.5 font-semibold text-gray-800">ふせんとは</p>
                                    <ul className="space-y-1 list-none">
                                        <li>📌 会話中の気づきやメモを記録</li>
                                        <li>🔄 レトロボードへKPTとして転記可能</li>
                                        <li>✏️ 自分でも自由に追加・編集できます</li>
                                    </ul>
                                </div>
                            </div>
                        </button>
                    </div>

                    <div className="relative flex-1 min-h-0">
                        <div className={`${activeTab === 'chat' ? 'flex h-full flex-col' : 'hidden'}`}>
                            {/* Chat History */}
                            <div className="relative flex-1 overflow-y-auto bg-gray-50/50">
                                <div className="relative z-10 space-y-3 px-3 py-4 pr-6 xl:pr-[7.5rem]">
                                    {messages.length === 0 && !isLoading && (
                                        <div className="flex h-full flex-col items-center justify-center px-6 py-12 text-center">
                                            <Avatar kind="po-assistant" size="lg" imageSrc={poAssistantAvatarImage} className="mb-4 shadow-sm" />
                                            <p className="mb-2 text-sm font-medium text-gray-600">
                                                {PO_ASSISTANT_ROLE_NAME}
                                            </p>
                                            <p className="text-xs leading-relaxed text-gray-400">
                                                プロジェクト全体を俯瞰しながら、優先順位づけや判断整理を支援します。
                                                バックログの優先順位、スプリントの進め方、要件の切り分けなどを気軽に相談してください。
                                            </p>
                                        </div>
                                    )}

                                    {messages.map((msg) => (
                                        <div key={msg.id} className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
                                            <div className={`flex gap-3 ${msg.role === 'user' ? 'max-w-[88%] flex-row-reverse' : 'max-w-full flex-row'} `}>
                                                {msg.role === 'user' ? (
                                                    <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-100 text-indigo-600">
                                                        <User size={14} />
                                                    </div>
                                                ) : (
                                                    <Avatar kind="po-assistant" size="md" imageSrc={poAssistantAvatarImage} className="mt-0.5 shadow-sm" />
                                                )}
                                                <div className={`rounded-2xl px-3.5 py-2.5 text-[13px] leading-relaxed ${
                                                    msg.role === 'user'
                                                        ? 'rounded-tr-md bg-indigo-600 text-white'
                                                        : 'rounded-tl-md border border-gray-200 bg-white text-gray-800 shadow-sm'
                                                }`}>
                                                    {msg.role === 'user' ? (
                                                        <span className="whitespace-pre-wrap">{msg.content}</span>
                                                    ) : (
                                                        <div className="prose prose-sm max-w-none prose-p:my-1.5 prose-ul:my-1 prose-ol:my-1 prose-li:my-0.5 prose-code:rounded prose-code:bg-gray-100 prose-code:px-1 prose-code:py-0.5 prose-code:text-[12px] prose-pre:bg-gray-900 prose-pre:text-gray-100 prose-headings:mt-3 prose-headings:mb-1 prose-headings:text-sm">
                                                            <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                                                {msg.content}
                                                            </ReactMarkdown>
                                                        </div>
                                                    )}
                                                </div>
                                            </div>
                                        </div>
                                    ))}

                                    {isLoading && (
                                        <div className="flex justify-start">
                                            <div className="flex max-w-full gap-3">
                                                <Avatar kind="po-assistant" size="md" imageSrc={poAssistantAvatarImage} className="mt-0.5 shadow-sm" />
                                                <div className="flex items-center gap-2 rounded-2xl rounded-tl-md border border-gray-200 bg-white px-4 py-3 shadow-sm">
                                                    <Loader2 size={14} className="animate-spin text-indigo-500" />
                                                    <span className="text-xs text-gray-400">判断材料を整理しています...</span>
                                                </div>
                                            </div>
                                        </div>
                                    )}

                                    <div ref={messagesEndRef} />
                                </div>
                            </div>

                            {/* Input Area */}
                            <div className="relative z-20 shrink-0 border-t border-gray-200 bg-white p-3">
                                <form onSubmit={handleSend} className="relative">
                                    <textarea
                                        ref={textareaRef}
                                        value={input}
                                        onChange={(e) => setInput(e.target.value)}
                                        onKeyDown={handleKeyDown}
                                        placeholder="メッセージを入力... (Ctrl+Enter で送信)"
                                        className="min-h-[44px] max-h-[120px] w-full resize-none rounded-xl border border-gray-300 bg-gray-50 py-2.5 pl-3 pr-11 text-[13px] placeholder:text-gray-400 transition-colors focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                                        disabled={isLoading}
                                        rows={1}
                                        onInput={(e) => {
                                            const target = e.target as HTMLTextAreaElement;
                                            target.style.height = 'auto';
                                            target.style.height = Math.min(target.scrollHeight, 120) + 'px';
                                        }}
                                    />
                                    <button
                                        type="submit"
                                        disabled={!input.trim() || isLoading}
                                        className="absolute bottom-2 right-2 rounded-lg bg-indigo-600 p-1.5 text-white transition-colors hover:bg-indigo-700 disabled:opacity-40 disabled:hover:bg-indigo-600"
                                    >
                                        <Send size={14} />
                                    </button>
                                </form>
                            </div>

                            {!isFigureHidden && (
                                <div className="pointer-events-none absolute bottom-[84px] right-[-34px] z-[1] hidden xl:block">
                                    <div className="absolute inset-x-6 bottom-10 top-14 rounded-full bg-emerald-300/14 blur-3xl" />
                                    <img
                                        src={poAssistantFigureSrc}
                                        alt={PO_ASSISTANT_ROLE_NAME}
                                        className="relative h-[365px] w-[210px] origin-bottom-right object-contain opacity-95 drop-shadow-[0_24px_30px_rgba(16,185,129,0.16)]"
                                        onError={() => setIsFigureHidden(true)}
                                    />
                                </div>
                            )}
                        </div>

                        <div className={`${activeTab === 'notes' ? 'flex h-full min-h-0 flex-col' : 'hidden'}`}>
                            <NotesPanel />
                        </div>
                    </div>
                </>
            )}
        </div>
    );
};
