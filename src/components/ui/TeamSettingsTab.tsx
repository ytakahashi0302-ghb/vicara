import { Cpu, Plus, RefreshCw, TerminalSquare, Trash2, Users } from 'lucide-react';
import { Button } from './Button';
import { AvatarImageField } from './AvatarImageField';
import { TeamConfiguration, TeamRoleSetting } from '../../types';

interface TeamSettingsTabProps {
    config: TeamConfiguration;
    validationMessages: string[];
    isLoading: boolean;
    anthropicModelsList: string[];
    isFetchingModels: boolean;
    canFetchModels: boolean;
    onChange: (config: TeamConfiguration) => void;
    onFetchModels: () => void;
}

function createEmptyRole(): TeamRoleSetting {
    return {
        id: crypto.randomUUID(),
        name: '',
        system_prompt: '',
        model: 'claude-3-5-sonnet-20241022',
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

export function TeamSettingsTab({
    config,
    validationMessages,
    isLoading,
    anthropicModelsList,
    isFetchingModels,
    canFetchModels,
    onChange,
    onFetchModels,
}: TeamSettingsTabProps) {
    const roleCount = config.roles.length;
    const hasFetchedModels = anthropicModelsList.length > 0;

    const updateRole = (roleId: string, patch: Partial<TeamRoleSetting>) => {
        const nextRoles = normalizeRoles(
            config.roles.map((role) => (role.id === roleId ? { ...role, ...patch } : role))
        );
        onChange({ ...config, roles: nextRoles });
    };

    const handleAddRole = () => {
        const nextRoles = normalizeRoles([...config.roles, createEmptyRole()]);
        onChange({ ...config, roles: nextRoles });
    };

    const handleRemoveRole = (roleId: string) => {
        const nextRoles = normalizeRoles(config.roles.filter((role) => role.id !== roleId));
        onChange({ ...config, roles: nextRoles });
    };

    const handleConcurrencyChange = (value: number) => {
        onChange({
            ...config,
            max_concurrent_agents: value,
        });
    };

    if (isLoading) {
        return (
            <div className="rounded-2xl border border-slate-200 bg-slate-50 px-4 py-8 text-sm text-slate-500">
                チーム設定を読み込んでいます...
            </div>
        );
    }

    return (
        <div className="space-y-5">
            <div className="rounded-2xl border border-sky-200 bg-gradient-to-br from-sky-50 via-white to-indigo-50 p-5 shadow-sm">
                <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
                    <div className="min-w-0 flex-1">
                        <div className="inline-flex items-center gap-2 rounded-full border border-sky-200 bg-white/80 px-3 py-1 text-xs font-semibold text-sky-700 shadow-sm">
                            <TerminalSquare size={14} />
                            Claude Code CLI Powered
                        </div>

                        <div className="mt-4 flex items-start gap-3">
                            <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-sky-100 text-sky-700">
                                <Users size={22} />
                            </div>
                            <div className="min-w-0">
                                <h3 className="text-lg font-semibold text-slate-900">
                                    自律エージェントチームを編成する
                                </h3>
                                <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600">
                                    本システムは Claude Code CLI を自律エージェントとして束ね、あなた専属の開発チームを編成・指揮するための心臓部です。
                                    ロール、モデル、並行数を整えることで、複数の専門家が分担して走るような開発体験を再現できます。
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
                        <div className="rounded-2xl border border-white/70 bg-white/80 p-4 shadow-sm">
                            <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                Templates
                            </div>
                            <div className="mt-2 text-2xl font-semibold text-slate-900">{roleCount}</div>
                            <div className="mt-1 text-sm text-slate-500">登録ロール数</div>
                        </div>
                        <div className="rounded-2xl border border-white/70 bg-white/80 p-4 shadow-sm">
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

            <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                    <div className="max-w-3xl">
                        <div className="text-xs font-semibold uppercase tracking-[0.18em] text-sky-600">
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
                        className="w-full accent-sky-600"
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
                            className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                        />
                        <p className="text-sm leading-6 text-slate-500">
                            ロールはテンプレートとして再利用され、同一ロールから複数エージェントを起動できます。並行数は「何人同時に走らせるか」を決めるスイッチです。
                        </p>
                    </div>
                </div>
            </div>

            <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                    <div className="max-w-3xl">
                        <div className="text-xs font-semibold uppercase tracking-[0.18em] text-sky-600">
                            02 API Readiness
                        </div>
                        <h3 className="mt-2 text-sm font-semibold text-slate-900">Claude モデル一覧</h3>
                        <p className="mt-1 text-sm leading-6 text-slate-600">
                            Anthropic API から利用可能なモデル一覧を取得し、各ロールのモデル選択肢として利用します。事前に API Key を設定しておくとスムーズです。
                        </p>
                    </div>

                    <button
                        type="button"
                        onClick={onFetchModels}
                        disabled={isFetchingModels || !canFetchModels}
                        className="inline-flex items-center justify-center gap-2 rounded-md border border-sky-200 bg-sky-50 px-3 py-2 text-sm font-medium text-sky-700 transition-colors hover:bg-sky-100 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                        <RefreshCw size={14} className={isFetchingModels ? 'animate-spin' : ''} />
                        モデル一覧を取得
                    </button>
                </div>

                <div
                    className={`mt-4 rounded-xl border px-4 py-3 text-sm ${
                        hasFetchedModels
                            ? 'border-emerald-200 bg-emerald-50/70 text-emerald-700'
                            : 'border-slate-200 bg-slate-50 text-slate-600'
                    }`}
                >
                    {hasFetchedModels ? (
                        <span>{anthropicModelsList.length} 件の Claude モデルを取得済みです。ロールごとに最適なモデルを割り当てられます。</span>
                    ) : (
                        <span>モデル一覧は未取得です。Anthropic API Key を設定してから取得してください。</span>
                    )}
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

            <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div className="max-w-3xl">
                        <div className="text-xs font-semibold uppercase tracking-[0.18em] text-sky-600">
                            03 Role Templates
                        </div>
                        <h3 className="mt-2 text-sm font-semibold text-slate-900">テンプレート定義</h3>
                        <p className="mt-1 text-sm leading-6 text-slate-600">
                            役割ごとの責務、システムプロンプト、Claude モデルを定義します。ここで作成したテンプレートを基に、実行時に複数エージェントが編成されます。
                        </p>
                    </div>

                    <Button
                        type="button"
                        variant="secondary"
                        onClick={handleAddRole}
                        className="border border-dashed border-slate-300 bg-slate-50 text-slate-700 hover:bg-slate-100"
                    >
                        <Plus size={16} className="mr-2" />
                        ロールを追加
                    </Button>
                </div>

                <div className="mt-5 space-y-4">
                    {config.roles.length === 0 && (
                        <div className="rounded-2xl border border-dashed border-slate-300 bg-slate-50 px-4 py-6 text-sm text-slate-500">
                            まだロールは定義されていません。最初のテンプレートを追加して、チーム構成を組み立てましょう。
                        </div>
                    )}

                    {config.roles.map((role, index) => (
                        <div key={role.id} className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                            <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                                <div className="min-w-0">
                                    <div className="text-[11px] font-semibold uppercase tracking-[0.16em] text-sky-600">
                                        Template {index + 1}
                                    </div>
                                    <h3 className="mt-1 text-base font-semibold text-slate-900">
                                        {role.name.trim() || '未設定のロール'}
                                    </h3>
                                    <p className="mt-1 text-sm text-slate-500">
                                        実行時にはこのテンプレートが担当エージェントの役割、モデル、システム指示として利用されます。
                                    </p>
                                </div>

                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => handleRemoveRole(role.id)}
                                    className="text-red-600 hover:bg-red-50 hover:text-red-700"
                                >
                                    <Trash2 size={14} className="mr-1" />
                                    削除
                                </Button>
                            </div>

                            <div className="mt-5 grid gap-4 md:grid-cols-[minmax(0,1fr)_260px]">
                                <div>
                                    <label className="mb-1 block text-sm font-medium text-slate-700">役割名</label>
                                    <input
                                        type="text"
                                        value={role.name}
                                        onChange={(e) => updateRole(role.id, { name: e.target.value })}
                                        placeholder="例: Frontend Dev"
                                        className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                                    />
                                </div>

                                <div>
                                    <label className="mb-1 flex items-center gap-2 text-sm font-medium text-slate-700">
                                        <Cpu size={14} />
                                        Claude モデル
                                    </label>
                                    {hasFetchedModels ? (
                                        <select
                                            value={role.model}
                                            onChange={(e) => updateRole(role.id, { model: e.target.value })}
                                            className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                                        >
                                            {anthropicModelsList.map((model) => (
                                                <option key={model} value={model}>
                                                    {model}
                                                </option>
                                            ))}
                                        </select>
                                    ) : (
                                        <input
                                            type="text"
                                            value={role.model}
                                            onChange={(e) => updateRole(role.id, { model: e.target.value })}
                                            placeholder="Anthropic のモデル一覧を取得してください"
                                            className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                                        />
                                    )}
                                </div>
                            </div>

                            <div className="mt-4">
                                <AvatarImageField
                                    label="Dev-agent アバター画像"
                                    description="このテンプレートから起動される Dev-agent の表示画像です。未設定時は標準の Dev-agent 画像を使用します。"
                                    value={role.avatar_image ?? null}
                                    fallbackKind="dev-agent"
                                    previewMode="avatar"
                                    onChange={(value) => updateRole(role.id, { avatar_image: value })}
                                />
                            </div>

                            <div className="mt-4">
                                <label className="mb-1 block text-sm font-medium text-slate-700">
                                    システムプロンプト
                                </label>
                                <textarea
                                    value={role.system_prompt}
                                    onChange={(e) => updateRole(role.id, { system_prompt: e.target.value })}
                                    placeholder="このロールの責務、出力方針、レビュー観点を記述してください"
                                    rows={5}
                                    className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                                />
                            </div>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}
