import { useState, useEffect } from 'react';
import { Modal } from './Modal';
import { Settings, Trash2, Shield, RefreshCw, AlertTriangle, Coins } from 'lucide-react';
import { Button } from './Button';
import { useWorkspace } from '../../context/WorkspaceContext';
import { invoke } from '@tauri-apps/api/core';
import { load } from '@tauri-apps/plugin-store';
import { confirm, open } from '@tauri-apps/plugin-dialog';
import toast from 'react-hot-toast';
import { TeamSettingsTab } from './TeamSettingsTab';
import { TeamConfiguration } from '../../types';
import { useLlmUsageSummary } from '../../hooks/useLlmUsageSummary';
import {
    normalizeStoredStringValue,
    PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY,
    VICARA_SETTINGS_UPDATED_EVENT,
} from '../../hooks/usePoAssistantAvatarImage';
import { AvatarImageField } from './AvatarImageField';

interface GlobalSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
}

const DEFAULT_TEAM_CONFIGURATION: TeamConfiguration = {
    max_concurrent_agents: 1,
    roles: [],
};

function validateTeamConfiguration(config: TeamConfiguration): string[] {
    const messages: string[] = [];

    if (!Number.isInteger(config.max_concurrent_agents) || config.max_concurrent_agents < 1 || config.max_concurrent_agents > 5) {
        messages.push('最大並行稼働数は 1〜5 の範囲で設定してください。');
    }

    if (config.roles.length === 0) {
        messages.push('ロールを最低 1 件追加してください。');
    }

    config.roles.forEach((role, index) => {
        if (!role.name.trim()) {
            messages.push(`Role ${index + 1} の役割名を入力してください。`);
        }
        if (!role.model.trim()) {
            messages.push(`Role ${index + 1} の Claude モデルを入力してください。`);
        }
        if (!role.system_prompt.trim()) {
            messages.push(`Role ${index + 1} のシステムプロンプトを入力してください。`);
        }
    });

    return Array.from(new Set(messages));
}

function formatTokenCount(value: number) {
    return new Intl.NumberFormat('ja-JP').format(value);
}

function formatEstimatedCost(value: number) {
    return `~$${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : value >= 1 ? 2 : 3)}`;
}

function formatSourceLabel(sourceKind: string) {
    const labels: Record<string, string> = {
        idea_refine: 'Idea',
        task_generation: 'Task Gen',
        inception: 'Inception',
        team_leader: 'POアシスタント',
        task_execution: 'Task Exec',
        scaffold_ai: 'Scaffold',
    };

    return labels[sourceKind] ?? sourceKind;
}

