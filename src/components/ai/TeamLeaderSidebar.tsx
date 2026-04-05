import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TeamChatMessage } from '../../types';
import { useWorkspace } from '../../context/WorkspaceContext';
import { Send, Bot, User, Loader2, X, Trash2, Sparkles } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import toast from 'react-hot-toast';
import { v4 as uuidv4 } from 'uuid';

interface TeamLeaderSidebarProps {
    isOpen: boolean;
    onClose: () => void;
}

export const TeamLeaderSidebar: React.FC<TeamLeaderSidebarProps> = ({ isOpen, onClose }) => {
    const { currentProjectId, projects } = useWorkspace();
    const [messages, setMessages] = useState<TeamChatMessage[]>([]);
    const [input, setInput] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const textareaRef = useRef<HTMLTextAreaElement>(null);

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
        if (isOpen) {
            setTimeout(() => textareaRef.current?.focus(), 300);
        }
    }, [isOpen]);

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

            // 2. Call AI Team Leader
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
            console.error('AI Team Leader chat failed:', error);
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

    return (
        <div
            className={`flex flex-col h-full w-full bg-white transition-opacity duration-300 ease-in-out overflow-hidden border-none ${
                isOpen ? 'opacity-100' : 'opacity-0 hidden'
            }`}
        >
            {isOpen && (
                <>
                    {/* Header */}
                    <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 bg-gradient-to-r from-indigo-50 to-blue-50 shrink-0">
                        <div className="flex items-center gap-2.5">
                            <div className="p-1.5 bg-indigo-100 rounded-lg">
                                <Sparkles size={18} className="text-indigo-600" />
                            </div>
                            <div>
                                <h2 className="text-sm font-bold text-gray-800 leading-tight">AI Team Leader</h2>
                                <p className="text-[10px] text-gray-500 leading-tight">Scrum Master & Lead Engineer</p>
                            </div>
                        </div>
                        <div className="flex items-center gap-1">
                            {messages.length > 0 && (
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

                    {/* Chat History */}
                    <div className="flex-1 overflow-y-auto px-3 py-4 space-y-3 bg-gray-50/50">
                        {messages.length === 0 && !isLoading && (
                            <div className="flex flex-col items-center justify-center h-full text-center px-6 py-12">
                                <div className="p-4 bg-indigo-50 rounded-2xl mb-4">
                                    <Bot size={32} className="text-indigo-400" />
                                </div>
                                <p className="text-sm font-medium text-gray-600 mb-2">
                                    AI Team Leader
                                </p>
                                <p className="text-xs text-gray-400 leading-relaxed">
                                    プロジェクト全体を俯瞰したアドバイスを提供します。
                                    バックログの優先順位、スプリントの進捗、技術的な相談など、何でもお気軽にどうぞ。
                                </p>
                            </div>
                        )}

                        {messages.map((msg) => (
                            <div key={msg.id} className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}>
                                <div className={`flex gap-2 max-w-[90%] ${msg.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}>
                                    <div className={`shrink-0 h-7 w-7 rounded-full flex items-center justify-center mt-0.5 ${
                                        msg.role === 'user'
                                            ? 'bg-indigo-100 text-indigo-600'
                                            : 'bg-gradient-to-br from-indigo-100 to-blue-100 text-indigo-600'
                                    }`}>
                                        {msg.role === 'user' ? <User size={14} /> : <Bot size={14} />}
                                    </div>
                                    <div className={`rounded-2xl px-3.5 py-2.5 text-[13px] leading-relaxed ${
                                        msg.role === 'user'
                                            ? 'bg-indigo-600 text-white rounded-tr-md'
                                            : 'bg-white border border-gray-200 text-gray-800 shadow-sm rounded-tl-md'
                                    }`}>
                                        {msg.role === 'user' ? (
                                            <span className="whitespace-pre-wrap">{msg.content}</span>
                                        ) : (
                                            <div className="prose prose-sm max-w-none prose-p:my-1.5 prose-ul:my-1 prose-ol:my-1 prose-li:my-0.5 prose-code:text-[12px] prose-code:bg-gray-100 prose-code:px-1 prose-code:py-0.5 prose-code:rounded prose-pre:bg-gray-900 prose-pre:text-gray-100 prose-headings:text-sm prose-headings:mt-3 prose-headings:mb-1">
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
                                <div className="flex gap-2 max-w-[90%]">
                                    <div className="shrink-0 h-7 w-7 rounded-full flex items-center justify-center bg-gradient-to-br from-indigo-100 to-blue-100 text-indigo-600 mt-0.5">
                                        <Bot size={14} />
                                    </div>
                                    <div className="rounded-2xl rounded-tl-md px-4 py-3 bg-white border border-gray-200 shadow-sm flex items-center gap-2">
                                        <Loader2 size={14} className="animate-spin text-indigo-500" />
                                        <span className="text-xs text-gray-400">分析中...</span>
                                    </div>
                                </div>
                            </div>
                        )}

                        <div ref={messagesEndRef} />
                    </div>

                    {/* Input Area */}
                    <div className="p-3 bg-white border-t border-gray-200 shrink-0">
                        <form onSubmit={handleSend} className="relative">
                            <textarea
                                ref={textareaRef}
                                value={input}
                                onChange={(e) => setInput(e.target.value)}
                                onKeyDown={handleKeyDown}
                                placeholder="メッセージを入力... (Ctrl+Enter で送信)"
                                className="w-full pl-3 pr-11 py-2.5 rounded-xl border border-gray-300 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500 resize-none min-h-[44px] max-h-[120px] text-[13px] bg-gray-50 placeholder:text-gray-400 transition-colors"
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
                                className="absolute right-2 bottom-2 p-1.5 bg-indigo-600 text-white rounded-lg hover:bg-indigo-700 disabled:opacity-40 disabled:hover:bg-indigo-600 transition-colors"
                            >
                                <Send size={14} />
                            </button>
                        </form>
                    </div>
                </>
            )}
        </div>
    );
};
