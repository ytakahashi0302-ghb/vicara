import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
    CalendarDays,
    CheckCircle2,
    ChevronDown,
    ChevronUp,
    Loader2,
    Pencil,
    Plus,
    ShieldCheck,
    Sparkles,
    Trash2,
    UserRound,
    Wand2,
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import toast from 'react-hot-toast';
import { useScrum } from '../../context/ScrumContext';
import { useWorkspace } from '../../context/WorkspaceContext';
import { useRetrospective } from '../../hooks/useRetrospective';
import { useProjectLabels } from '../../hooks/useProjectLabels';
import {
    usePoAssistantAvatarImage,
    VICARA_SETTINGS_UPDATED_EVENT,
} from '../../hooks/usePoAssistantAvatarImage';
import {
    RetroCategory,
    RetroItem,
    RetroSession,
    TeamConfiguration,
    TeamRoleSetting,
} from '../../types';
import { Avatar } from '../ai/Avatar';
import { RETRO_ITEMS_UPDATED_EVENT } from '../ai/NotesPanel';
import { PO_ASSISTANT_ROLE_NAME, resolveAvatarForRoleName } from '../ai/avatarRegistry';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { Textarea } from '../ui/Textarea';

type ColumnStyle = {
    title: string;
    panelClassName: string;
    badgeClassName: string;
    addButtonClassName: string;
    emptyClassName: string;
    cardAccentClassName: string;
};

const STATUS_STYLES: Record<RetroSession['status'], string> = {
    draft: 'border-slate-200 bg-slate-100 text-slate-700',
    in_progress: 'border-amber-200 bg-amber-100 text-amber-800',
    completed: 'border-emerald-200 bg-emerald-100 text-emerald-800',
};

const STATUS_LABELS: Record<RetroSession['status'], string> = {
    draft: 'Draft',
    in_progress: 'In Progress',
    completed: 'Completed',
};

const COLUMN_STYLES: Record<RetroCategory, ColumnStyle> = {
    keep: {
        title: 'Keep',
        panelClassName: 'border-emerald-200 bg-emerald-50/80',
        badgeClassName: 'border-emerald-200 bg-emerald-100 text-emerald-800',
        addButtonClassName: 'border-emerald-200 text-emerald-800 hover:bg-emerald-100',
        emptyClassName: 'text-emerald-700/70',
        cardAccentClassName: 'border-l-4 border-l-emerald-400',
    },
    problem: {
        title: 'Problem',
        panelClassName: 'border-rose-200 bg-rose-50/80',
        badgeClassName: 'border-rose-200 bg-rose-100 text-rose-800',
        addButtonClassName: 'border-rose-200 text-rose-800 hover:bg-rose-100',
        emptyClassName: 'text-rose-700/70',
        cardAccentClassName: 'border-l-4 border-l-rose-400',
    },
    try: {
        title: 'Try',
        panelClassName: 'border-sky-200 bg-sky-50/80',
        badgeClassName: 'border-sky-200 bg-sky-100 text-sky-800',
        addButtonClassName: 'border-sky-200 text-sky-800 hover:bg-sky-100',
        emptyClassName: 'text-sky-700/70',
        cardAccentClassName: 'border-l-4 border-l-sky-400',
    },
};

interface SourceBadgeProps {
    item: RetroItem;
    roleLookup: Record<string, TeamRoleSetting>;
    poAssistantAvatarImage: string | null;
}