export function GlobalSettingsModal({ isOpen, onClose }: GlobalSettingsModalProps) {
    const { currentProjectId, deleteProject, projects, updateProjectPath } = useWorkspace();
    const currentProject = projects.find(p => p.id === currentProjectId);
    const {
        summary: usageSummary,
        loading: usageLoading,
        error: usageError,
    } = useLlmUsageSummary(currentProjectId);
    
    // Tabs state
    const [activeTab, setActiveTab] = useState<'general' | 'ai' | 'team'>('ai');
    
    // AI Settings State
    const [provider, setProvider] = useState<'anthropic' | 'gemini'>('anthropic');
    const [anthropicKey, setAnthropicKey] = useState('');
    const [geminiKey, setGeminiKey] = useState('');
    const [anthropicModel, setAnthropicModel] = useState('');
    const [geminiModel, setGeminiModel] = useState('');
    const [poAssistantAvatarImage, setPoAssistantAvatarImage] = useState<string | null>(null);
    
    // Custom model toggles
    const [isCustomAnthropic, setIsCustomAnthropic] = useState(false);
    const [isCustomGemini, setIsCustomGemini] = useState(false);
    
    // Models list from API
    const [anthropicModelsList, setAnthropicModelsList] = useState<string[]>([]);
    const [geminiModelsList, setGeminiModelsList] = useState<string[]>([]);
    const [isFetchingModels, setIsFetchingModels] = useState(false);
    
    // Path selection state
    const [isSelectingPath, setIsSelectingPath] = useState(false);

    // Team settings state
    const [teamConfig, setTeamConfig] = useState<TeamConfiguration>(DEFAULT_TEAM_CONFIGURATION);
    const [isLoadingTeamConfig, setIsLoadingTeamConfig] = useState(false);
    const [isSaving, setIsSaving] = useState(false);

    // Initial load
    useEffect(() => {
        if (isOpen) {
            loadSettings();
        }
    }, [isOpen]);

    const loadSettings = async () => {
        setIsLoadingTeamConfig(true);
        try {
            const [store, loadedTeamConfig] = await Promise.all([
                load('settings.json'),
                invoke<TeamConfiguration>('get_team_configuration'),
            ]);
            
            const p = await store.get<{ value: string }>('default-ai-provider');
            if (p && p.value === 'gemini') setProvider('gemini');
            else setProvider('anthropic');
            
            const ak = await store.get<{ value: string }>('anthropic-api-key');
            if (ak && ak.value) setAnthropicKey(ak.value);
            else if (typeof ak === 'string') setAnthropicKey(ak);
            else setAnthropicKey('');

            const gk = await store.get<{ value: string }>('gemini-api-key');
            if (gk && gk.value) setGeminiKey(gk.value);
            else if (typeof gk === 'string') setGeminiKey(gk);
            else setGeminiKey('');

            const am = await store.get<{ value: string }>('anthropic-model');
            if (am && am.value) setAnthropicModel(am.value);
            else if (typeof am === 'string') setAnthropicModel(am);
            else setAnthropicModel('claude-3-5-sonnet-latest'); // default

            const gm = await store.get<{ value: string }>('gemini-model');
            if (gm && gm.value) setGeminiModel(gm.value);
            else if (typeof gm === 'string') setGeminiModel(gm);
            else setGeminiModel('gemini-2.5-flash'); // default

            const poAssistantImage = await store.get(PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY);
            setPoAssistantAvatarImage(normalizeStoredStringValue(poAssistantImage));

            setTeamConfig(loadedTeamConfig);
            
        } catch (e) {
            console.error('Failed to load settings', e);
            toast.error(`設定の読み込みに失敗しました: ${e}`);
        } finally {
            setIsLoadingTeamConfig(false);
        }
    };

    const fetchModels = async (targetProvider: 'anthropic' | 'gemini') => {
        setIsFetchingModels(true);
        try {
            const models = await invoke<string[]>('get_available_models', { provider: targetProvider });
            if (targetProvider === 'anthropic') {
                setAnthropicModelsList(models);
                if (models.length > 0 && !isCustomAnthropic && !models.includes(anthropicModel)) {
                    setAnthropicModel(models[0]);
                }
            } else {
                setGeminiModelsList(models);
                if (models.length > 0 && !isCustomGemini && !models.includes(geminiModel)) {
                    setGeminiModel(models[0]);
                }
            }
            toast.success(`${targetProvider === 'anthropic' ? 'Anthropic' : 'Gemini'} のモデル一覧を取得しました`);
        } catch (e) {
            console.error(`Failed to fetch ${targetProvider} models`, e);
            toast.error(`モデルの取得に失敗しました。APIキーを確認してください。\nError: ${e}`);
        } finally {
            setIsFetchingModels(false);
        }
    };

    const teamValidationMessages = validateTeamConfiguration(teamConfig);
    const isSaveDisabled = isSaving || isLoadingTeamConfig || teamValidationMessages.length > 0;

    const handleSave = async () => {
        if (teamValidationMessages.length > 0) {
            toast.error(teamValidationMessages[0]);
            return;
        }

        setIsSaving(true);
        try {
            const store = await load('settings.json');
            await store.set('default-ai-provider', { value: provider });
            await store.set('anthropic-api-key', { value: anthropicKey });
            await store.set('gemini-api-key', { value: geminiKey });
            await store.set('anthropic-model', { value: anthropicModel });
            await store.set('gemini-model', { value: geminiModel });
            await store.set(PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY, { value: poAssistantAvatarImage ?? '' });
            await store.save();
            await invoke('save_team_configuration', { config: teamConfig });
            window.dispatchEvent(new Event(VICARA_SETTINGS_UPDATED_EVENT));
            toast.success('設定を保存しました');
            onClose();
        } catch (e) {
            console.error('Failed to save settings', e);
            toast.error('設定の保存に失敗しました');
        } finally {
            setIsSaving(false);
        }
    };

    const handleDeleteProject = async () => {
        if (!currentProjectId || currentProjectId === 'default') {
            toast.error('このプロジェクトは削除できません');
            return;
        }

        // Tauri dialog plugin の confirm を await で必ず確認を待つ
        const confirmed = await confirm(
            '本当にこのプロジェクトを削除しますか？\n紐づくすべてのバックログやスプリントデータが消去されます。',
            { title: 'プロジェクトの削除確認', kind: 'warning' }
        );
        if (!confirmed) return;

        try {
            await deleteProject(currentProjectId);
            onClose();
        } catch {
            // Error toast は WorkspaceContext 側で処理
        }
    };

    const handleSelectFolder = async () => {
        setIsSelectingPath(true);
        try {
            const selectedPath = await open({
                directory: true,
                multiple: false,
                title: 'プロジェクトのディレクトリを選択してください'
            });

            if (selectedPath && typeof selectedPath === 'string') {
                const result = await updateProjectPath(currentProjectId, selectedPath);
                if (result.success) {
                    toast.success('ワークスペースのディレクトリを設定しました');
                }
            }
        } catch (error) {
            console.error('Failed to select directory:', error);
            toast.error('ディレクトリの選択に失敗しました');
        } finally {
            setIsSelectingPath(false);
        }
    };

    return (
        <Modal
            isOpen={isOpen}
            onClose={onClose}
            width="xl"
            title={
                <div className="flex items-center gap-2 text-gray-900">
                    <Settings size={20} className="text-gray-500" />
                    <span>グローバル設定</span>
                </div>
            }
        >
            <div className="flex border-b border-gray-200 mb-4">
                <button
                    onClick={() => setActiveTab('ai')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'ai' 
                            ? 'border-b-2 border-blue-500 text-blue-600' 
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    POアシスタント設定
                </button>
                <button
                    onClick={() => setActiveTab('general')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'general' 
                            ? 'border-b-2 border-blue-500 text-blue-600' 
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    プロジェクト設定
                </button>
                <button
                    onClick={() => setActiveTab('team')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'team'
                            ? 'border-b-2 border-blue-500 text-blue-600'
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    チーム設定
                </button>
            </div>

            <div className="min-h-[350px] max-h-[60vh] overflow-y-auto px-1 custom-scrollbar">
                {activeTab === 'ai' && (
                    <div className="space-y-6">
                        <AvatarImageField
                            label="POアシスタント画像"
                            description="ヘッダー、チャット、サイドバー右下の立ち絵表示に使用する画像です。未設定時は標準の POアシスタント画像を使用します。"
                            value={poAssistantAvatarImage}
                            fallbackKind="po-assistant"
                            previewMode="figure"
                            onChange={setPoAssistantAvatarImage}
                        />

                        {/* Provider Selection */}
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-2">
                                POアシスタントで使用するデフォルト AI プロバイダー
                            </label>
                            <div className="grid grid-cols-2 gap-3">
                                <label className={`border rounded-lg p-3 cursor-pointer flex items-center transition-all ${
                                    provider === 'anthropic' ? 'border-blue-500 bg-blue-50 ring-1 ring-blue-500' : 'border-gray-200 hover:bg-gray-50'
                                }`}>
                                    <input 
                                        type="radio" 
                                        name="provider" 
                                        value="anthropic" 
                                        checked={provider === 'anthropic'} 
                                        onChange={() => setProvider('anthropic')}
                                        className="hidden"
                                    />
                                    <span className="font-semibold text-gray-800 ml-2">Anthropic (Claude)</span>
                                </label>
                                <label className={`border rounded-lg p-3 cursor-pointer flex items-center transition-all ${
                                    provider === 'gemini' ? 'border-purple-500 bg-purple-50 ring-1 ring-purple-500' : 'border-gray-200 hover:bg-gray-50'
                                }`}>
                                    <input 
                                        type="radio" 
                                        name="provider" 
                                        value="gemini" 
                                        checked={provider === 'gemini'} 
                                        onChange={() => setProvider('gemini')}
                                        className="hidden"
                                    />
                                    <span className="font-semibold text-gray-800 ml-2">Google Gemini</span>
                                </label>
                            </div>
                        </div>

                        {/* Anthropic Settings */}
                        <div className={`p-4 rounded-lg border ${provider === 'anthropic' ? 'border-blue-200 bg-blue-50/30' : 'border-gray-200 opacity-60'}`}>
                            <h3 className="font-medium text-gray-900 flex items-center gap-2 mb-4">
                                <Shield size={16} className="text-gray-500" />
                                Anthropic 設定
                            </h3>
                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm text-gray-600 mb-1">API Key</label>
                                    <input
                                        type="password"
                                        placeholder="sk-ant-api03-..."
                                        value={anthropicKey}
                                        onChange={(e) => setAnthropicKey(e.target.value)}
                                        className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <div className="flex justify-between items-center mb-1">
                                        <label className="block text-sm text-gray-600">Model</label>
                                        <button 
                                            onClick={() => fetchModels('anthropic')}
                                            disabled={isFetchingModels || !anthropicKey}
                                            className="text-xs text-blue-600 hover:text-blue-800 flex items-center gap-1 disabled:opacity-50"
                                        >
                                            <RefreshCw size={12} className={isFetchingModels && provider === 'anthropic' ? "animate-spin" : ""} />
                                            モデル一覧を取得
                                        </button>
                                    </div>
                                    
                                    {!isCustomAnthropic && anthropicModelsList.length > 0 ? (
                                        <select 
                                            value={anthropicModel}
                                            onChange={(e) => setAnthropicModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none mb-2 bg-white"
                                        >
                                            {anthropicModelsList.map(m => <option key={m} value={m}>{m}</option>)}
                                        </select>
                                    ) : (
                                        <input
                                            type="text"
                                            placeholder="claude-3-5-sonnet-latest"
                                            value={anthropicModel}
                                            onChange={(e) => setAnthropicModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none mb-2"
                                        />
                                    )}
                                    <label className="flex items-center gap-2 text-xs text-gray-500 cursor-pointer">
                                        <input type="checkbox" checked={isCustomAnthropic} onChange={(e) => setIsCustomAnthropic(e.target.checked)} className="rounded border-gray-300 text-blue-600 focus:ring-blue-500" />
                                        カスタムモデル名を手動で入力する
                                    </label>
                                </div>
                            </div>
                        </div>

                        {/* Gemini Settings */}
                        <div className={`p-4 rounded-lg border ${provider === 'gemini' ? 'border-purple-200 bg-purple-50/30' : 'border-gray-200 opacity-60'}`}>
                            <h3 className="font-medium text-gray-900 flex items-center gap-2 mb-4">
                                <Shield size={16} className="text-gray-500" />
                                Gemini 設定
                            </h3>
                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm text-gray-600 mb-1">API Key</label>
                                    <input
                                        type="password"
                                        placeholder="AIzaSy..."
                                        value={geminiKey}
                                        onChange={(e) => setGeminiKey(e.target.value)}
                                        className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-purple-500 focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <div className="flex justify-between items-center mb-1">
                                        <label className="block text-sm text-gray-600">Model</label>
                                        <button 
                                            onClick={() => fetchModels('gemini')}
                                            disabled={isFetchingModels || !geminiKey}
                                            className="text-xs text-purple-600 hover:text-purple-800 flex items-center gap-1 disabled:opacity-50"
                                        >
                                            <RefreshCw size={12} className={isFetchingModels && provider === 'gemini' ? "animate-spin" : ""} />
                                            モデル一覧を取得
                                        </button>
                                    </div>

                                    {!isCustomGemini && geminiModelsList.length > 0 ? (
                                        <select 
                                            value={geminiModel}
                                            onChange={(e) => setGeminiModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-purple-500 focus:outline-none mb-2 bg-white"
                                        >
                                            {geminiModelsList.map(m => <option key={m} value={m}>{m}</option>)}
                                        </select>
                                    ) : (
                                        <input
                                            type="text"
                                            placeholder="gemini-2.5-flash"
                                            value={geminiModel}
                                            onChange={(e) => setGeminiModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-purple-500 focus:outline-none mb-2"
                                        />
                                    )}
                                    <label className="flex items-center gap-2 text-xs text-gray-500 cursor-pointer">
                                        <input type="checkbox" checked={isCustomGemini} onChange={(e) => setIsCustomGemini(e.target.checked)} className="rounded border-gray-300 text-purple-600 focus:ring-purple-500" />
                                        カスタムモデル名を手動で入力する
                                    </label>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'general' && (
                    <div className="space-y-6">
                        <div className="rounded-lg border border-emerald-200 bg-emerald-50/40 p-4">
                            <div className="mb-4 flex items-start justify-between gap-3">
                                <div>
                                    <h3 className="flex items-center gap-2 font-medium text-gray-900">
                                        <Coins size={16} className="text-emerald-700" />
                                        LLM Observability
                                    </h3>
                                    <p className="mt-1 text-sm text-gray-600">
                                        プロジェクト全体とアクティブスプリント内での LLM 使用量を確認できます。
                                    </p>
                                </div>
                                <div className="rounded-full border border-emerald-200 bg-white px-3 py-1 text-xs font-semibold text-emerald-700">
                                    {usageLoading ? '更新中...' : '概算コスト'}
                                </div>
                            </div>

                            {usageError ? (
                                <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-800">
                                    usage の取得に失敗しました: {usageError}
                                </div>
                            ) : (
                                <div className="space-y-4">
                                    <div className="grid gap-3 md:grid-cols-3">
                                        <div className="rounded-lg border border-white/70 bg-white px-4 py-3 shadow-sm">
                                            <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                                Project Total
                                            </div>
                                            <div className="mt-1 text-lg font-semibold text-slate-900">
                                                {usageSummary
                                                    ? formatEstimatedCost(usageSummary.project_totals.estimated_cost_usd)
                                                    : '~$0.000'}
                                            </div>
                                            <div className="mt-1 text-sm text-slate-600">
                                                {usageSummary
                                                    ? `${formatTokenCount(usageSummary.project_totals.total_tokens)} token`
                                                    : '0 token'}
                                            </div>
                                        </div>
                                        <div className="rounded-lg border border-white/70 bg-white px-4 py-3 shadow-sm">
                                            <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                                Active Sprint
                                            </div>
                                            <div className="mt-1 text-lg font-semibold text-slate-900">
                                                {usageSummary
                                                    ? formatEstimatedCost(usageSummary.active_sprint_totals.estimated_cost_usd)
                                                    : '~$0.000'}
                                            </div>
                                            <div className="mt-1 text-sm text-slate-600">
                                                {usageSummary
                                                    ? `${formatTokenCount(usageSummary.active_sprint_totals.total_tokens)} token`
                                                    : '0 token'}
                                            </div>
                                        </div>
                                        <div className="rounded-lg border border-white/70 bg-white px-4 py-3 shadow-sm">
                                            <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                                Today
                                            </div>
                                            <div className="mt-1 text-lg font-semibold text-slate-900">
                                                {usageSummary
                                                    ? formatEstimatedCost(usageSummary.today_totals.estimated_cost_usd)
                                                    : '~$0.000'}
                                            </div>
                                            <div className="mt-1 text-sm text-slate-600">
                                                {usageSummary
                                                    ? `${formatTokenCount(usageSummary.today_totals.total_tokens)} token`
                                                    : '0 token'}
                                            </div>
                                        </div>
                                    </div>

                                    {usageSummary && usageSummary.project_totals.unavailable_event_count > 0 && (
                                        <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
                                            Claude CLI など一部実行は厳密 token 未計測です。現在は
                                            <code className="mx-1 rounded bg-amber-100 px-1 py-0.5 text-xs">measurement_status='unavailable'</code>
                                            として保存しています。
                                        </div>
                                    )}

                                    <div className="grid gap-4 lg:grid-cols-2">
                                        <div className="rounded-lg border border-white/70 bg-white p-4 shadow-sm">
                                            <div className="mb-3 text-sm font-semibold text-slate-900">Source別内訳</div>
                                            <div className="space-y-2">
                                                {usageSummary?.by_source.length ? usageSummary.by_source.map((item) => (
                                                    <div key={item.source_kind} className="flex items-center justify-between rounded-md bg-slate-50 px-3 py-2">
                                                        <div className="min-w-0">
                                                            <div className="truncate text-sm font-medium text-slate-800">
                                                                {formatSourceLabel(item.source_kind)}
                                                            </div>
                                                            <div className="text-xs text-slate-500">
                                                                {formatTokenCount(item.total_tokens)} token / {item.event_count} event
                                                            </div>
                                                        </div>
                                                        <div className="shrink-0 text-sm font-semibold text-slate-900">
                                                            {formatEstimatedCost(item.estimated_cost_usd)}
                                                        </div>
                                                    </div>
                                                )) : (
                                                    <div className="rounded-md bg-slate-50 px-3 py-3 text-sm text-slate-500">
                                                        まだ usage データはありません。
                                                    </div>
                                                )}
                                            </div>
                                        </div>

                                        <div className="rounded-lg border border-white/70 bg-white p-4 shadow-sm">
                                            <div className="mb-3 text-sm font-semibold text-slate-900">Model別内訳</div>
                                            <div className="space-y-2">
                                                {usageSummary?.by_model.length ? usageSummary.by_model.map((item) => (
                                                    <div key={`${item.provider}:${item.model}`} className="flex items-center justify-between rounded-md bg-slate-50 px-3 py-2">
                                                        <div className="min-w-0">
                                                            <div className="truncate text-sm font-medium text-slate-800">
                                                                {item.model}
                                                            </div>
                                                            <div className="text-xs uppercase tracking-[0.14em] text-slate-500">
                                                                {item.provider}
                                                            </div>
                                                        </div>
                                                        <div className="shrink-0 text-right">
                                                            <div className="text-sm font-semibold text-slate-900">
                                                                {formatEstimatedCost(item.estimated_cost_usd)}
                                                            </div>
                                                            <div className="text-xs text-slate-500">
                                                                {formatTokenCount(item.total_tokens)} token
                                                            </div>
                                                        </div>
                                                    </div>
                                                )) : (
                                                    <div className="rounded-md bg-slate-50 px-3 py-3 text-sm text-slate-500">
                                                        まだ model 別内訳はありません。
                                                    </div>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            )}
                        </div>

                        <div className="p-4 rounded-lg border border-gray-200 bg-white">
                            <h3 className="font-medium text-gray-900 mb-2">対象ディレクトリパス (Local Path)</h3>
                            <p className="text-sm text-gray-500 mb-4">
                                ClaudeCLIが自動開発を行う際の作業ディレクトリを指定してください。（ローカル環境の絶対パス）
                            </p>
                            <div className="flex items-center gap-3">
                                <input 
                                    type="text" 
                                    readOnly 
                                    value={currentProject?.local_path || '未設定'} 
                                    className="flex-1 border border-gray-300 rounded-md px-3 py-2 text-sm bg-gray-50 text-gray-700"
                                    placeholder="パスが未設定です"
                                />
                                <Button 
                                    onClick={handleSelectFolder} 
                                    disabled={isSelectingPath}
                                    variant="secondary"
                                >
                                    フォルダを選択
                                </Button>
                            </div>
                        </div>

                        <div className="p-4 rounded-lg border border-red-200 bg-red-50">
                            <h3 className="font-medium text-red-800 flex items-center gap-2 mb-2">
                                <AlertTriangle size={18} />
                                Danger Zone
                            </h3>
                            <p className="text-sm text-red-600 mb-4">
                                現在開いているプロジェクトを完全に削除します。この操作は取り消せません。バックログ、スプリント履歴、タスクのデータがすべて失われます。
                            </p>
                            <Button 
                                onClick={handleDeleteProject}
                                variant="secondary" 
                                className="bg-white text-red-600 border-red-200 hover:bg-red-50"
                                disabled={!currentProjectId || currentProjectId === 'default'}
                            >
                                <Trash2 size={16} className="mr-2" />
                                このプロジェクトを削除
                            </Button>
                        </div>
                    </div>
                )}

                {activeTab === 'team' && (
                    <TeamSettingsTab
                        config={teamConfig}
                        validationMessages={teamValidationMessages}
                        isLoading={isLoadingTeamConfig}
                        anthropicModelsList={anthropicModelsList}
                        isFetchingModels={isFetchingModels}
                        canFetchModels={Boolean(anthropicKey.trim())}
                        onChange={setTeamConfig}
                        onFetchModels={() => fetchModels('anthropic')}
                    />
                )}
            </div>

            <div className="mt-6 flex justify-end gap-3 pt-4 border-t border-gray-100">
                <button
                    onClick={onClose}
                    className="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none transition-colors"
                >
                    キャンセル
                </button>
                <button
                    onClick={handleSave}
                    disabled={isSaveDisabled}
                    className="px-4 py-2 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700 focus:outline-none transition-colors"
                >
                    {isSaving ? '保存中...' : '設定を保存'}
                </button>
            </div>
        </Modal>
    );
}
