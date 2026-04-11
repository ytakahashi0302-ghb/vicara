import { ChangeEvent, useEffect, useState } from 'react';
import {
    Bot,
    Cpu,
    ImagePlus,
    Plus,
    RefreshCw,
    RotateCcw,
    TerminalSquare,
    Trash2,
    Users,
} from 'lucide-react';
import { Button } from './Button';
import { TeamConfiguration, TeamRoleSetting } from '../../types';
import type { CliDetectionResult } from '../../hooks/useCliDetection';
import { getAvatarDefinition, resolveAvatarImageSource } from '../ai/avatarRegistry';

interface TeamSettingsTabProps {
    embedded?: boolean;
    config: TeamConfiguration;
    validationMessages: string[];
    isLoading: boolean;
    anthropicModelsList: string[];
    geminiModelsList: string[];
    cliResults: CliDetectionResult[];
    installedCliMap: Record<SupportedCliType, boolean>;
    isCliDetectionLoading: boolean;
    isFetchingAnthropicModels: boolean;
    isFetchingGeminiModels: boolean;
    canFetchAnthropicModels: boolean;
    canFetchGeminiModels: boolean;
    onChange: (config: TeamConfiguration) => void;
    onFetchAnthropicModels: () => void;
    onFetchGeminiModels: () => void;
}

type SupportedCliType = 'claude' | 'gemini' | 'codex';

const CLI_OPTIONS: Array<{
    value: SupportedCliType;
    label: string;
    description: string;
}> = [
    {
        value: 'claude',
        label: 'Claude Code',
        description: 'Anthropic CLI',
    },
    {
        value: 'gemini',
        label: 'Gemini CLI',
        description: 'Google CLI',
    },
    {
        value: 'codex',
        label: 'Codex CLI',
        description: 'OpenAI CLI',
    },
];

const DEFAULT_MODELS: Record<SupportedCliType, string> = {
    claude: 'claude-haiku-4-5',
    gemini: 'gemini-3-flash-preview',
    codex: 'gpt-5.4-mini',
};

function normalizeCliType(value: string): SupportedCliType {
    switch (value) {
        case 'gemini':
            return 'gemini';
        case 'codex':
            return 'codex';
        default:
            return 'claude';
    }
}

function getDefaultModel(cliType: SupportedCliType): string {
    return DEFAULT_MODELS[cliType];
}

function getCliDetectionResult(
    cliType: SupportedCliType,
    cliResults: CliDetectionResult[],
): CliDetectionResult | undefined {
    return cliResults.find((result) => result.name === cliType);
}

function getCliOptionMeta(
    cliType: SupportedCliType,
    cliResults: CliDetectionResult[],
    isCliDetectionLoading: boolean,
): {
    label: string;
    detail: string;
    isInstalled: boolean;
} {
    const option = CLI_OPTIONS.find((candidate) => candidate.value === cliType);
    const detection = getCliDetectionResult(cliType, cliResults);
    const baseLabel = option?.label ?? cliType;
    const baseDescription = option?.description ?? cliType;

    if (isCliDetectionLoading) {
        return {
            label: baseLabel,
            detail: `${baseDescription} / 検出状況を確認中`,
            isInstalled: true,
        };
    }

    if (!detection?.installed) {
        return {
            label: baseLabel,
            detail: `${baseDescription} / 未検出`,
            isInstalled: false,
        };
    }

    return {
        label: baseLabel,
        detail: detection.version
            ? `${baseDescription} / 検出済み: ${detection.version}`
            : `${baseDescription} / 検出済み`,
        isInstalled: true,
    };
}

function getModelLabel(cliType: SupportedCliType): string {
    switch (cliType) {
        case 'gemini':
            return 'Gemini モデル';
        case 'codex':
            return 'Codex モデル';
        default:
            return 'Claude モデル';
    }
}

function getModelPlaceholder(cliType: SupportedCliType): string {
    switch (cliType) {
        case 'gemini':
            return '例: gemini-3-flash-preview';
        case 'codex':
            return '例: gpt-5.4-mini';
        default:
            return '例: claude-haiku-4-5';
    }
}