function SourceBadge({ item, roleLookup, poAssistantAvatarImage }: SourceBadgeProps) {
    if (item.source === 'agent') {
        const sourceRole = item.source_role_id ? roleLookup[item.source_role_id] : undefined;
        const roleName = sourceRole?.name?.trim() || '開発エージェント';
        const avatar = resolveAvatarForRoleName(roleName);

        return (
            <div className="flex items-center gap-2">
                <Avatar
                    kind={avatar.kind}
                    size="xs"
                    imageSrc={sourceRole?.avatar_image ?? null}
                    alt={roleName}
                />
                <div className="min-w-0">
                    <div className="truncate text-xs font-semibold text-slate-800">{roleName}</div>
                    <div className="text-[11px] uppercase tracking-[0.18em] text-slate-500">Agent</div>
                </div>
            </div>
        );
    }

    if (item.source === 'po') {
        return (
            <div className="flex items-center gap-2">
                <Avatar
                    kind="po-assistant"
                    size="xs"
                    imageSrc={poAssistantAvatarImage}
                    alt={PO_ASSISTANT_ROLE_NAME}
                />
                <div>
                    <div className="text-xs font-semibold text-slate-800">{PO_ASSISTANT_ROLE_NAME}</div>
                    <div className="text-[11px] uppercase tracking-[0.18em] text-slate-500">PO</div>
                </div>
            </div>
        );
    }

    if (item.source === 'sm') {
        return (
            <div className="flex items-center gap-2">
                <div className="flex h-6 w-6 items-center justify-center rounded-full bg-amber-100 text-amber-700 ring-2 ring-amber-200/80">
                    <ShieldCheck size={12} />
                </div>
                <div>
                    <div className="text-xs font-semibold text-slate-800">Scrum Master</div>
                    <div className="text-[11px] uppercase tracking-[0.18em] text-slate-500">SM</div>
                </div>
            </div>
        );
    }

    return (
        <div className="flex items-center gap-2">
            <div className="flex h-6 w-6 items-center justify-center rounded-full bg-slate-100 text-slate-700 ring-2 ring-slate-200/80">
                <UserRound size={12} />
            </div>
            <div>
                <div className="text-xs font-semibold text-slate-800">Team Note</div>
                <div className="text-[11px] uppercase tracking-[0.18em] text-slate-500">User</div>
            </div>
        </div>
    );
}

