import { Cpu, Plus, RefreshCw, Trash2, Users } from 'lucide-react';
import { Button } from './Button';
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
    const availableSlots = Math.max(0, roleCount);

    const updateRole = (roleId: string, patch: Partial<TeamRoleSetting>) => {
        const nextRoles = normalizeRoles(
            config.roles.map(role => role.id === roleId ? { ...role, ...patch } : role)
        );
        onChange({ ...config, roles: nextRoles });
    };

    const handleAddRole = () => {
        const nextRoles = normalizeRoles([...config.roles, createEmptyRole()]);
        onChange({ ...config, roles: nextRoles });
    };

    const handleRemoveRole = (roleId: string) => {
        const nextRoles = normalizeRoles(config.roles.filter(role => role.id !== roleId));
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
            <div className="rounded-lg border border-gray-200 bg-gray-50 px-4 py-8 text-sm text-gray-500">
                チーム設定を読み込んでいます...
            </div>
        );
    }

    return (
        <div className="space-y-6">
            <div className="rounded-xl border border-blue-100 bg-gradient-to-br from-blue-50 to-white p-4">
                <div className="flex items-start gap-3">
                    <div className="rounded-lg bg-blue-100 p-2 text-blue-700">
                        <Users size={18} />
                    </div>
                    <div className="flex-1">
                        <h3 className="font-medium text-gray-900">Devチーム構成</h3>
                        <p className="mt-1 text-sm text-gray-600">
                            将来のマルチエージェント実行で利用する役割、システムプロンプト、Claude モデルを管理します。
                        </p>
                    </div>
                </div>
            </div>

            <div className="rounded-lg border border-gray-200 bg-white p-4">
                <div className="flex items-center justify-between gap-3">
                    <div>
                        <h3 className="font-medium text-gray-900">最大並行稼働数</h3>
                        <p className="mt-1 text-sm text-gray-500">
                            同時に動かせる Dev エージェント数を 1〜5 の範囲で設定します。
                        </p>
                    </div>
                    <div className="rounded-lg bg-gray-100 px-3 py-2 text-sm font-semibold text-gray-700">
                        {config.max_concurrent_agents}
                    </div>
                </div>

                <div className="mt-4 space-y-3">
                    <input
                        type="range"
                        min={1}
                        max={5}
                        step={1}
                        value={config.max_concurrent_agents}
                        onChange={(e) => handleConcurrencyChange(Number(e.target.value))}
                        className="w-full accent-blue-600"
                    />
                    <div className="flex items-center justify-between gap-4">
                        <input
                            type="number"
                            min={1}
                            max={5}
                            value={config.max_concurrent_agents}
                            onChange={(e) => handleConcurrencyChange(Number(e.target.value))}
                            className="w-24 rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                        />
                        <div className="text-right text-sm text-gray-500">
                            登録ロール数: {roleCount} / 利用可能枠: {availableSlots}
                        </div>
                    </div>
                </div>
            </div>

            <div className="rounded-lg border border-gray-200 bg-white p-4">
                <div className="flex items-start justify-between gap-3">
                    <div>
                        <h3 className="font-medium text-gray-900">Claude モデル一覧</h3>
                        <p className="mt-1 text-sm text-gray-500">
                            Anthropic API から利用可能なモデル一覧を取得し、各ロールのモデル選択に利用します。
                        </p>
                    </div>
                    <button
                        type="button"
                        onClick={onFetchModels}
                        disabled={isFetchingModels || !canFetchModels}
                        className="text-xs text-blue-600 hover:text-blue-800 flex items-center gap-1 disabled:opacity-50"
                    >
                        <RefreshCw size={12} className={isFetchingModels ? 'animate-spin' : ''} />
                        モデル一覧を取得
                    </button>
                </div>

                <div className="mt-3 text-sm text-gray-600">
                    {anthropicModelsList.length > 0 ? (
                        <span>{anthropicModelsList.length} 件の Claude モデルを取得済みです。</span>
                    ) : (
                        <span>モデル一覧は未取得です。Anthropic API Key を設定してから取得してください。</span>
                    )}
                </div>
            </div>

            {validationMessages.length > 0 && (
                <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
                    <p className="text-sm font-medium text-amber-800">保存前に以下を確認してください。</p>
                    <ul className="mt-2 list-disc pl-5 text-sm text-amber-700">
                        {validationMessages.map(message => (
                            <li key={message}>{message}</li>
                        ))}
                    </ul>
                </div>
            )}

            <div className="space-y-4">
                {config.roles.map((role, index) => (
                    <div key={role.id} className="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
                        <div className="flex items-start justify-between gap-3">
                            <div>
                                <div className="text-xs font-semibold uppercase tracking-wide text-blue-600">
                                    Role {index + 1}
                                </div>
                                <h3 className="mt-1 text-sm font-medium text-gray-900">
                                    {role.name.trim() || '未設定のロール'}
                                </h3>
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

                        <div className="mt-4 grid gap-4 md:grid-cols-[minmax(0,1fr)_240px]">
                            <div>
                                <label className="mb-1 block text-sm text-gray-600">役割名</label>
                                <input
                                    type="text"
                                    value={role.name}
                                    onChange={(e) => updateRole(role.id, { name: e.target.value })}
                                    placeholder="例: Frontend Dev"
                                    className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                                />
                            </div>

                            <div>
                                <label className="mb-1 flex items-center gap-2 text-sm text-gray-600">
                                    <Cpu size={14} />
                                    Claude モデル
                                </label>
                                {anthropicModelsList.length > 0 ? (
                                    <select
                                        value={role.model}
                                        onChange={(e) => updateRole(role.id, { model: e.target.value })}
                                        className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white"
                                    >
                                        {anthropicModelsList.map(model => (
                                            <option key={model} value={model}>{model}</option>
                                        ))}
                                    </select>
                                ) : (
                                    <input
                                        type="text"
                                        value={role.model}
                                        onChange={(e) => updateRole(role.id, { model: e.target.value })}
                                        placeholder="Anthropic のモデル一覧を取得してください"
                                        className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                                    />
                                )}
                            </div>
                        </div>

                        <div className="mt-4">
                            <label className="mb-1 block text-sm text-gray-600">システムプロンプト</label>
                            <textarea
                                value={role.system_prompt}
                                onChange={(e) => updateRole(role.id, { system_prompt: e.target.value })}
                                placeholder="このロールの責務、出力方針、レビュー観点を記述してください"
                                rows={4}
                                className="w-full rounded-md border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500"
                            />
                        </div>
                    </div>
                ))}
            </div>

            <Button type="button" variant="secondary" onClick={handleAddRole} className="w-full bg-white">
                <Plus size={16} className="mr-2" />
                ロールを追加
            </Button>
        </div>
    );
}
