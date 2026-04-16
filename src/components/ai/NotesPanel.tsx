import React, { useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import {
    ChevronDown,
    ChevronUp,
    Loader2,
    Pencil,
    Plus,
    Trash2,
    UserRound,
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import toast from 'react-hot-toast';
import { useWorkspace } from '../../context/WorkspaceContext';
import { useProjectNotes } from '../../hooks/useProjectNotes';
import { useRetrospective } from '../../hooks/useRetrospective';
import { usePoAssistantAvatarImage } from '../../hooks/usePoAssistantAvatarImage';
import { ProjectNote, RetroCategory } from '../../types';
import { Avatar } from './Avatar';
import { PO_ASSISTANT_ROLE_NAME } from './avatarRegistry';
import { Button } from '../ui/Button';
import { Textarea } from '../ui/Textarea';

export const RETRO_ITEMS_UPDATED_EVENT = 'vicara:retro-items-updated';

const RETRO_CATEGORY_OPTIONS: Array<{
    value: RetroCategory;
    label: string;
    className: string;
    badgeClassName: string;
}> = [
    {
        value: 'keep',
        label: 'Keep',
        className: 'border-emerald-200 bg-emerald-50 text-emerald-800 hover:bg-emerald-100',
        badgeClassName: 'border-emerald-200 bg-emerald-50 text-emerald-700',
    },
    {
        value: 'problem',
        label: 'Problem',
        className: 'border-rose-200 bg-rose-50 text-rose-800 hover:bg-rose-100',
        badgeClassName: 'border-rose-200 bg-rose-50 text-rose-700',
    },
    {
        value: 'try',
        label: 'Try',
        className: 'border-sky-200 bg-sky-50 text-sky-800 hover:bg-sky-100',
        badgeClassName: 'border-sky-200 bg-sky-50 text-sky-700',
    },
];

function resolveStickyTitle(content: string) {
    const fallback = content
        .split(/\r?\n/)
        .map((line) => line.replace(/^[#>*+\-\s]+/, '').trim())
        .find(Boolean);

    if (!fallback) {
        return '無題のふせん';
    }

    return fallback.length > 36 ? `${fallback.slice(0, 36)}...` : fallback;
}

function summarizeContent(content: string) {
    const normalized = content.replace(/\s+/g, ' ').trim();
    if (normalized.length <= 120) {
        return normalized;
    }

    return `${normalized.slice(0, 120)}...`;
}

function formatTimestamp(value: string) {
    return new Date(value).toLocaleString('ja-JP', {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    });
}

interface SourceBadgeProps {
    source: ProjectNote['source'];
    poAssistantAvatarImage: string | null;
}

function SourceBadge({ source, poAssistantAvatarImage }: SourceBadgeProps) {
    if (source === 'po_assistant') {
        return (
            <div className="inline-flex items-center gap-2 rounded-full border border-amber-200 bg-white/80 px-2.5 py-1 text-[11px] font-semibold text-amber-900 shadow-sm">
                <Avatar kind="po-assistant" size="xs" imageSrc={poAssistantAvatarImage} alt={PO_ASSISTANT_ROLE_NAME} />
                {PO_ASSISTANT_ROLE_NAME}
            </div>
        );
    }

    return (
        <div className="inline-flex items-center gap-2 rounded-full border border-amber-200 bg-white/80 px-2.5 py-1 text-[11px] font-semibold text-amber-900 shadow-sm">
            <span className="flex h-5 w-5 items-center justify-center rounded-full bg-amber-100 text-amber-700">
                <UserRound size={11} />
            </span>
            手入力
        </div>
    );
}

export function NotesPanel() {
    const { currentProjectId } = useWorkspace();
    const { notes, loading, fetchNotes, addNote, updateNote, deleteNote } = useProjectNotes(currentProjectId);
    const {
        sessions,
        loading: retroLoading,
        fetchSessions,
        fetchItems,
        addItem,
    } = useRetrospective(currentProjectId);
    const poAssistantAvatarImage = usePoAssistantAvatarImage();

    const [composerContent, setComposerContent] = useState('');
    const [editingNoteId, setEditingNoteId] = useState<string | null>(null);
    const [editingContent, setEditingContent] = useState('');
    const [expandedNoteId, setExpandedNoteId] = useState<string | null>(null);
    const [transferMenuNoteId, setTransferMenuNoteId] = useState<string | null>(null);
    const [pendingDeleteNoteId, setPendingDeleteNoteId] = useState<string | null>(null);
    const [workingKey, setWorkingKey] = useState<string | null>(null);
    const orderedNotes = useMemo(
        () => [...notes].sort((left, right) => new Date(right.updated_at).getTime() - new Date(left.updated_at).getTime()),
        [notes],
    );

    const activeRetroSession = useMemo(() => {
        // in_progress → draft → completed の優先順で選択
        // completed セッションにも転記できるよう全ステータスを許可する
        return (
            sessions.find((session) => session.status === 'in_progress') ??
            sessions.find((session) => session.status === 'draft') ??
            sessions.find((session) => session.status === 'completed') ??
            null
        );
    }, [sessions]);

    const retroDisabledReason = retroLoading && sessions.length === 0
        ? 'レトロセッションを読み込み中です。'
        : 'レトロセッションがないため、まだ転記できません。';

    useEffect(() => {
        if (!currentProjectId) {
            return;
        }

        void fetchNotes();
        void fetchSessions();
    }, [currentProjectId, fetchNotes, fetchSessions]);

    // POアシスタントがバックエンドからふせん/レトロを追加したときに再取得する
    useEffect(() => {
        const unlisten = listen('kanban-updated', () => {
            void fetchNotes();
            void fetchSessions();
        });
        return () => {
            void unlisten.then((fn) => fn());
        };
    }, [fetchNotes, fetchSessions]);

    useEffect(() => {
        if (editingNoteId && !notes.some((note) => note.id === editingNoteId)) {
            setEditingNoteId(null);
            setEditingContent('');
        }

        if (expandedNoteId && !notes.some((note) => note.id === expandedNoteId)) {
            setExpandedNoteId(null);
        }

        if (transferMenuNoteId && !notes.some((note) => note.id === transferMenuNoteId)) {
            setTransferMenuNoteId(null);
        }

        if (pendingDeleteNoteId && !notes.some((note) => note.id === pendingDeleteNoteId)) {
            setPendingDeleteNoteId(null);
        }
    }, [editingNoteId, expandedNoteId, notes, pendingDeleteNoteId, transferMenuNoteId]);

    useEffect(() => {
        setComposerContent('');
        setEditingNoteId(null);
        setEditingContent('');
        setExpandedNoteId(null);
        setTransferMenuNoteId(null);
        setPendingDeleteNoteId(null);
    }, [currentProjectId]);

    const handleComposerSubmit = async () => {
        const content = composerContent.trim();
        if (!content) {
            toast.error('ふせんの内容を入力してください。');
            return;
        }

        setWorkingKey('create-note');
        try {
            const createdNote = await addNote(resolveStickyTitle(content), content, null);
            setComposerContent('');
            setExpandedNoteId(createdNote.id);
            toast.success('ふせんを追加しました。');
        } finally {
            setWorkingKey(null);
        }
    };

    const beginEdit = (note: ProjectNote) => {
        setEditingNoteId(note.id);
        setEditingContent(note.content);
        setExpandedNoteId(note.id);
        setTransferMenuNoteId(null);
    };

    const cancelEdit = () => {
        setEditingNoteId(null);
        setEditingContent('');
    };

    const handleSaveEdit = async (note: ProjectNote) => {
        const content = editingContent.trim();
        if (!content) {
            toast.error('ふせんの内容を入力してください。');
            return;
        }

        setWorkingKey(`save-${note.id}`);
        try {
            await updateNote(note.id, resolveStickyTitle(content), content);
            cancelEdit();
            toast.success('ふせんを更新しました。');
        } finally {
            setWorkingKey(null);
        }
    };

    const handleDelete = async (note: ProjectNote) => {
        setWorkingKey(`delete-${note.id}`);
        try {
            await deleteNote(note.id);
            setPendingDeleteNoteId(null);
            toast.success('ふせんを削除しました。');
        } finally {
            setWorkingKey(null);
        }
    };

    const handleTransfer = async (note: ProjectNote, category: RetroCategory) => {
        if (!activeRetroSession) {
            return;
        }

        const content = note.content.trim();
        if (!content) {
            toast.error('空のふせんはレトロへ転記できません。');
            return;
        }

        const transferKey = `transfer-${note.id}-${category}`;
        setWorkingKey(transferKey);
        try {
            const currentItems = await fetchItems(activeRetroSession.id);
            const sortOrder = currentItems.filter((item) => item.category === category).length;
            const retroSource = note.source === 'po_assistant' ? 'po' : 'user';

            await addItem(activeRetroSession.id, category, content, {
                source: retroSource,
                sortOrder,
            });
            await deleteNote(note.id);
            window.dispatchEvent(new CustomEvent(RETRO_ITEMS_UPDATED_EVENT, {
                detail: {
                    sessionId: activeRetroSession.id,
                },
            }));
            setTransferMenuNoteId(null);
            toast.success(`${RETRO_CATEGORY_OPTIONS.find((option) => option.value === category)?.label ?? category} に追加しました。`);
        } finally {
            setWorkingKey(null);
        }
    };

    const handleComposerKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
        event.stopPropagation();
        if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
            event.preventDefault();
            void handleComposerSubmit();
        }
    };

    const handleEditorKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>, note: ProjectNote) => {
        event.stopPropagation();
        if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
            event.preventDefault();
            void handleSaveEdit(note);
        }
    };

    if (!currentProjectId) {
        return (
            <div className="flex h-full items-center justify-center bg-amber-50/40 px-6 text-center text-sm text-amber-900/70">
                プロジェクトを選択すると、PO のふせんをここで管理できます。
            </div>
        );
    }

    return (
        <div className="flex h-full min-h-0 flex-col bg-[radial-gradient(circle_at_top,_rgba(254,243,199,0.72),_rgba(255,255,255,1)_62%)]">
            <div className="shrink-0 border-b border-amber-200/70 px-3 py-3">
                <div className="rounded-[28px] border border-amber-200 bg-gradient-to-br from-amber-100 via-yellow-50 to-amber-50 p-4 shadow-[0_12px_24px_-16px_rgba(180,120,30,0.45)]">
                    <div className="flex items-start justify-between gap-3">
                        <div>
                            <p className="text-[11px] font-semibold uppercase tracking-[0.22em] text-amber-700/80">
                                PO ふせん
                            </p>
                            <h3 className="mt-1 text-sm font-semibold text-amber-950">思いついたことを、そのまま残す</h3>
                            <p className="mt-1 text-xs leading-relaxed text-amber-900/70">
                                あとでレトロやバックログに振り分けやすいように、まずは短くメモしておけます。
                            </p>
                        </div>
                        <Button
                            type="button"
                            onClick={() => void handleComposerSubmit()}
                            disabled={workingKey === 'create-note'}
                            className="rounded-full bg-amber-500 text-amber-950 hover:bg-amber-400 focus:ring-amber-300"
                        >
                            {workingKey === 'create-note' ? (
                                <>
                                    <Loader2 size={14} className="mr-2 animate-spin" />
                                    保存中...
                                </>
                            ) : (
                                <>
                                    <Plus size={14} className="mr-2" />
                                    ふせんを追加
                                </>
                            )}
                        </Button>
                    </div>

                    <Textarea
                        value={composerContent}
                        onChange={(event) => setComposerContent(event.target.value)}
                        onKeyDown={handleComposerKeyDown}
                        placeholder="気づいたことをそのまま書き留めます。Ctrl+Enter で追加できます。"
                        className="mt-3 min-h-[132px] resize-none rounded-[24px] border-amber-200 bg-white/85 text-sm leading-6 text-amber-950"
                    />
                </div>
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto px-3 py-4">
                {loading ? (
                    <div className="flex h-full min-h-[220px] items-center justify-center text-sm text-amber-900/60">
                        <Loader2 size={16} className="mr-2 animate-spin" />
                        ふせんを読み込み中...
                    </div>
                ) : orderedNotes.length === 0 ? (
                    <div className="flex min-h-[240px] flex-col items-center justify-center rounded-[28px] border border-dashed border-amber-300 bg-white/65 px-6 text-center shadow-sm">
                        <div className="flex h-12 w-12 items-center justify-center rounded-full bg-amber-100 text-amber-700 shadow-sm">
                            <Plus size={18} />
                        </div>
                        <p className="mt-4 text-sm font-semibold text-amber-950">まだふせんがありません</p>
                        <p className="mt-2 text-xs leading-relaxed text-amber-900/65">
                            思いついた瞬間に一言だけでも残しておくと、あとで整理しやすくなります。
                        </p>
                    </div>
                ) : (
                    <div className="space-y-3">
                        {orderedNotes.map((note) => {
                            const isExpanded = expandedNoteId === note.id;
                            const isEditing = editingNoteId === note.id;
                            const isPendingDelete = pendingDeleteNoteId === note.id;

                            return (
                                <div
                                    key={note.id}
                                    className="relative overflow-hidden rounded-[30px] border border-amber-200 bg-[linear-gradient(145deg,rgba(254,243,199,0.95),rgba(255,251,235,0.98))] p-4 shadow-[0_14px_28px_-20px_rgba(120,90,20,0.45)]"
                                >
                                    <div className="absolute right-6 top-0 h-5 w-16 -translate-y-1/2 rotate-3 rounded-md bg-white/65 shadow-sm" />
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0 flex-1">
                                            <div className="flex flex-wrap items-center gap-2">
                                                <SourceBadge
                                                    source={note.source}
                                                    poAssistantAvatarImage={poAssistantAvatarImage}
                                                />
                                                <span className="text-[11px] text-amber-900/60">
                                                    {formatTimestamp(note.created_at)}
                                                </span>
                                            </div>
                                        </div>

                                        <div className="flex items-center gap-1">
                                            <button
                                                type="button"
                                                onClick={() => {
                                                    setExpandedNoteId((current) => current === note.id ? null : note.id);
                                                    setTransferMenuNoteId(null);
                                                }}
                                                className="rounded-full p-1.5 text-amber-900/60 transition-colors hover:bg-white/70 hover:text-amber-950"
                                                title={isExpanded ? '折りたたむ' : '内容を表示'}
                                            >
                                                {isExpanded ? <ChevronUp size={15} /> : <ChevronDown size={15} />}
                                            </button>
                                            {!isEditing && (
                                                <button
                                                    type="button"
                                                    onClick={() => beginEdit(note)}
                                                    className="rounded-full p-1.5 text-amber-900/60 transition-colors hover:bg-white/70 hover:text-amber-950"
                                                    title="編集"
                                                >
                                                    <Pencil size={15} />
                                                </button>
                                            )}
                                            <button
                                                type="button"
                                                onClick={() => setPendingDeleteNoteId((current) => current === note.id ? null : note.id)}
                                                className="rounded-full p-1.5 text-rose-500 transition-colors hover:bg-white/70 hover:text-rose-600"
                                                title="削除"
                                                disabled={workingKey === `delete-${note.id}`}
                                            >
                                                {workingKey === `delete-${note.id}` ? (
                                                    <Loader2 size={15} className="animate-spin" />
                                                ) : (
                                                    <Trash2 size={15} />
                                                )}
                                            </button>
                                        </div>
                                    </div>

                                    {isEditing ? (
                                        <div className="mt-4 space-y-3 rounded-[24px] border border-white/75 bg-white/72 p-3">
                                            <Textarea
                                                value={editingContent}
                                                onChange={(event) => setEditingContent(event.target.value)}
                                                onKeyDown={(event) => handleEditorKeyDown(event, note)}
                                                className="min-h-[140px] resize-none rounded-[24px] border-amber-200 bg-white text-sm leading-6 text-amber-950"
                                            />

                                            <div className="flex items-center justify-end gap-2">
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="sm"
                                                    onClick={cancelEdit}
                                                >
                                                    キャンセル
                                                </Button>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    onClick={() => void handleSaveEdit(note)}
                                                    disabled={workingKey === `save-${note.id}`}
                                                    className="rounded-full bg-amber-500 text-amber-950 hover:bg-amber-400 focus:ring-amber-300"
                                                >
                                                    {workingKey === `save-${note.id}` ? (
                                                        <>
                                                            <Loader2 size={14} className="mr-2 animate-spin" />
                                                            保存中...
                                                        </>
                                                    ) : (
                                                        '保存'
                                                    )}
                                                </Button>
                                            </div>
                                        </div>
                                    ) : (
                                        <>
                                            <div className="mt-4 rounded-[24px] border border-white/70 bg-white/70 px-4 py-3">
                                                {isExpanded ? (
                                                    <div className="prose prose-sm max-w-none text-amber-950 prose-headings:mb-2 prose-headings:mt-3 prose-p:my-2 prose-ul:my-2 prose-li:my-1">
                                                        <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                                            {note.content}
                                                        </ReactMarkdown>
                                                    </div>
                                                ) : (
                                                    <p className="whitespace-pre-wrap text-sm leading-6 text-amber-950/85">
                                                        {summarizeContent(note.content)}
                                                    </p>
                                                )}
                                            </div>

                                            {isPendingDelete && (
                                                <div className="mt-3 flex flex-wrap items-center justify-between gap-2 rounded-[20px] border border-rose-200 bg-rose-50/80 px-3 py-2">
                                                    <p className="text-xs font-medium text-rose-700">
                                                        このふせんを削除しますか？
                                                    </p>
                                                    <div className="flex items-center gap-2">
                                                        <Button
                                                            type="button"
                                                            variant="ghost"
                                                            size="sm"
                                                            onClick={() => setPendingDeleteNoteId(null)}
                                                            className="rounded-full text-rose-700 hover:bg-rose-100"
                                                        >
                                                            キャンセル
                                                        </Button>
                                                        <Button
                                                            type="button"
                                                            size="sm"
                                                            onClick={() => void handleDelete(note)}
                                                            disabled={workingKey === `delete-${note.id}`}
                                                            className="rounded-full bg-rose-500 text-white hover:bg-rose-600 focus:ring-rose-300"
                                                        >
                                                            {workingKey === `delete-${note.id}` ? (
                                                                <>
                                                                    <Loader2 size={14} className="mr-2 animate-spin" />
                                                                    削除中...
                                                                </>
                                                            ) : (
                                                                '削除する'
                                                            )}
                                                        </Button>
                                                    </div>
                                                </div>
                                            )}

                                            <div className="mt-3 space-y-2">
                                                <div className="flex flex-wrap items-center justify-between gap-2">
                                                    <div className="flex flex-wrap items-center gap-2">
                                                        {!activeRetroSession && (
                                                            <span className="text-[11px] text-amber-900/60">
                                                                {retroDisabledReason}
                                                            </span>
                                                        )}
                                                    </div>

                                                    <Button
                                                        type="button"
                                                        variant="ghost"
                                                        size="sm"
                                                        onClick={() => setTransferMenuNoteId((current) => current === note.id ? null : note.id)}
                                                        disabled={!activeRetroSession}
                                                        className="rounded-full border border-amber-300 bg-white/80 text-amber-950 hover:bg-white"
                                                        title={activeRetroSession ? '転記カテゴリを選ぶ' : retroDisabledReason}
                                                    >
                                                        レトロに追加
                                                        <ChevronDown size={13} className="ml-1" />
                                                    </Button>
                                                </div>

                                                {transferMenuNoteId === note.id && activeRetroSession && (
                                                    <div className="flex flex-wrap gap-2 rounded-[20px] border border-amber-200/80 bg-white/70 p-2">
                                                        {RETRO_CATEGORY_OPTIONS.map((option) => {
                                                            const isWorking = workingKey === `transfer-${note.id}-${option.value}`;

                                                            return (
                                                                <button
                                                                    key={option.value}
                                                                    type="button"
                                                                    onClick={() => void handleTransfer(note, option.value)}
                                                                    disabled={Boolean(workingKey) && !isWorking}
                                                                    className={`inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-semibold transition-colors ${option.className}`}
                                                                >
                                                                    {option.label}
                                                                    {isWorking && <Loader2 size={13} className="animate-spin" />}
                                                                </button>
                                                            );
                                                        })}
                                                    </div>
                                                )}
                                            </div>
                                        </>
                                    )}
                                </div>
                            );
                        })}
                    </div>
                )}
            </div>
        </div>
    );
}
