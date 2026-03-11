import React, { useState, useRef, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../ui/Button';
import { Textarea } from '../ui/Textarea';
import { Loader2, Send, Lightbulb, X, Plus } from 'lucide-react';
import toast from 'react-hot-toast';
import { StoryFormData } from '../board/StoryFormModal';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useWorkspace } from '../../context/WorkspaceContext';

interface Message {
    role: 'user' | 'assistant';
    content: string;
}

interface StoryDraft {
    title: string;
    description: string;
    acceptance_criteria: string;
}

interface RefinedIdeaResponse {
    reply: string;
    story_draft?: StoryDraft;
}

interface IdeaRefinementDrawerProps {
    isOpen: boolean;
    onClose: () => void;
    onComplete: (data: Partial<StoryFormData>) => void;
}

export const IdeaRefinementDrawer: React.FC<IdeaRefinementDrawerProps> = ({ isOpen, onClose, onComplete }) => {
    const { currentProjectId } = useWorkspace();
    const [messages, setMessages] = useState<Message[]>([]);
    const [input, setInput] = useState('');
    const [draft, setDraft] = useState<StoryDraft | null>(null);
    const [isLoading, setIsLoading] = useState(false);
    const messagesEndRef = useRef<HTMLDivElement>(null);

    // Auto-scroll to bottom of messages
    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }, [messages]);

    // Reset when drawer opens
    useEffect(() => {
        if (isOpen && messages.length === 0) {
            setMessages([]);
            setInput('');
            setDraft(null);
            setIsLoading(false);
        }
    }, [isOpen, messages.length]);

    const handleSend = async () => {
        if (!input.trim() || isLoading) return;

        const newUserMessage: Message = { role: 'user', content: input.trim() };
        const newMessages = [...messages, newUserMessage];

        setMessages(newMessages);
        setInput('');
        setIsLoading(true);

        try {
            const previousContext = messages.length > 0 ? messages : null;

            const response = await invoke<RefinedIdeaResponse>('refine_idea', {
                ideaSeed: newUserMessage.content,
                previousContext: previousContext,
                projectId: currentProjectId
            });

            setMessages(prev => [...prev, { role: 'assistant', content: response.reply }]);
            if (response.story_draft) {
                setDraft(response.story_draft);
            }

        } catch (error) {
            console.error('Failed to refine idea:', error);
            toast.error(`エラーが発生しました: ${error}`);
        } finally {
            setIsLoading(false);
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            handleSend();
        }
    };

    const handleComplete = () => {
        if (!draft) {
            toast.error("Storyの草案がまだ作成されていません");
            return;
        }

        const initialData: Partial<StoryFormData> = {
            title: draft.title || '',
            description: draft.description || '',
            acceptance_criteria: draft.acceptance_criteria || ''
        };

        onComplete(initialData);
    };

    if (!isOpen) return null;

    return (
        <div className="fixed inset-0 z-50 flex justify-end bg-black/20 transition-opacity">
            <div
                className="w-full max-w-5xl bg-white h-full shadow-2xl flex flex-col transform transition-transform duration-300 ease-in-out translate-x-0"
                onClick={(e) => e.stopPropagation()}
            >
                {/* Header */}
                <div className="flex items-center justify-between p-4 border-b bg-white">
                    <div className="flex items-center gap-2">
                        <Lightbulb className="text-yellow-500" size={24} />
                        <h2 className="text-xl font-semibold text-gray-800">AI 要件定義アシスタント</h2>
                    </div>
                    <button
                        onClick={onClose}
                        className="p-2 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-full transition-colors"
                    >
                        <X size={24} />
                    </button>
                </div>

                {/* 2-Pane Content */}
                <div className="flex-1 overflow-hidden flex flex-col md:flex-row bg-gray-50">

                    {/* Left Pane: Chat */}
                    <div className="flex-1 flex flex-col border-r border-gray-200 bg-white md:w-1/2">
                        <div className="flex-1 overflow-y-auto p-4">
                            {messages.length === 0 ? (
                                <div className="h-full flex flex-col items-center justify-center text-gray-400 space-y-4">
                                    <p className="text-center text-sm">
                                        実装したい機能や、解決したい課題を教えてください。<br />
                                        AIが壁打ち相手となり、右側のパネルに要件定義書をリアルタイムで作成します。
                                    </p>
                                </div>
                            ) : (
                                <div className="space-y-4">
                                    {messages.map((msg, idx) => (
                                        <div
                                            key={idx}
                                            className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
                                        >
                                            <div
                                                className={`max-w-[85%] rounded-2xl px-4 py-3 whitespace-pre-wrap text-sm ${msg.role === 'user'
                                                    ? 'bg-blue-600 text-white rounded-tr-none'
                                                    : 'bg-gray-100 text-gray-800 border border-gray-200 rounded-tl-none'
                                                    }`}
                                            >
                                                {msg.content}
                                            </div>
                                        </div>
                                    ))}
                                    {isLoading && (
                                        <div className="flex justify-start">
                                            <div className="bg-gray-100 border border-gray-200 rounded-2xl rounded-tl-none px-4 py-3 flex items-center space-x-2 text-gray-500">
                                                <Loader2 size={16} className="animate-spin" />
                                                <span className="text-sm">考え中...</span>
                                            </div>
                                        </div>
                                    )}
                                    <div ref={messagesEndRef} />
                                </div>
                            )}
                        </div>

                        {/* Input Area */}
                        <div className="p-4 border-t bg-white">
                            <div className="flex gap-2 items-end">
                                <div className="flex-1">
                                    <Textarea
                                        value={input}
                                        onChange={(e) => setInput(e.target.value)}
                                        onKeyDown={handleKeyDown}
                                        placeholder="メッセージを入力... (Shift+Enterで改行)"
                                        rows={2}
                                        disabled={isLoading}
                                        className="resize-none text-sm"
                                    />
                                </div>
                                <Button
                                    onClick={handleSend}
                                    disabled={!input.trim() || isLoading}
                                    className="mb-1 h-[42px] px-4"
                                >
                                    <Send size={18} />
                                </Button>
                            </div>
                        </div>
                    </div>

                    {/* Right Pane: Live Document */}
                    <div className="flex-1 flex flex-col bg-white md:w-1/2">
                        <div className="px-4 py-2 border-b bg-gray-50 flex justify-between items-center">
                            <span className="text-sm font-semibold text-gray-600">📝 Live Document (Story Draft)</span>
                        </div>
                        <div className="flex-1 overflow-y-auto p-6 text-sm text-gray-800 space-y-4">
                            {draft ? (
                                <div className="space-y-4">
                                    <div>
                                        <h3 className="text-xs font-bold text-gray-400 uppercase tracking-wider mb-1">Title</h3>
                                        <p className="font-semibold text-lg border-b pb-2">{draft.title}</p>
                                    </div>
                                    <div>
                                        <h3 className="text-xs font-bold text-gray-400 uppercase tracking-wider mb-1">Description</h3>
                                        <div className="prose prose-sm max-w-none text-gray-700 bg-white p-3 rounded-md border">
                                            <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                                {draft.description}
                                            </ReactMarkdown>
                                        </div>
                                    </div>
                                    <div>
                                        <h3 className="text-xs font-bold text-gray-400 uppercase tracking-wider mb-1">Acceptance Criteria</h3>
                                        <div className="prose prose-sm max-w-none text-gray-700 bg-white p-3 rounded-md border whitespace-pre-wrap">
                                            {draft.acceptance_criteria}
                                        </div>
                                    </div>
                                </div>
                            ) : (
                                <div className="h-full flex items-center justify-center text-gray-400">
                                    <p className="text-sm text-center">
                                        チャットを開始すると、ここに要件の草案が<br />自動的に生成されます。
                                    </p>
                                </div>
                            )}
                        </div>

                        {/* Action Area */}
                        <div className="p-4 border-t bg-gray-50 flex justify-end">
                            <Button
                                variant="primary"
                                onClick={handleComplete}
                                disabled={!draft || isLoading}
                            >
                                <Plus size={18} className="mr-2" />
                                この内容でStoryを作成
                            </Button>
                        </div>
                    </div>

                </div>
            </div>
        </div>
    );
};