function getModelHint(cliType: SupportedCliType): string {
    switch (cliType) {
        case 'gemini':
            return 'Gemini CLI はプロジェクトや認証方法によって利用可能モデルが変わるため、必要に応じて API カタログまたは公式ドキュメントを参考に入力してください。';
        case 'codex':
            return 'Codex CLI は CLI から安定したモデル一覧取得を提供していないため、推奨既定値 `gpt-5.4-mini` を起点に手動で指定してください。';
        default:
            return 'Claude Code CLI では Anthropic API カタログを参考にできます。未取得時はモデル名を手動入力してください。';
    }
}

function getCatalogStatusMessage(
    providerName: string,
    models: string[],
    keyConfigured: boolean,
): string {
    if (models.length > 0) {
        return `${models.length} 件の ${providerName} モデルを取得済みです。該当 CLI のロール設定時に参考として利用できます。`;
    }

    if (!keyConfigured) {
        return `${providerName} API Key が未設定です。必要な場合のみ設定してモデルカタログを取得してください。`;
    }

    return `${providerName} モデルカタログは未取得です。必要に応じて取得してロール設定の参考にしてください。`;
}

function getSelectableModels(
    cliType: SupportedCliType,
    anthropicModelsList: string[],
    geminiModelsList: string[],
): string[] {
    switch (cliType) {
        case 'gemini':
            return geminiModelsList;
        case 'claude':
            return anthropicModelsList;
        default:
            return [];
    }
}

function getDefaultNewRoleCliType(installedCliMap: Record<SupportedCliType, boolean>): SupportedCliType {
    if (installedCliMap.claude) return 'claude';
    if (installedCliMap.gemini) return 'gemini';
    if (installedCliMap.codex) return 'codex';
    return 'claude';
}

function createEmptyRole(cliType: SupportedCliType): TeamRoleSetting {
    return {
        id: crypto.randomUUID(),
        name: '',
        system_prompt: '',
        cli_type: cliType,
        model: getDefaultModel(cliType),
        avatar_image: null,
        sort_order: 0,
    };
}

function normalizeRoles(roles: TeamRoleSetting[]): TeamRoleSetting[] {
    return roles.map((role, index) => ({
        ...role,
        sort_order: index,
    }));
}

function getRoleAvatarSource(avatarImage: string | null | undefined): string {
    return resolveAvatarImageSource(avatarImage) ?? getAvatarDefinition('dev-agent').src;
}