export function RetrospectiveView() {
    const { sprints } = useScrum();
    const { currentProjectId } = useWorkspace();
    const { formatSprintLabel: formatSprintReference } = useProjectLabels();
    const {
        sessions,
        items,
        loading,
        fetchSessions,
        fetchItems,
        createSession,
        addItem,
        updateItem,
        deleteItem,
        approveItem,
    } = useRetrospective();
    const poAssistantAvatarImage = usePoAssistantAvatarImage();

    const [teamRoles, setTeamRoles] = useState<TeamRoleSetting[]>([]);
    const [selectedSprintId, setSelectedSprintId] = useState<string | null>(null);
    const [creatingSession, setCreatingSession] = useState(false);
    const [openComposerCategory, setOpenComposerCategory] = useState<RetroCategory | null>(null);
    const [drafts, setDrafts] = useState<Record<RetroCategory, string>>({
        keep: '',
        problem: '',
        try: '',
    });
    const [editingItemId, setEditingItemId] = useState<string | null>(null);
    const [editingContent, setEditingContent] = useState('');
    const [editingCategory, setEditingCategory] = useState<RetroCategory>('keep');
    const [workingKey, setWorkingKey] = useState<string | null>(null);
    // SMサマリは最初たたまれた状態
    const [summaryOpen, setSummaryOpen] = useState(false);
    const [agentLoading, setAgentLoading] = useState(false);
    const [kptLoading, setKptLoading] = useState(false);
    // true: スプリント内で稼働実績のないロールをスキップ（デフォルト）
    const [skipInactiveRoles, setSkipInactiveRoles] = useState(true);
    // 承認済み Try 一覧
    const [approvedTryItems, setApprovedTryItems] = useState<RetroItem[]>([]);
    const [approvedTryOpen, setApprovedTryOpen] = useState(false);

    const completedSprints = [...sprints]
        .filter((sprint) => sprint.status === 'Completed')
        .sort((left, right) => (right.completed_at ?? 0) - (left.completed_at ?? 0));

    const currentSession = selectedSprintId
        ? sessions.find((session) => session.sprint_id === selectedSprintId) ?? null
        : null;

    const roleLookup = Object.fromEntries(teamRoles.map((role) => [role.id, role]));

    const groupedItems: Record<RetroCategory, RetroItem[]> = {
        keep: items.filter((item) => item.category === 'keep'),
        problem: items.filter((item) => item.category === 'problem'),
        try: items.filter((item) => item.category === 'try'),
    };

    const formatSprintSelectorLabel = (sequenceSprint: { sequence_number: number; completed_at: number | null }) => {
        const completedAt = sequenceSprint.completed_at
            ? new Date(sequenceSprint.completed_at).toLocaleString('ja-JP', {
                month: 'short',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
            })
            : '完了時刻未記録';

        return `${formatSprintReference(sequenceSprint)} / ${completedAt}`;
    };

    useEffect(() => {
        void fetchSessions();
    }, [fetchSessions]);

    useEffect(() => {
        if (completedSprints.length === 0) {
            setSelectedSprintId(null);
            return;
        }

        if (!selectedSprintId || !completedSprints.some((sprint) => sprint.id === selectedSprintId)) {
            setSelectedSprintId(completedSprints[0].id);
        }
    }, [completedSprints, selectedSprintId]);

    useEffect(() => {
        const sessionId = currentSession?.id ?? null;
        void fetchItems(sessionId);
    }, [currentSession?.id, fetchItems]);

    useEffect(() => {
        const handleRetroItemsUpdated = (event: Event) => {
            const customEvent = event as CustomEvent<{ sessionId?: string }>;
            const updatedSessionId = customEvent.detail?.sessionId ?? null;
            if (!updatedSessionId || updatedSessionId !== currentSession?.id) {
                return;
            }

            void fetchItems(updatedSessionId);
        };

        window.addEventListener(RETRO_ITEMS_UPDATED_EVENT, handleRetroItemsUpdated);

        return () => {
            window.removeEventListener(RETRO_ITEMS_UPDATED_EVENT, handleRetroItemsUpdated);
        };
    }, [currentSession?.id, fetchItems]);

    // POアシスタントがバックエンドからレトロアイテムを追加したときに再取得する
    useEffect(() => {
        const sessionId = currentSession?.id ?? null;
        const unlisten = listen('kanban-updated', () => {
            void fetchItems(sessionId);
        });
        return () => {
            void unlisten.then((fn) => fn());
        };
    }, [currentSession?.id, fetchItems]);

    useEffect(() => {
        setOpenComposerCategory(null);
        setEditingItemId(null);
    }, [selectedSprintId]);

    // 承認済み Try アイテムを取得（セッション/アイテム更新時に再取得）
    useEffect(() => {
        if (!currentProjectId) return;
        void invoke<RetroItem[]>('get_approved_try_items', { projectId: currentProjectId })
            .then(setApprovedTryItems)
            .catch((err) => console.error('Failed to load approved try items', err));
    }, [currentProjectId, items]);

    useEffect(() => {
        let cancelled = false;

        const loadTeamRoles = async () => {
            try {
                const config = await invoke<TeamConfiguration>('get_team_configuration');
                if (!cancelled) {
                    setTeamRoles(config.roles);
                }
            } catch (error) {
                console.error('Failed to load team roles for retrospective view', error);
                if (!cancelled) {
                    setTeamRoles([]);
                }
            }
        };

        const handleSettingsUpdated = () => {
            void loadTeamRoles();
        };

        void loadTeamRoles();
        window.addEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);

        return () => {
            cancelled = true;
            window.removeEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
        };
    }, []);

    const resetEditor = () => {
        setEditingItemId(null);
        setEditingContent('');
        setEditingCategory('keep');
    };

    const ensureSession = async () => {
        if (currentSession) {
            return currentSession;
        }

        if (!selectedSprintId) {
            toast.error('先にスプリントを選択してください。');
            return null;
        }

        setCreatingSession(true);
        setWorkingKey('create-session');

        try {
            const session = await createSession(selectedSprintId);
            await fetchItems(session.id);
            return session;
        } finally {
            setCreatingSession(false);
            setWorkingKey(null);
        }
    };

    const handleCreateSession = async () => {
        await ensureSession();
    };

    const handleDraftChange = (category: RetroCategory, value: string) => {
        setDrafts((current) => ({ ...current, [category]: value }));
    };

    const handleAddItem = async (category: RetroCategory) => {
        const content = drafts[category].trim();
        if (!content) {
            toast.error('カード内容を入力してください。');
            return;
        }

        const session = await ensureSession();
        if (!session) {
            return;
        }

        setWorkingKey(`add-${category}`);
        try {
            await addItem(session.id, category, content, {
                sortOrder: groupedItems[category].length,
            });
            setDrafts((current) => ({ ...current, [category]: '' }));
            setOpenComposerCategory(null);
        } finally {
            setWorkingKey(null);
        }
    };

    const beginEdit = (item: RetroItem) => {
        setEditingItemId(item.id);
        setEditingContent(item.content);
        setEditingCategory(item.category);
    };

    const handleSaveEdit = async (item: RetroItem) => {
        const content = editingContent.trim();
        if (!content) {
            toast.error('カード内容を入力してください。');
            return;
        }

        setWorkingKey(`edit-${item.id}`);
        try {
            await updateItem(item.id, content, editingCategory);
            resetEditor();
        } finally {
            setWorkingKey(null);
        }
    };

    const handleDelete = async (item: RetroItem) => {
        const confirmed = window.confirm('この KPT カードを削除しますか？');
        if (!confirmed) {
            return;
        }

        setWorkingKey(`delete-${item.id}`);
        try {
            await deleteItem(item.id);
            if (editingItemId === item.id) {
                resetEditor();
            }
        } finally {
            setWorkingKey(null);
        }
    };

    const handleStartRetro = async () => {
        const session = await ensureSession();
        if (!session) {
            return;
        }

        if (teamRoles.length === 0) {
            toast.error('チームロールが設定されていません。先にチーム構成を保存してください。');
            return;
        }

        setAgentLoading(true);
        const toastId = toast.loading(`エージェントレビューを生成中 (0/${teamRoles.length})`);
        try {
            let completed = 0;
            for (const role of teamRoles) {
                try {
                    await invoke<RetroItem[]>('generate_agent_retro_review', {
                        projectId: session.project_id,
                        sprintId: session.sprint_id,
                        retroSessionId: session.id,
                        roleId: role.id,
                        skipInactive: skipInactiveRoles,
                    });
                } catch (error) {
                    console.error('generate_agent_retro_review failed', role.name, error);
                    toast.error(`${role.name} のレビュー生成に失敗しました: ${String(error)}`);
                }
                completed += 1;
                toast.loading(`エージェントレビューを生成中 (${completed}/${teamRoles.length})`, {
                    id: toastId,
                });
            }
            await fetchItems(session.id);
            toast.success('エージェントレビューを取り込みました。', { id: toastId });
        } catch (error) {
            console.error('handleStartRetro failed', error);
            toast.error(`レトロ開始に失敗しました: ${String(error)}`, { id: toastId });
        } finally {
            setAgentLoading(false);
        }
    };

    const handleSynthesizeKpt = async () => {
        if (!currentSession) {
            toast.error('セッションが見つかりません。');
            return;
        }

        if (items.length === 0) {
            toast.error('まとめ対象のカードがありません。先にレトロ開始かカード追加を行ってください。');
            return;
        }

        setKptLoading(true);
        const toastId = toast.loading('SM がスプリントをまとめています...');
        try {
            await invoke<string>('synthesize_retro_kpt', {
                projectId: currentSession.project_id,
                sprintId: currentSession.sprint_id,
                retroSessionId: currentSession.id,
            });
            await Promise.all([fetchSessions(), fetchItems(currentSession.id)]);
            toast.success('SM サマリを生成しました。', { id: toastId });
            setSummaryOpen(true); // 完了後はサマリを自動展開
        } catch (error) {
            console.error('handleSynthesizeKpt failed', error);
            toast.error(`レトロの締めに失敗しました: ${String(error)}`, { id: toastId });
        } finally {
            setKptLoading(false);
        }
    };

    const handleApprove = async (item: RetroItem) => {
        if (item.is_approved) {
            return;
        }

        setWorkingKey(`approve-${item.id}`);
        try {
            await approveItem(item.id);
        } finally {
            setWorkingKey(null);
        }
    };

    if (completedSprints.length === 0) {
        return (
            <div className="p-6">
                <Card className="border-dashed border-slate-300 bg-slate-50 p-10 text-center">
                    <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-full bg-white text-slate-500 shadow-sm">
                        <CalendarDays size={22} />
                    </div>
                    <h3 className="text-lg font-semibold text-slate-900">完了済みスプリントがありません</h3>
                    <p className="mt-2 text-sm text-slate-600">
                        スプリントを完了すると、自動生成されたレトロセッションをここで確認できます。
                    </p>
                </Card>
            </div>
        );
    }

    return (
        <div className="space-y-6 p-6">
            <Card className="overflow-hidden">
                <div className="flex flex-col gap-4 border-b border-slate-200 bg-white p-5 lg:flex-row lg:items-end lg:justify-between">
                    <div className="space-y-2">
                        <p className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">
                            Sprint Retrospective
                        </p>
                        <div>
                            <label htmlFor="retro-sprint-selector" className="mb-2 block text-sm font-medium text-slate-700">
                                振り返るスプリント
                            </label>
                            <select
                                id="retro-sprint-selector"
                                value={selectedSprintId ?? ''}
                                onChange={(event) => setSelectedSprintId(event.target.value || null)}
                                className="w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 shadow-sm outline-none transition focus:border-blue-500 focus:ring-2 focus:ring-blue-500 lg:min-w-[320px]"
                            >
                                {completedSprints.map((sprint) => (
                                    <option key={sprint.id} value={sprint.id}>
                                        {formatSprintSelectorLabel(sprint)}
                                    </option>
                                ))}
                            </select>
                        </div>
                    </div>

                    {currentSession ? (
                        <div className="flex flex-wrap items-center gap-2 self-start lg:self-auto">
                            <span className={`rounded-full border px-3 py-1 text-xs font-semibold ${STATUS_STYLES[currentSession.status]}`}>
                                {STATUS_LABELS[currentSession.status]}
                            </span>
                            <label className="flex cursor-pointer items-center gap-1.5 rounded-full border border-slate-200 bg-slate-50 px-3 py-1.5 text-xs font-medium text-slate-700 select-none">
                                <input
                                    type="checkbox"
                                    checked={skipInactiveRoles}
                                    onChange={(e) => {
                                        const next = e.target.checked;
                                        setSkipInactiveRoles(next);
                                        if (!next) {
                                            toast('全ロールをレビューします。LLM 使用量が増加します。', {
                                                icon: '⚠️',
                                            });
                                        }
                                    }}
                                    className="h-3.5 w-3.5 rounded border-slate-300 text-blue-600"
                                    disabled={agentLoading || kptLoading}
                                />
                                稼働DEVのみ
                            </label>
                            <Button
                                type="button"
                                size="sm"
                                variant="secondary"
                                onClick={() => void handleStartRetro()}
                                disabled={
                                    agentLoading ||
                                    kptLoading ||
                                    currentSession.status === 'completed'
                                }
                            >
                                {agentLoading ? (
                                    <>
                                        <Loader2 size={14} className="mr-2 animate-spin" />
                                        生成中...
                                    </>
                                ) : (
                                    <>
                                        <Sparkles size={14} className="mr-2" />
                                        レトロ開始
                                    </>
                                )}
                            </Button>
                            <Button
                                type="button"
                                size="sm"
                                onClick={() => void handleSynthesizeKpt()}
                                disabled={agentLoading || kptLoading || items.length === 0}
                            >
                                {kptLoading ? (
                                    <>
                                        <Loader2 size={14} className="mr-2 animate-spin" />
                                        まとめ中...
                                    </>
                                ) : (
                                    <>
                                        <Wand2 size={14} className="mr-2" />
                                        レトロを締める
                                    </>
                                )}
                            </Button>
                        </div>
                    ) : (
                        <div className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm text-slate-600">
                            セッション未作成
                        </div>
                    )}
                </div>

                <div className="p-5">
                    {loading && !currentSession ? (
                        <div className="flex min-h-[240px] items-center justify-center text-slate-500">
                            <Loader2 className="mr-2 animate-spin" size={18} />
                            レトロセッションを読み込み中...
                        </div>
                    ) : currentSession ? (
                        <div className="space-y-6">
                            {/* SM サマリ — 上部に配置、デフォルトは折りたたみ */}
                            <Card className="overflow-hidden border border-slate-200">
                                <button
                                    type="button"
                                    className="flex w-full items-center justify-between gap-3 bg-slate-50 px-5 py-4 text-left"
                                    onClick={() => setSummaryOpen((current) => !current)}
                                >
                                    <div>
                                        <div className="text-xs font-semibold uppercase tracking-[0.2em] text-slate-500">
                                            SM Summary
                                        </div>
                                        <div className="mt-1 text-sm font-semibold text-slate-900">
                                            Scrum Master 合成サマリ
                                        </div>
                                    </div>
                                    {summaryOpen ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
                                </button>
                                {summaryOpen && (
                                    <div className="border-t border-slate-200 bg-white px-5 py-4">
                                        {currentSession.summary ? (
                                            <div className="prose prose-sm max-w-none prose-slate prose-headings:mb-2 prose-p:leading-relaxed">
                                                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                                    {currentSession.summary}
                                                </ReactMarkdown>
                                            </div>
                                        ) : (
                                            <p className="text-sm leading-6 text-slate-500">
                                                SM 合成サマリはまだ生成されていません。「レトロを締める」を実行するとここに表示されます。
                                            </p>
                                        )}
                                    </div>
                                )}
                            </Card>

                            <div className="grid gap-4 xl:grid-cols-3">
                                {(['keep', 'problem', 'try'] as RetroCategory[]).map((category) => {
                                    const style = COLUMN_STYLES[category];
                                    const columnItems = groupedItems[category];
                                    const composerOpen = openComposerCategory === category;

                                    return (
                                        <Card
                                            key={category}
                                            className={`flex h-full flex-col border ${style.panelClassName}`}
                                        >
                                            <div className="flex items-center justify-between border-b border-white/70 px-4 py-3">
                                                <div className="flex items-center gap-2">
                                                    <h3 className="text-base font-semibold text-slate-900">
                                                        {style.title}
                                                    </h3>
                                                    <span className={`rounded-full border px-2 py-0.5 text-xs font-semibold ${style.badgeClassName}`}>
                                                        {columnItems.length}
                                                    </span>
                                                </div>
                                                <Button
                                                    type="button"
                                                    variant="ghost"
                                                    size="sm"
                                                    className={`border bg-white/70 ${style.addButtonClassName}`}
                                                    onClick={() => setOpenComposerCategory(composerOpen ? null : category)}
                                                >
                                                    <Plus size={14} className="mr-1" />
                                                    カード追加
                                                </Button>
                                            </div>

                                            <div className="flex flex-1 flex-col gap-3 p-4">
                                                {composerOpen && (
                                                    <div className="rounded-xl border border-white/70 bg-white/85 p-3 shadow-sm">
                                                        <Textarea
                                                            value={drafts[category]}
                                                            onChange={(event) => handleDraftChange(category, event.target.value)}
                                                            placeholder={`${style.title} に追加したい内容を入力してください`}
                                                            className="min-h-[96px] bg-white"
                                                        />
                                                        <div className="mt-3 flex items-center justify-end gap-2">
                                                            <Button
                                                                type="button"
                                                                variant="ghost"
                                                                size="sm"
                                                                onClick={() => setOpenComposerCategory(null)}
                                                            >
                                                                キャンセル
                                                            </Button>
                                                            <Button
                                                                type="button"
                                                                size="sm"
                                                                onClick={() => void handleAddItem(category)}
                                                                disabled={workingKey === `add-${category}`}
                                                            >
                                                                {workingKey === `add-${category}` ? (
                                                                    <>
                                                                        <Loader2 size={14} className="mr-2 animate-spin" />
                                                                        追加中...
                                                                    </>
                                                                ) : (
                                                                    '追加'
                                                                )}
                                                            </Button>
                                                        </div>
                                                    </div>
                                                )}

                                                {columnItems.length === 0 ? (
                                                    <div className={`flex flex-1 items-center justify-center rounded-xl border border-dashed border-white/80 bg-white/50 px-4 py-8 text-center text-sm ${style.emptyClassName}`}>
                                                        まだカードがありません。気づきを最初の 1 枚として追加できます。
                                                    </div>
                                                ) : (
                                                    columnItems.map((item) => {
                                                        const isEditing = editingItemId === item.id;
                                                        const isWorking =
                                                            workingKey === `edit-${item.id}` ||
                                                            workingKey === `delete-${item.id}` ||
                                                            workingKey === `approve-${item.id}`;

                                                        return (
                                                            <div
                                                                key={item.id}
                                                                className={`rounded-2xl border border-white/80 bg-white p-4 shadow-sm ${style.cardAccentClassName}`}
                                                            >
                                                                <div className="flex items-start justify-between gap-3">
                                                                    <SourceBadge
                                                                        item={item}
                                                                        roleLookup={roleLookup}
                                                                        poAssistantAvatarImage={poAssistantAvatarImage}
                                                                    />
                                                                    <div className="flex items-center gap-1">
                                                                        {!isEditing && (
                                                                            <Button
                                                                                type="button"
                                                                                variant="ghost"
                                                                                size="sm"
                                                                                className="h-8 w-8 px-0 text-slate-500"
                                                                                onClick={() => beginEdit(item)}
                                                                            >
                                                                                <Pencil size={14} />
                                                                            </Button>
                                                                        )}
                                                                        <Button
                                                                            type="button"
                                                                            variant="ghost"
                                                                            size="sm"
                                                                            className="h-8 w-8 px-0 text-rose-500 hover:bg-rose-50"
                                                                            onClick={() => void handleDelete(item)}
                                                                            disabled={isWorking}
                                                                        >
                                                                            {workingKey === `delete-${item.id}` ? (
                                                                                <Loader2 size={14} className="animate-spin" />
                                                                            ) : (
                                                                                <Trash2 size={14} />
                                                                            )}
                                                                        </Button>
                                                                    </div>
                                                                </div>

                                                                {isEditing ? (
                                                                    <div className="mt-3 space-y-3">
                                                                        <select
                                                                            value={editingCategory}
                                                                            onChange={(event) => setEditingCategory(event.target.value as RetroCategory)}
                                                                            className="w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none transition focus:border-blue-500 focus:ring-2 focus:ring-blue-500"
                                                                        >
                                                                            {(['keep', 'problem', 'try'] as RetroCategory[]).map((value) => (
                                                                                <option key={value} value={value}>
                                                                                    {COLUMN_STYLES[value].title}
                                                                                </option>
                                                                            ))}
                                                                        </select>
                                                                        <Textarea
                                                                            value={editingContent}
                                                                            onChange={(event) => setEditingContent(event.target.value)}
                                                                            className="min-h-[110px] bg-white"
                                                                        />
                                                                        <div className="flex items-center justify-end gap-2">
                                                                            <Button
                                                                                type="button"
                                                                                variant="ghost"
                                                                                size="sm"
                                                                                onClick={resetEditor}
                                                                            >
                                                                                キャンセル
                                                                            </Button>
                                                                            <Button
                                                                                type="button"
                                                                                size="sm"
                                                                                onClick={() => void handleSaveEdit(item)}
                                                                                disabled={workingKey === `edit-${item.id}`}
                                                                            >
                                                                                {workingKey === `edit-${item.id}` ? (
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
                                                                        <p className="mt-3 whitespace-pre-wrap text-sm leading-6 text-slate-700">
                                                                            {item.content}
                                                                        </p>
                                                                        <div className="mt-4 flex flex-wrap items-center justify-between gap-2">
                                                                            <span className="text-xs text-slate-400">
                                                                                {new Date(item.created_at).toLocaleString('ja-JP')}
                                                                            </span>
                                                                            {category === 'try' ? (
                                                                                <label className={`inline-flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-semibold ${
                                                                                    item.is_approved
                                                                                        ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
                                                                                        : 'border-sky-200 bg-sky-50 text-sky-700'
                                                                                }`}>
                                                                                    <input
                                                                                        type="checkbox"
                                                                                        checked={item.is_approved}
                                                                                        onChange={() => void handleApprove(item)}
                                                                                        disabled={item.is_approved || isWorking}
                                                                                        className="h-3.5 w-3.5 rounded border-slate-300 text-emerald-600 focus:ring-emerald-500"
                                                                                    />
                                                                                    {item.is_approved ? '承認済み' : '承認'}
                                                                                </label>
                                                                            ) : (
                                                                                item.is_approved && (
                                                                                    <span className="inline-flex items-center gap-1 rounded-full border border-emerald-200 bg-emerald-50 px-3 py-1 text-xs font-semibold text-emerald-700">
                                                                                        <CheckCircle2 size={12} />
                                                                                        承認済み
                                                                                    </span>
                                                                                )
                                                                            )}
                                                                        </div>
                                                                    </>
                                                                )}
                                                            </div>
                                                        );
                                                    })
                                                )}
                                            </div>
                                        </Card>
                                    );
                                })}
                            </div>

                        </div>
                    ) : (
                        <div className="rounded-xl border border-dashed border-slate-300 bg-slate-50 p-10 text-center">
                            <h3 className="text-lg font-semibold text-slate-900">
                                レトロセッションがまだ作成されていません
                            </h3>
                            <p className="mt-2 text-sm text-slate-600">
                                スプリントが完了後、ここから手動でレトロを開始できます。
                            </p>
                            <Button
                                type="button"
                                className="mt-5"
                                onClick={() => void handleCreateSession()}
                                disabled={creatingSession || workingKey === 'create-session'}
                            >
                                {creatingSession ? (
                                    <>
                                        <Loader2 size={16} className="mr-2 animate-spin" />
                                        セッションを作成中...
                                    </>
                                ) : (
                                    'レトロセッションを作成'
                                )}
                            </Button>
                        </div>
                    )}
                </div>
            </Card>

            {/* 承認済み Try 一覧（全スプリント横断）*/}
            {approvedTryItems.length > 0 && (
                <Card className="overflow-hidden border border-emerald-200">
                    <button
                        type="button"
                        className="flex w-full items-center justify-between gap-3 bg-emerald-50 px-5 py-4 text-left"
                        onClick={() => setApprovedTryOpen((prev) => !prev)}
                    >
                        <div className="flex items-center gap-3">
                            <div className="flex h-8 w-8 items-center justify-center rounded-full bg-emerald-100 text-emerald-700">
                                <CheckCircle2 size={16} />
                            </div>
                            <div>
                                <div className="text-xs font-semibold uppercase tracking-[0.2em] text-emerald-700">
                                    Approved Try
                                </div>
                                <div className="mt-0.5 text-sm font-semibold text-slate-900">
                                    承認済み Try 一覧
                                    <span className="ml-2 rounded-full border border-emerald-200 bg-emerald-100 px-2 py-0.5 text-xs font-semibold text-emerald-800">
                                        {approvedTryItems.length}
                                    </span>
                                </div>
                            </div>
                        </div>
                        {approvedTryOpen ? <ChevronUp size={18} /> : <ChevronDown size={18} />}
                    </button>
                    {approvedTryOpen && (
                        <div className="divide-y divide-emerald-100 border-t border-emerald-200 bg-white">
                            {approvedTryItems.map((item) => {
                                const session = sessions.find((s) => s.id === item.retro_session_id);
                                const sprint = session
                                    ? sprints.find((sp) => sp.id === session.sprint_id)
                                    : undefined;
                                const sprintLabel = sprint
                                    ? formatSprintReference(sprint)
                                    : session
                                      ? 'Sprint'
                                      : '—';

                                return (
                                    <div key={item.id} className="flex items-start gap-4 px-5 py-4">
                                        <span className="mt-0.5 shrink-0 rounded-full border border-emerald-200 bg-emerald-50 px-2 py-0.5 text-[11px] font-semibold text-emerald-700">
                                            {sprintLabel}
                                        </span>
                                        <p className="flex-1 whitespace-pre-wrap text-sm leading-6 text-slate-800">
                                            {item.content}
                                        </p>
                                        <span className="shrink-0 text-xs text-slate-400">
                                            {new Date(item.created_at).toLocaleDateString('ja-JP')}
                                        </span>
                                    </div>
                                );
                            })}
                        </div>
                    )}
                </Card>
            )}
        </div>
    );
}