export function TeamSettingsTab({
    embedded = false,
    config,
    validationMessages,
    isLoading,
    anthropicModelsList,
    geminiModelsList,
    cliResults,
    installedCliMap,
    isCliDetectionLoading,
    isFetchingAnthropicModels,
    isFetchingGeminiModels,
    canFetchAnthropicModels,
    canFetchGeminiModels,
    onChange,
    onFetchAnthropicModels,
    onFetchGeminiModels,
}: TeamSettingsTabProps) {
    const roleCount = config.roles.length;
    const [activeRoleId, setActiveRoleId] = useState<string | null>(config.roles[0]?.id ?? null);

    useEffect(() => {
        if (config.roles.length === 0) {
            setActiveRoleId(null);
            return;
        }

        if (!activeRoleId || !config.roles.some((role) => role.id === activeRoleId)) {
            setActiveRoleId(config.roles[0].id);
        }
    }, [config.roles, activeRoleId]);

    const activeRole = config.roles.find((role) => role.id === activeRoleId) ?? config.roles[0] ?? null;
    const activeCliType = activeRole ? normalizeCliType(activeRole.cli_type) : null;
    const activeCliMeta = activeCliType
        ? getCliOptionMeta(activeCliType, cliResults, isCliDetectionLoading)
        : null;
    const selectableModels = activeCliType
        ? getSelectableModels(activeCliType, anthropicModelsList, geminiModelsList)
        : [];
    const hasSelectableModels = selectableModels.length > 0;
    const modelSuggestionsId = activeRole ? `team-role-models-${activeRole.id}` : '';
    const avatarInputId = activeRole ? `team-role-avatar-${activeRole.id}` : '';
    const canFetchCatalog =
        activeCliType === 'claude'
            ? canFetchAnthropicModels
            : activeCliType === 'gemini'
              ? canFetchGeminiModels
              : false;
    const isFetchingCatalog =
        activeCliType === 'claude'
            ? isFetchingAnthropicModels
            : activeCliType === 'gemini'
              ? isFetchingGeminiModels
              : false;
    const onFetchCatalog =
        activeCliType === 'claude'
            ? onFetchAnthropicModels
            : activeCliType === 'gemini'
              ? onFetchGeminiModels
              : null;
    const catalogStatusMessage =
        activeCliType === 'claude'
            ? getCatalogStatusMessage('Anthropic', anthropicModelsList, canFetchAnthropicModels)
            : activeCliType === 'gemini'
              ? getCatalogStatusMessage('Gemini', geminiModelsList, canFetchGeminiModels)
              : activeCliType === 'codex'
                ? getModelHint('codex')
                : '';

    const updateRole = (roleId: string, patch: Partial<TeamRoleSetting>) => {
        const nextRoles = normalizeRoles(
            config.roles.map((role) => (role.id === roleId ? { ...role, ...patch } : role))
        );
        onChange({ ...config, roles: nextRoles });
    };

    const updateRoleCliType = (roleId: string, cliType: SupportedCliType) => {
        updateRole(roleId, {
            cli_type: cliType,
            model: getDefaultModel(cliType),
        });
    };

    const handleAddRole = () => {
        const newRole = createEmptyRole(getDefaultNewRoleCliType(installedCliMap));
        const nextRoles = normalizeRoles([
            ...config.roles,
            newRole,
        ]);
        setActiveRoleId(newRole.id);
        onChange({ ...config, roles: nextRoles });
    };

    const handleRemoveRole = (roleId: string) => {
        const nextRoles = normalizeRoles(config.roles.filter((role) => role.id !== roleId));
        setActiveRoleId(nextRoles[0]?.id ?? null);
        onChange({ ...config, roles: nextRoles });
    };

    const handleConcurrencyChange = (value: number) => {
        const normalizedValue = Number.isFinite(value)
            ? Math.min(5, Math.max(1, value))
            : 5;
        onChange({
            ...config,
            max_concurrent_agents: normalizedValue,
        });
    };

    const handleAvatarChange = (roleId: string, event: ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = () => {
            if (typeof reader.result === 'string') {
                updateRole(roleId, { avatar_image: reader.result });
            }
        };

        reader.readAsDataURL(file);
        event.target.value = '';
    };

    if (isLoading) {
        return (
            <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-8 text-sm text-slate-500">
                チーム設定を読み込んでいます...
            </div>
        );
    }

    return (
        <div className="space-y-5">
            {!embedded && (
                <div className="rounded-xl border border-slate-200 bg-white p-5">
                    <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
                        <div className="min-w-0 flex-1">
                            <div className="inline-flex items-center gap-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-400">
                                <TerminalSquare size={14} />
                                Multi-CLI Agent Team
                            </div>

                            <div className="mt-3 flex items-start gap-3">
                                <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-slate-100 text-slate-500">
                                    <Users size={20} />
                                </div>
                                <div className="min-w-0">
                                    <h3 className="text-lg font-semibold text-slate-900">
                                        自律エージェントチームを編成する
                                    </h3>
                                    <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600">
                                        本システムは Claude Code CLI / Gemini CLI / Codex CLI を自律エージェントとして束ね、
                                        あなた専属の開発チームを編成・指揮するための心臓部です。ロール、CLI 種別、モデル、並行数を整えることで、
                                        複数の専門家が分担して走るような開発体験を再現できます。
                                    </p>
                                </div>
                            </div>

                            <div className="mt-4 flex flex-wrap gap-2">
                                <span className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-600">
                                    Parallel Team Simulation
                                </span>
                                <span className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-600">
                                    CLI-native Execution
                                </span>
                                <span className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-600">
                                    Role-driven Delivery
                                </span>
                            </div>
                        </div>

                        <div className="grid min-w-[220px] gap-3 sm:grid-cols-2 lg:grid-cols-1 xl:grid-cols-2">
                            <div className="rounded-xl border border-white/70 bg-white/80 p-4 shadow-sm">
                                <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                    Templates
                                </div>
                                <div className="mt-2 text-2xl font-semibold text-slate-900">{roleCount}</div>
                                <div className="mt-1 text-sm text-slate-500">登録ロール数</div>
                            </div>
                            <div className="rounded-xl border border-white/70 bg-white/80 p-4 shadow-sm">
                                <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                    Throughput
                                </div>
                                <div className="mt-2 text-2xl font-semibold text-slate-900">
                                    {config.max_concurrent_agents}
                                </div>
                                <div className="mt-1 text-sm text-slate-500">最大並行稼働数</div>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            <div className="rounded-xl border border-slate-200 bg-white p-5 shadow-sm">
                <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                    <div className="max-w-3xl">
                        <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">
                            01 Global Control
                        </div>
                        <h3 className="mt-2 text-sm font-semibold text-slate-900">最大並行稼働数</h3>
                        <p className="mt-1 text-sm leading-6 text-slate-600">
                            同時に動かせる Dev エージェント数の上限を 1〜5 の範囲で設定します。登録ロール数とは独立した、システム全体のスループット制御です。
                        </p>
                    </div>

                    <div className="rounded-xl bg-slate-100 px-4 py-3 text-lg font-semibold text-slate-800">
                        {config.max_concurrent_agents}
                    </div>
                </div>

                <div className="mt-5 space-y-4">
                    <input
                        type="range"
                        min={1}
                        max={5}
                        step={1}
                        value={config.max_concurrent_agents}
                        onChange={(e) => handleConcurrencyChange(Number(e.target.value))}
                        className="w-full accent-blue-600"
                    />

                    <div className="flex items-center justify-between text-xs font-medium text-slate-500">
                        <span>1 agent</span>
                        <span>5 agents</span>
                    </div>

                    <div className="grid gap-3 md:grid-cols-[112px_minmax(0,1fr)] md:items-start">
                        <input
                            type="number"
                            min={1}
                            max={5}
                            value={config.max_concurrent_agents}
                            onChange={(e) => handleConcurrencyChange(Number(e.target.value))}
                            className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                        />
                        <p className="text-sm leading-6 text-slate-500">
                            ロールはテンプレートとして再利用され、同一ロールから複数エージェントを起動できます。並行数は「何人同時に走らせるか」を決めるスイッチです。
                        </p>
                    </div>
                </div>
            </div>

            {validationMessages.length > 0 && (
                <div className="rounded-xl border border-amber-200 bg-amber-50 px-4 py-3">
                    <p className="text-sm font-medium text-amber-800">保存前に以下を確認してください。</p>
                    <ul className="mt-2 list-disc pl-5 text-sm text-amber-700">
                        {validationMessages.map((message) => (
                            <li key={message}>{message}</li>
                        ))}
                    </ul>
                </div>
            )}

            <div className="grid gap-5 xl:grid-cols-[300px_minmax(0,1fr)]">
                <div className="rounded-xl border border-slate-200 bg-slate-50/70 p-4">
                    <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">
                        03 Role Templates
                    </div>
                    <div className="mt-2 flex items-center justify-between gap-3">
                        <h3 className="text-sm font-semibold text-slate-900">ロール一覧</h3>
                        <span className="rounded-full border border-slate-200 bg-white px-2.5 py-1 text-xs font-medium text-slate-600">
                            {roleCount} roles
                        </span>
                    </div>
                    <p className="mt-2 text-sm leading-6 text-slate-500">
                        登録済みロールを左から選び、右側で内容を編集します。サブメニューではなく、チーム構成そのものを扱うデータリストとして整理しています。
                    </p>

                    <div className="mt-4 space-y-3">
                        {config.roles.length === 0 && (
                            <div className="rounded-xl border border-dashed border-slate-300 bg-white px-4 py-5 text-sm text-slate-500">
                                まだロールは定義されていません。最初のロールを追加してチームを組み立てましょう。
                            </div>
                        )}

                        {config.roles.map((role) => {
                            const cliType = normalizeCliType(role.cli_type);
                            const cliMeta = getCliOptionMeta(cliType, cliResults, isCliDetectionLoading);
                            const isActive = activeRole?.id === role.id;

                            return (
                                <button
                                    key={role.id}
                                    type="button"
                                    onClick={() => setActiveRoleId(role.id)}
                                    className={`w-full rounded-xl border p-3 text-left transition ${
                                        isActive
                                            ? 'border-blue-300 bg-blue-50/80 shadow-sm'
                                            : 'border-slate-200 bg-white hover:border-slate-300 hover:bg-slate-50'
                                    }`}
                                >
                                    <div className="flex items-center gap-3">
                                        <div className="flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden rounded-xl border border-slate-200 bg-white shadow-sm">
                                            <img
                                                src={getRoleAvatarSource(role.avatar_image)}
                                                alt={role.name.trim() || '未設定のロール'}
                                                className="h-full w-full object-contain p-1.5"
                                            />
                                        </div>

                                        <div className="min-w-0 flex-1">
                                            <div className="flex items-center justify-between gap-2">
                                                <div className="truncate text-sm font-semibold text-slate-900">
                                                    {role.name.trim() || '未設定のロール'}
                                                </div>
                                                <span className="shrink-0 rounded-full border border-slate-200 bg-white px-2.5 py-1 text-[11px] font-medium text-slate-600">
                                                    {cliMeta.label}
                                                </span>
                                            </div>

                                            <div className="mt-2">
                                                <span className="inline-flex rounded-full border border-slate-200 bg-white px-2.5 py-1 text-[11px] font-medium text-slate-600">
                                                    {role.model || getDefaultModel(cliType)}
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </button>
                            );
                        })}

                        <Button
                            type="button"
                            variant="secondary"
                            onClick={handleAddRole}
                            className="w-full justify-center border border-dashed border-slate-300 bg-white text-slate-700 hover:bg-slate-100"
                        >
                            <Plus size={16} className="mr-2" />
                            ロールを新規追加
                        </Button>
                    </div>
                </div>

                <div className="rounded-xl border border-slate-200 bg-white p-5 shadow-sm">
                    {!activeRole || !activeCliType || !activeCliMeta ? (
                        <div className="flex min-h-[360px] flex-col items-center justify-center rounded-xl border border-dashed border-slate-300 bg-slate-50 px-6 py-10 text-center">
                            <h4 className="text-base font-semibold text-slate-900">編集するロールを選択してください</h4>
                            <p className="mt-2 max-w-md text-sm leading-6 text-slate-500">
                                左のリストから既存ロールを選ぶか、新しいロールを追加すると詳細設定がここに表示されます。
                            </p>
                        </div>
                    ) : (
                        <>
                            <div className="flex flex-col gap-3 border-b border-slate-200 pb-5 sm:flex-row sm:items-start sm:justify-between">
                                <div>
                                    <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">
                                        Role Detail
                                    </div>
                                    <h3 className="mt-2 text-lg font-semibold text-slate-900">
                                        {activeRole.name.trim() || '未設定のロール'}
                                    </h3>
                                    <p className="mt-1 text-sm text-slate-500">
                                        左のロール一覧から選択中のエージェント設定を編集しています。
                                    </p>
                                </div>

                                <Button
                                    type="button"
                                    variant="ghost"
                                    onClick={() => handleRemoveRole(activeRole.id)}
                                    className="text-red-600 hover:bg-red-50 hover:text-red-700"
                                >
                                    <Trash2 size={15} className="mr-2" />
                                    このロールを削除
                                </Button>
                            </div>

                            <div className="space-y-6 pt-6">
                                <section>
                                    <div className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-400">
                                        Identity
                                    </div>
                                    <h4 className="mt-1 text-sm font-semibold text-slate-900">アイデンティティ</h4>
                                    <p className="mt-1 text-sm text-slate-500">
                                        アバター画像と役職名を横に並べ、誰の設定かすぐ分かるようにします。
                                    </p>

                                    <div className="mt-4 flex flex-col gap-4 md:flex-row md:items-center">
                                        <div className="flex h-24 w-24 shrink-0 items-center justify-center overflow-hidden rounded-2xl border border-slate-200 bg-slate-50">
                                            <img
                                                src={getRoleAvatarSource(activeRole.avatar_image)}
                                                alt={activeRole.name.trim() || '未設定のロール'}
                                                className="h-full w-full object-contain p-2"
                                            />
                                        </div>

                                        <div className="min-w-0 flex-1">
                                            <label className="block text-sm font-medium text-slate-700">役職名</label>
                                            <input
                                                type="text"
                                                value={activeRole.name}
                                                onChange={(e) => updateRole(activeRole.id, { name: e.target.value })}
                                                placeholder="例: Lead Engineer"
                                                className="mt-2 w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                            />
                                            <p className="mt-2 text-sm text-slate-500">
                                                ロール名はチーム編成時の表示名になります。責務が伝わる短い名前にすると分かりやすくなります。
                                            </p>
                                        </div>
                                    </div>

                                    <div className="mt-4 flex flex-wrap gap-2">
                                        <label
                                            htmlFor={avatarInputId}
                                            className="inline-flex cursor-pointer items-center justify-center rounded-xl border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50"
                                        >
                                            <ImagePlus size={15} className="mr-2" />
                                            画像を変更
                                        </label>
                                        <Button
                                            type="button"
                                            variant="ghost"
                                            onClick={() => updateRole(activeRole.id, { avatar_image: null })}
                                        >
                                            <RotateCcw size={15} className="mr-2" />
                                            デフォルトに戻す
                                        </Button>
                                    </div>

                                    <input
                                        id={avatarInputId}
                                        type="file"
                                        accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                                        className="hidden"
                                        onChange={(event) => handleAvatarChange(activeRole.id, event)}
                                    />
                                </section>

                                <section className="border-t border-slate-200 pt-6">
                                    <div className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-400">
                                        Engine Setup
                                    </div>
                                    <h4 className="mt-1 text-sm font-semibold text-slate-900">エンジン設定</h4>
                                    <p className="mt-1 text-sm text-slate-500">
                                        CLI 種別とモデル名を上から順に整えることで、ロールごとの実行特性をコントロールできます。
                                    </p>

                                    <div className="mt-4 grid gap-5 xl:grid-cols-[240px_minmax(0,1fr)] xl:items-start">
                                        <div>
                                            <label className="mb-2 flex items-center gap-2 text-sm font-medium text-slate-700">
                                                <Bot size={14} />
                                                CLI種別
                                            </label>
                                            <div className="inline-flex w-full rounded-xl border border-slate-200 bg-slate-100 p-1">
                                                {CLI_OPTIONS.map((option) => {
                                                    const selected = activeCliType === option.value;

                                                    return (
                                                        <button
                                                            key={option.value}
                                                            type="button"
                                                            onClick={() => updateRoleCliType(activeRole.id, option.value)}
                                                            className={`flex-1 rounded-lg px-3 py-2 text-sm transition ${
                                                                selected
                                                                    ? 'bg-white text-slate-900 shadow-sm ring-2 ring-blue-500'
                                                                    : 'text-slate-600 hover:bg-white'
                                                            }`}
                                                        >
                                                            {option.label}
                                                        </button>
                                                    );
                                                })}
                                            </div>
                                            <p className="mt-2 text-xs leading-5 text-slate-500">{activeCliMeta.detail}</p>
                                        </div>

                                        <div>
                                            <label className="mb-2 flex items-center gap-2 text-sm font-medium text-slate-700">
                                                <Cpu size={14} />
                                                {getModelLabel(activeCliType)}
                                            </label>
                                            <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_120px] md:items-start">
                                                <div>
                                                    {hasSelectableModels ? (
                                                        <>
                                                            <input
                                                                list={modelSuggestionsId}
                                                                value={activeRole.model}
                                                                onChange={(e) => updateRole(activeRole.id, { model: e.target.value })}
                                                                placeholder={getModelPlaceholder(activeCliType)}
                                                                className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                                            />
                                                            <datalist id={modelSuggestionsId}>
                                                                {selectableModels.map((model) => (
                                                                    <option key={model} value={model} />
                                                                ))}
                                                            </datalist>
                                                        </>
                                                    ) : (
                                                        <input
                                                            type="text"
                                                            value={activeRole.model}
                                                            onChange={(e) => updateRole(activeRole.id, { model: e.target.value })}
                                                            placeholder={getModelPlaceholder(activeCliType)}
                                                            className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                                        />
                                                    )}
                                                </div>

                                                <div className="md:pt-0.5">
                                                    {onFetchCatalog ? (
                                                        <Button
                                                            type="button"
                                                            size="sm"
                                                            variant="ghost"
                                                            onClick={onFetchCatalog}
                                                            disabled={isFetchingCatalog || !canFetchCatalog}
                                                            className="w-full border border-slate-200 bg-white text-slate-600 hover:bg-slate-50"
                                                        >
                                                            <RefreshCw
                                                                size={14}
                                                                className={`mr-2 ${isFetchingCatalog ? 'animate-spin' : ''}`}
                                                            />
                                                            取得
                                                        </Button>
                                                    ) : (
                                                        <div className="rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-center text-xs font-medium text-slate-600">
                                                            手動指定
                                                        </div>
                                                    )}
                                                </div>
                                            </div>

                                            <div className="mt-2 flex flex-wrap items-center gap-2">
                                                {!isCliDetectionLoading && !installedCliMap[activeCliType] && (
                                                    <span className="rounded-full border border-amber-200 bg-amber-50 px-2.5 py-1 text-xs font-medium text-amber-700">
                                                        この環境では未検出
                                                    </span>
                                                )}
                                                <p className="text-xs leading-5 text-slate-500">{catalogStatusMessage}</p>
                                            </div>

                                            <p className="mt-2 text-xs leading-5 text-slate-500">
                                                {hasSelectableModels
                                                    ? `候補から選択するか、モデル ID を直接入力できます。${getModelHint(activeCliType)}`
                                                    : getModelHint(activeCliType)}
                                            </p>
                                        </div>
                                    </div>
                                </section>

                                <section className="border-t border-slate-200 pt-6">
                                    <div className="text-xs font-semibold uppercase tracking-[0.16em] text-slate-400">
                                        System Prompt
                                    </div>
                                    <h4 className="mt-1 text-sm font-semibold text-slate-900">システムプロンプト</h4>
                                    <p className="mt-1 text-sm text-slate-500">
                                        役割の責務、期待する出力、レビュー観点をここで定義します。必要な時だけ縦方向へ広げて編集できます。
                                    </p>

                                    <textarea
                                        value={activeRole.system_prompt}
                                        onChange={(e) => updateRole(activeRole.id, { system_prompt: e.target.value })}
                                        placeholder="このロールの責務、出力方針、レビュー観点を記述してください"
                                        rows={5}
                                        className="mt-4 min-h-[120px] w-full resize-y rounded-xl border border-slate-300 px-3 py-2 text-sm leading-6 text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                    />
                                </section>
                            </div>
                        </>
                    )}
                </div>
            </div>
        </div>
    );
}
