import { useState, useEffect, useCallback } from 'react';
import { Modal } from './Modal';
import { Settings, Trash2, Shield, RefreshCw, AlertTriangle, Bot, CheckCircle2, ImageIcon, XCircle } from 'lucide-react';
import { Button } from './Button';
import { useWorkspace } from '../../context/WorkspaceContext';
import { invoke } from '@tauri-apps/api/core';
import { load } from '@tauri-apps/plugin-store';
import { confirm, open } from '@tauri-apps/plugin-dialog';
import toast from 'react-hot-toast';
import { TeamSettingsTab } from './TeamSettingsTab';
import { TeamConfiguration } from '../../types';
import {
    normalizeStoredStringValue,
    PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY,
    VICARA_SETTINGS_UPDATED_EVENT,
} from '../../hooks/usePoAssistantAvatarImage';
import { AvatarImageField } from './AvatarImageField';
import { CliDetectionResult, useCliDetection } from '../../hooks/useCliDetection';
import { ApiKeyStatus, OllamaStatus, SetupStatusTab } from './SetupStatusTab';
import { AnalyticsTab } from './AnalyticsTab';

interface GlobalSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
}

type SettingsTab = 'setup' | 'general' | 'analytics' | 'ai' | 'team';
type AiProvider = 'anthropic' | 'gemini' | 'openai' | 'ollama';
type PoAssistantTransport = 'api' | 'cli';
type SupportedCliType = 'claude' | 'gemini' | 'codex';
type InstalledCliMap = Record<SupportedCliType, boolean>;

const DEFAULT_OLLAMA_ENDPOINT = 'http://localhost:11434';
const DEFAULT_PO_ASSISTANT_CLI_MODELS: Record<SupportedCliType, string> = {
    claude: 'claude-sonnet-4-20250514',
    gemini: 'gemini-2.5-pro',
    codex: 'o3',
};

const DEFAULT_TEAM_CONFIGURATION: TeamConfiguration = {
    max_concurrent_agents: 1,
    roles: [],
};

const EMPTY_INSTALLED_CLI_MAP: InstalledCliMap = {
    claude: false,
    gemini: false,
    codex: false,
};

function buildInstalledCliMap(cliResults: CliDetectionResult[]): InstalledCliMap {
    const nextMap = { ...EMPTY_INSTALLED_CLI_MAP };

    cliResults.forEach((result) => {
        if (result.name === 'claude' || result.name === 'gemini' || result.name === 'codex') {
            nextMap[result.name] = result.installed;
        }
    });

    return nextMap;
}

function validateTeamConfiguration(
    config: TeamConfiguration,
): string[] {
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
        if (!role.cli_type.trim()) {
            messages.push(`Role ${index + 1} の CLI 種別を選択してください。`);
        }
        if (!role.model.trim()) {
            messages.push(`Role ${index + 1} のモデル名を入力してください。`);
        }
        if (!role.system_prompt.trim()) {
            messages.push(`Role ${index + 1} のシステムプロンプトを入力してください。`);
        }
    });

    return Array.from(new Set(messages));
}

function collectTeamConfigurationWarnings(
    config: TeamConfiguration,
    installedCliMap: InstalledCliMap,
    isCliDetectionLoading: boolean,
): string[] {
    if (isCliDetectionLoading) {
        return [];
    }

    const messages: string[] = [];

    config.roles.forEach((role, index) => {
        const cliType = (role.cli_type.trim() || 'claude') as SupportedCliType;
        if (cliType !== 'claude' && cliType !== 'gemini' && cliType !== 'codex') {
            return;
        }

        if (installedCliMap[cliType]) {
            return;
        }

        const label =
            cliType === 'claude' ? 'Claude Code CLI' : cliType === 'gemini' ? 'Gemini CLI' : 'Codex CLI';
        const roleLabel = role.name.trim() || `Role ${index + 1}`;
        messages.push(`${roleLabel} は ${label} を使用しますが、この環境ではまだ検出されていません。`);
    });

    return Array.from(new Set(messages));
}

function shouldOpenSetupTab(
    cliResults: { installed: boolean }[],
    apiKeyStatuses: ApiKeyStatus[],
    ollamaStatus: OllamaStatus | null,
) {
    const hasAnyCli = cliResults.some((result) => result.installed);
    const hasAnyApiKey = apiKeyStatuses.some((status) => status.configured);
    const hasAnyPoAssistantProvider = hasAnyApiKey || Boolean(ollamaStatus?.running);
    return !hasAnyCli || !hasAnyPoAssistantProvider;
}

function getAiProviderLabel(provider: AiProvider) {
    switch (provider) {
        case 'anthropic':
            return 'Anthropic (Claude)';
        case 'gemini':
            return 'Google Gemini';
        case 'openai':
            return 'OpenAI';
        case 'ollama':
            return 'Ollama';
    }
}

function getCliTypeLabel(cliType: SupportedCliType) {
    switch (cliType) {
        case 'claude':
            return 'Claude Code CLI';
        case 'gemini':
            return 'Gemini CLI';
        case 'codex':
            return 'Codex CLI';
    }
}

function getDefaultPoAssistantCliModel(cliType: SupportedCliType) {
    return DEFAULT_PO_ASSISTANT_CLI_MODELS[cliType];
}

function ConfigurationBadge({
    configured,
    configuredLabel = '設定済み',
    unconfiguredLabel = '未設定',
}: {
    configured: boolean;
    configuredLabel?: string;
    unconfiguredLabel?: string;
}) {
    return (
        <span
            className={`inline-flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium ${
                configured
                    ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
                    : 'border-amber-200 bg-amber-50 text-amber-700'
            }`}
        >
            {configured ? <CheckCircle2 size={14} /> : <XCircle size={14} />}
            {configured ? configuredLabel : unconfiguredLabel}
        </span>
    );
}

export function GlobalSettingsModal({ isOpen, onClose }: GlobalSettingsModalProps) {
    const { currentProjectId, deleteProject, projects, updateProjectPath, gitStatus, refreshGitStatus } = useWorkspace();
    const {
        results: cliResults,
        loading: isCliDetectionLoading,
        error: cliDetectionError,
        refresh: refreshCliDetection,
    } = useCliDetection();
    const currentProject = projects.find(p => p.id === currentProjectId);
    const installedCliMap = buildInstalledCliMap(cliResults);
    
    // Tabs state
    const [activeTab, setActiveTab] = useState<SettingsTab>('ai');
    
    // AI Settings State
    const [provider, setProvider] = useState<AiProvider>('anthropic');
    const [poAssistantTransport, setPoAssistantTransport] = useState<PoAssistantTransport>('api');
    const [poAssistantCliType, setPoAssistantCliType] = useState<SupportedCliType>('claude');
    const [poAssistantCliModel, setPoAssistantCliModel] = useState(DEFAULT_PO_ASSISTANT_CLI_MODELS.claude);
    const [anthropicKey, setAnthropicKey] = useState('');
    const [geminiKey, setGeminiKey] = useState('');
    const [openaiKey, setOpenaiKey] = useState('');
    const [ollamaEndpoint, setOllamaEndpoint] = useState(DEFAULT_OLLAMA_ENDPOINT);
    const [anthropicModel, setAnthropicModel] = useState('');
    const [geminiModel, setGeminiModel] = useState('');
    const [openaiModel, setOpenaiModel] = useState('');
    const [ollamaModel, setOllamaModel] = useState('');
    const [poAssistantAvatarImage, setPoAssistantAvatarImage] = useState<string | null>(null);
    
    // Custom model toggles
    const [isCustomAnthropic, setIsCustomAnthropic] = useState(false);
    const [isCustomGemini, setIsCustomGemini] = useState(false);
    const [isCustomOpenai, setIsCustomOpenai] = useState(false);
    const [isCustomOllama, setIsCustomOllama] = useState(false);
    
    // Models list from API
    const [anthropicModelsList, setAnthropicModelsList] = useState<string[]>([]);
    const [geminiModelsList, setGeminiModelsList] = useState<string[]>([]);
    const [openaiModelsList, setOpenaiModelsList] = useState<string[]>([]);
    const [ollamaModelsList, setOllamaModelsList] = useState<string[]>([]);
    const [isFetchingModels, setIsFetchingModels] = useState(false);
    const [fetchingModelsProvider, setFetchingModelsProvider] = useState<AiProvider | null>(null);
    
    // Path selection state
    const [isSelectingPath, setIsSelectingPath] = useState(false);

    // Team settings state
    const [teamConfig, setTeamConfig] = useState<TeamConfiguration>(DEFAULT_TEAM_CONFIGURATION);
    const [isLoadingTeamConfig, setIsLoadingTeamConfig] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [apiKeyStatuses, setApiKeyStatuses] = useState<ApiKeyStatus[]>([]);
    const [isLoadingApiKeyStatus, setIsLoadingApiKeyStatus] = useState(false);
    const [apiKeyStatusError, setApiKeyStatusError] = useState<string | null>(null);
    const [hasLoadedApiKeyStatus, setHasLoadedApiKeyStatus] = useState(false);
    const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus | null>(null);
    const [isLoadingOllamaStatus, setIsLoadingOllamaStatus] = useState(false);
    const [ollamaStatusError, setOllamaStatusError] = useState<string | null>(null);
    const [hasLoadedOllamaStatus, setHasLoadedOllamaStatus] = useState(false);
    const [isCheckingOllamaConnection, setIsCheckingOllamaConnection] = useState(false);
    const [hasInitializedTabForOpen, setHasInitializedTabForOpen] = useState(false);
    const [isRefreshingSetupStatus, setIsRefreshingSetupStatus] = useState(false);

    const loadApiKeyStatus = useCallback(async () => {
        setIsLoadingApiKeyStatus(true);
        try {
            const statuses = await invoke<ApiKeyStatus[]>('check_api_key_status');
            setApiKeyStatuses(statuses);
            setApiKeyStatusError(null);
            return statuses;
        } catch (error) {
            const message = String(error);
            console.error('Failed to load API key status', error);
            setApiKeyStatuses([]);
            setApiKeyStatusError(message);
            return [];
        } finally {
            setIsLoadingApiKeyStatus(false);
            setHasLoadedApiKeyStatus(true);
        }
    }, []);

    const loadOllamaStatus = useCallback(async (endpointOverride?: string) => {
        setIsLoadingOllamaStatus(true);
        try {
            const status = await invoke<OllamaStatus>('check_ollama_status', {
                endpointOverride: endpointOverride?.trim() ? endpointOverride.trim() : undefined,
            });
            setOllamaStatus(status);
            setOllamaStatusError(null);
            return status;
        } catch (error) {
            const message = String(error);
            console.error('Failed to load Ollama status', error);
            setOllamaStatus(null);
            setOllamaStatusError(message);
            return null;
        } finally {
            setIsLoadingOllamaStatus(false);
            setHasLoadedOllamaStatus(true);
        }
    }, []);

    const refreshSetupStatus = useCallback(async (showSpinner = true) => {
        if (showSpinner) {
            setIsRefreshingSetupStatus(true);
        } else {
            setHasLoadedApiKeyStatus(false);
            setHasLoadedOllamaStatus(false);
        }

        try {
            await Promise.all([
                refreshGitStatus(),
                refreshCliDetection(),
                loadApiKeyStatus(),
                loadOllamaStatus(),
            ]);
        } finally {
            if (showSpinner) {
                setIsRefreshingSetupStatus(false);
            }
        }
    }, [loadApiKeyStatus, loadOllamaStatus, refreshCliDetection, refreshGitStatus]);

    const handleRefreshSetupStatus = useCallback(async () => {
        try {
            await refreshSetupStatus();
            toast.success('セットアップ状況を更新しました');
        } catch (error) {
            console.error('Failed to refresh setup status', error);
            toast.error(`セットアップ状況の更新に失敗しました: ${error}`);
        }
    }, [refreshSetupStatus]);

    // Initial load
    useEffect(() => {
        if (isOpen) {
            void loadSettings();
            void refreshSetupStatus(false);
        } else {
            setHasInitializedTabForOpen(false);
            setHasLoadedApiKeyStatus(false);
            setHasLoadedOllamaStatus(false);
            setApiKeyStatusError(null);
            setOllamaStatusError(null);
        }
    }, [isOpen, refreshSetupStatus]);

    useEffect(() => {
        if (
            !isOpen ||
            hasInitializedTabForOpen ||
            isCliDetectionLoading ||
            !hasLoadedApiKeyStatus ||
            !hasLoadedOllamaStatus
        ) {
            return;
        }

        setActiveTab(shouldOpenSetupTab(cliResults, apiKeyStatuses, ollamaStatus) ? 'setup' : 'ai');
        setHasInitializedTabForOpen(true);
    }, [
        apiKeyStatuses,
        cliResults,
        hasInitializedTabForOpen,
        hasLoadedApiKeyStatus,
        hasLoadedOllamaStatus,
        isCliDetectionLoading,
        isOpen,
        ollamaStatus,
    ]);

    const loadSettings = async () => {
        setIsLoadingTeamConfig(true);
        try {
            const [store, loadedTeamConfig] = await Promise.all([
                load('settings.json'),
                invoke<TeamConfiguration>('get_team_configuration'),
            ]);

            const storedProvider = normalizeStoredStringValue(await store.get('default-ai-provider'));
            if (
                storedProvider === 'gemini' ||
                storedProvider === 'openai' ||
                storedProvider === 'ollama'
            ) {
                setProvider(storedProvider);
            } else {
                setProvider('anthropic');
            }

            const storedPoAssistantTransport = normalizeStoredStringValue(await store.get('po-assistant-transport'));
            setPoAssistantTransport(storedPoAssistantTransport === 'cli' ? 'cli' : 'api');

            const storedPoAssistantCliType = normalizeStoredStringValue(await store.get('po-assistant-cli-type'));
            const nextPoAssistantCliType: SupportedCliType =
                storedPoAssistantCliType === 'gemini' || storedPoAssistantCliType === 'codex'
                    ? storedPoAssistantCliType
                    : 'claude';
            setPoAssistantCliType(nextPoAssistantCliType);
            setPoAssistantCliModel(
                normalizeStoredStringValue(await store.get('po-assistant-cli-model'))
                    ?? getDefaultPoAssistantCliModel(nextPoAssistantCliType),
            );

            setAnthropicKey(normalizeStoredStringValue(await store.get('anthropic-api-key')) ?? '');
            setGeminiKey(normalizeStoredStringValue(await store.get('gemini-api-key')) ?? '');
            setOpenaiKey(normalizeStoredStringValue(await store.get('openai-api-key')) ?? '');
            setOllamaEndpoint(
                normalizeStoredStringValue(await store.get('ollama-endpoint')) ?? DEFAULT_OLLAMA_ENDPOINT,
            );

            setAnthropicModel(
                normalizeStoredStringValue(await store.get('anthropic-model')) ?? 'claude-3-5-sonnet-latest',
            );
            setGeminiModel(
                normalizeStoredStringValue(await store.get('gemini-model')) ?? 'gemini-2.5-flash',
            );
            setOpenaiModel(normalizeStoredStringValue(await store.get('openai-model')) ?? 'gpt-4o');
            setOllamaModel(normalizeStoredStringValue(await store.get('ollama-model')) ?? 'llama3.2');

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

    const fetchModels = async (targetProvider: AiProvider) => {
        setIsFetchingModels(true);
        setFetchingModelsProvider(targetProvider);
        try {
            const models = await invoke<string[]>('get_available_models', {
                provider: targetProvider,
                apiKeyOverride:
                    targetProvider === 'anthropic'
                        ? anthropicKey
                        : targetProvider === 'gemini'
                          ? geminiKey
                          : targetProvider === 'openai'
                            ? openaiKey
                            : undefined,
                endpointOverride: targetProvider === 'ollama' ? ollamaEndpoint : undefined,
            });

            if (targetProvider === 'anthropic') {
                setAnthropicModelsList(models);
                if (models.length > 0 && !isCustomAnthropic && !models.includes(anthropicModel)) {
                    setAnthropicModel(models[0]);
                }
            } else if (targetProvider === 'gemini') {
                setGeminiModelsList(models);
                if (models.length > 0 && !isCustomGemini && !models.includes(geminiModel)) {
                    setGeminiModel(models[0]);
                }
            } else if (targetProvider === 'openai') {
                setOpenaiModelsList(models);
                if (models.length > 0 && !isCustomOpenai && !models.includes(openaiModel)) {
                    setOpenaiModel(models[0]);
                }
            } else {
                setOllamaModelsList(models);
                if (models.length > 0 && !isCustomOllama && !models.includes(ollamaModel)) {
                    setOllamaModel(models[0]);
                }
                setOllamaStatus({
                    running: true,
                    models,
                    endpoint: ollamaEndpoint.trim() || DEFAULT_OLLAMA_ENDPOINT,
                    message: null,
                });
                setOllamaStatusError(null);
            }

            toast.success(`${getAiProviderLabel(targetProvider)} のモデル一覧を取得しました`);
        } catch (e) {
            console.error(`Failed to fetch ${targetProvider} models`, e);
            toast.error(`モデルの取得に失敗しました。設定値を確認してください。\nError: ${e}`);
        } finally {
            setIsFetchingModels(false);
            setFetchingModelsProvider(null);
        }
    };

    const teamValidationMessages = validateTeamConfiguration(teamConfig);
    const teamWarningMessages = collectTeamConfigurationWarnings(teamConfig, installedCliMap, isCliDetectionLoading);
    const isSaveDisabled = isSaving || isLoadingTeamConfig || teamValidationMessages.length > 0;
    const anthropicConfigured = Boolean(anthropicKey.trim());
    const geminiConfigured = Boolean(geminiKey.trim());
    const openaiConfigured = Boolean(openaiKey.trim());
    const ollamaConfigured = Boolean(ollamaStatus?.running);
    const configuredAiProviderCount =
        Number(anthropicConfigured) +
        Number(geminiConfigured) +
        Number(openaiConfigured) +
        Number(ollamaConfigured);
    const defaultAiProviderLabel = getAiProviderLabel(provider);
    const poAssistantCliInstalled = installedCliMap[poAssistantCliType];
    const poAssistantExecutionLabel =
        poAssistantTransport === 'api'
            ? defaultAiProviderLabel
            : getCliTypeLabel(poAssistantCliType);
    const poAssistantExecutionCaption =
        poAssistantTransport === 'api' ? 'API モード' : 'CLI モード';

    const handleSave = async () => {
        if (teamValidationMessages.length > 0) {
            toast.error(teamValidationMessages[0]);
            return;
        }

        setIsSaving(true);
        try {
            const store = await load('settings.json');
            await store.set('default-ai-provider', { value: provider });
            await store.set('po-assistant-transport', { value: poAssistantTransport });
            await store.set('po-assistant-cli-type', { value: poAssistantCliType });
            await store.set('po-assistant-cli-model', {
                value: poAssistantCliModel.trim() || getDefaultPoAssistantCliModel(poAssistantCliType),
            });
            await store.set('anthropic-api-key', { value: anthropicKey });
            await store.set('gemini-api-key', { value: geminiKey });
            await store.set('openai-api-key', { value: openaiKey });
            await store.set('ollama-endpoint', { value: ollamaEndpoint.trim() || DEFAULT_OLLAMA_ENDPOINT });
            await store.set('anthropic-model', { value: anthropicModel });
            await store.set('gemini-model', { value: geminiModel });
            await store.set('openai-model', { value: openaiModel });
            await store.set('ollama-model', { value: ollamaModel });
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

    const handleCheckOllamaConnection = async () => {
        setIsCheckingOllamaConnection(true);
        try {
            const status = await loadOllamaStatus(ollamaEndpoint);
            if (!status) {
                setOllamaModelsList([]);
                toast.error('Ollama の接続確認に失敗しました');
                return;
            }

            setOllamaModelsList(status.models);
            if (status.running) {
                if (status.models.length > 0 && !isCustomOllama && !status.models.includes(ollamaModel)) {
                    setOllamaModel(status.models[0]);
                }
                toast.success(
                    status.models.length > 0
                        ? `Ollama に接続しました（${status.models.length} モデル検出）`
                        : 'Ollama に接続しました',
                );
            } else {
                toast.error(status.message ?? 'Ollama に接続できませんでした');
            }
        } finally {
            setIsCheckingOllamaConnection(false);
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
                    onClick={() => setActiveTab('setup')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'setup'
                            ? 'border-b-2 border-blue-500 text-blue-600'
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    セットアップ状況
                </button>
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
                    onClick={() => setActiveTab('team')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'team'
                            ? 'border-b-2 border-blue-500 text-blue-600' 
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    チーム設定
                </button>
                <button
                    onClick={() => setActiveTab('analytics')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'analytics'
                            ? 'border-b-2 border-blue-500 text-blue-600'
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    アナリティクス
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
            </div>

            <div className="min-h-[350px] max-h-[60vh] overflow-y-auto px-1 custom-scrollbar">
                {activeTab === 'setup' && (
                    <SetupStatusTab
                        gitStatus={gitStatus}
                        cliResults={cliResults}
                        cliLoading={isCliDetectionLoading}
                        cliError={cliDetectionError}
                        apiKeyStatuses={apiKeyStatuses}
                        apiLoading={isLoadingApiKeyStatus}
                        apiError={apiKeyStatusError}
                        ollamaStatus={ollamaStatus}
                        ollamaLoading={isLoadingOllamaStatus}
                        ollamaError={ollamaStatusError}
                        isRefreshing={isRefreshingSetupStatus}
                        onRefresh={handleRefreshSetupStatus}
                    />
                )}

                {activeTab === 'ai' && (
                    <div className="space-y-5">
                        <div className="rounded-2xl border border-sky-200 bg-gradient-to-br from-sky-50 via-white to-cyan-50 p-5 shadow-sm">
                            <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
                                <div className="min-w-0 flex-1">
                                    <div className="inline-flex items-center gap-2 rounded-full border border-sky-200 bg-white/80 px-3 py-1 text-xs font-semibold text-sky-700 shadow-sm">
                                        <Bot size={14} />
                                        PO Assistant Studio
                                    </div>

                                    <div className="mt-4 flex items-start gap-3">
                                        <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-2xl bg-sky-100 text-sky-700">
                                            <Shield size={22} />
                                        </div>
                                        <div className="min-w-0">
                                            <h3 className="text-lg font-semibold text-slate-900">
                                                POアシスタントの既定動作を整える
                                            </h3>
                                            <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600">
                                                実行方式、既定プロバイダー、CLI、APIキー、モデル、画像をまとめて管理します。ここで保存した内容は、
                                                POアシスタントのチャット体験とサイドバー表示に反映されます。
                                            </p>
                                        </div>
                                    </div>

                                    <div className="mt-4 flex flex-wrap gap-2">
                                        <span className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-600">
                                            Conversational Control
                                        </span>
                                        <span className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-600">
                                            Transport Switching
                                        </span>
                                        <span className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-600">
                                            Avatar Customization
                                        </span>
                                    </div>
                                </div>

                                <div className="grid min-w-[220px] gap-3 sm:grid-cols-2 lg:grid-cols-1 xl:grid-cols-2">
                                    <div className="rounded-2xl border border-white/70 bg-white/80 p-4 shadow-sm">
                                        <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                            Default
                                        </div>
                                        <div className="mt-2 text-lg font-semibold text-slate-900">{poAssistantExecutionLabel}</div>
                                        <div className="mt-1 text-sm text-slate-500">{poAssistantExecutionCaption}</div>
                                    </div>
                                    <div className="rounded-2xl border border-white/70 bg-white/80 p-4 shadow-sm">
                                        <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-500">
                                            Ready
                                        </div>
                                        <div className="mt-2 text-2xl font-semibold text-slate-900">{configuredAiProviderCount}/4</div>
                                        <div className="mt-1 text-sm text-slate-500">利用可能なプロバイダー数</div>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                            <div className="flex items-center gap-3">
                                <div className="flex h-10 w-10 items-center justify-center rounded-2xl bg-sky-100 text-sky-700">
                                    <ImageIcon size={18} />
                                </div>
                                <div>
                                    <div className="text-xs font-semibold uppercase tracking-[0.18em] text-sky-600">
                                        01 Visual Identity
                                    </div>
                                    <h3 className="text-sm font-semibold text-slate-900">POアシスタント画像</h3>
                                </div>
                            </div>

                            <div className="mt-4">
                                <AvatarImageField
                                    label="POアシスタント画像"
                                    description="ヘッダー、チャット、サイドバー右下の立ち絵表示に使用する画像です。未設定時は標準の POアシスタント画像を使用します。"
                                    value={poAssistantAvatarImage}
                                    fallbackKind="po-assistant"
                                    previewMode="figure"
                                    onChange={setPoAssistantAvatarImage}
                                />
                            </div>
                        </div>

                        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                            <div className="max-w-3xl">
                                <div className="text-xs font-semibold uppercase tracking-[0.18em] text-sky-600">
                                    02 Execution Mode
                                </div>
                                <h3 className="mt-2 text-sm font-semibold text-slate-900">実行方式</h3>
                                <p className="mt-1 text-sm leading-6 text-slate-600">
                                    POアシスタントを API で動かすか、CLI サブスクリプション / ローカル実行で動かすかを選択します。
                                </p>
                            </div>

                            <div className="mt-5 grid gap-3 md:grid-cols-2">
                                <label
                                    className={`cursor-pointer rounded-2xl border p-4 shadow-sm transition-all ${
                                        poAssistantTransport === 'api'
                                            ? 'border-sky-300 bg-sky-50/80 ring-1 ring-sky-200'
                                            : 'border-slate-200 bg-white hover:bg-slate-50'
                                    }`}
                                >
                                    <input
                                        type="radio"
                                        name="po-assistant-transport"
                                        value="api"
                                        checked={poAssistantTransport === 'api'}
                                        onChange={() => setPoAssistantTransport('api')}
                                        className="hidden"
                                    />
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="text-sm font-semibold text-slate-900">API</div>
                                            <p className="mt-2 text-sm leading-6 text-slate-600">
                                                Anthropic / Gemini / OpenAI / Ollama の既存プロバイダー設定を使って POアシスタントを実行します。
                                            </p>
                                        </div>
                                        <ConfigurationBadge configured={configuredAiProviderCount > 0} />
                                    </div>
                                </label>

                                <label
                                    className={`cursor-pointer rounded-2xl border p-4 shadow-sm transition-all ${
                                        poAssistantTransport === 'cli'
                                            ? 'border-cyan-300 bg-cyan-50/80 ring-1 ring-cyan-200'
                                            : 'border-slate-200 bg-white hover:bg-slate-50'
                                    }`}
                                >
                                    <input
                                        type="radio"
                                        name="po-assistant-transport"
                                        value="cli"
                                        checked={poAssistantTransport === 'cli'}
                                        onChange={() => setPoAssistantTransport('cli')}
                                        className="hidden"
                                    />
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="text-sm font-semibold text-slate-900">CLI</div>
                                            <p className="mt-2 text-sm leading-6 text-slate-600">
                                                Claude Code / Gemini CLI / Codex CLI などのサブスク・ローカル環境を使って追加 API コストを抑えます。
                                            </p>
                                        </div>
                                        <ConfigurationBadge
                                            configured={poAssistantCliInstalled}
                                            configuredLabel="検出済み"
                                            unconfiguredLabel="未検出"
                                        />
                                    </div>
                                </label>
                            </div>

                            {poAssistantTransport === 'cli' && (
                                <div className="mt-5 grid gap-4 lg:grid-cols-[minmax(0,260px)_1fr]">
                                    <div>
                                        <label className="mb-1 block text-sm font-medium text-slate-700">CLI種別</label>
                                        <select
                                            value={poAssistantCliType}
                                            onChange={(e) => {
                                                const nextCliType = e.target.value as SupportedCliType;
                                                setPoAssistantCliType(nextCliType);
                                                setPoAssistantCliModel(getDefaultPoAssistantCliModel(nextCliType));
                                            }}
                                            className="w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-cyan-500"
                                        >
                                            <option value="claude">Claude Code CLI</option>
                                            <option value="gemini">Gemini CLI</option>
                                            <option value="codex">Codex CLI</option>
                                        </select>
                                    </div>

                                    <div>
                                        <div className="mb-1 flex items-center justify-between gap-3">
                                            <label className="block text-sm font-medium text-slate-700">モデル</label>
                                            <ConfigurationBadge
                                                configured={poAssistantCliInstalled}
                                                configuredLabel="CLI 検出済み"
                                                unconfiguredLabel="CLI 未検出"
                                            />
                                        </div>
                                        <input
                                            type="text"
                                            value={poAssistantCliModel}
                                            onChange={(e) => setPoAssistantCliModel(e.target.value)}
                                            placeholder={getDefaultPoAssistantCliModel(poAssistantCliType)}
                                            className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-cyan-500"
                                        />
                                        <p className="mt-2 text-xs leading-5 text-slate-500">
                                            {poAssistantCliInstalled
                                                ? `${getCliTypeLabel(poAssistantCliType)} を検出済みです。必要に応じてモデル名だけを上書きできます。`
                                                : `${getCliTypeLabel(poAssistantCliType)} はまだ検出されていません。保存は可能ですが、実行前にセットアップタブで導入を確認してください。`}
                                        </p>
                                    </div>
                                </div>
                            )}
                        </div>

                        {poAssistantTransport === 'api' && (
                            <>
                        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                            <div className="max-w-3xl">
                                <div className="text-xs font-semibold uppercase tracking-[0.18em] text-sky-600">
                                    03 Default Provider
                                </div>
                                <h3 className="mt-2 text-sm font-semibold text-slate-900">既定AIプロバイダー</h3>
                                <p className="mt-1 text-sm leading-6 text-slate-600">
                                    POアシスタントが最初に使う既定プロバイダーを選択します。各カードには、APIキー設定またはローカル稼働状態を表示します。
                                </p>
                            </div>

                            <div className="mt-5 grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                                <label
                                    className={`cursor-pointer rounded-2xl border p-4 shadow-sm transition-all ${
                                        provider === 'anthropic'
                                            ? 'border-sky-300 bg-sky-50/80 ring-1 ring-sky-200'
                                            : 'border-slate-200 bg-white hover:bg-slate-50'
                                    }`}
                                >
                                    <input
                                        type="radio"
                                        name="provider"
                                        value="anthropic"
                                        checked={provider === 'anthropic'}
                                        onChange={() => setProvider('anthropic')}
                                        className="hidden"
                                    />
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="text-sm font-semibold text-slate-900">Anthropic (Claude)</div>
                                            <p className="mt-2 text-sm leading-6 text-slate-600">
                                                Claude 系モデルを既定にし、設計整理や長文レビュー寄りの対話を起点にします。
                                            </p>
                                        </div>
                                        <ConfigurationBadge configured={anthropicConfigured} />
                                    </div>
                                    <div className="mt-4 text-xs font-medium text-sky-700">
                                        {provider === 'anthropic' ? '現在の既定プロバイダーです。' : 'クリックすると既定プロバイダーに切り替わります。'}
                                    </div>
                                </label>

                                <label
                                    className={`cursor-pointer rounded-2xl border p-4 shadow-sm transition-all ${
                                        provider === 'gemini'
                                            ? 'border-violet-300 bg-violet-50/80 ring-1 ring-violet-200'
                                            : 'border-slate-200 bg-white hover:bg-slate-50'
                                    }`}
                                >
                                    <input
                                        type="radio"
                                        name="provider"
                                        value="gemini"
                                        checked={provider === 'gemini'}
                                        onChange={() => setProvider('gemini')}
                                        className="hidden"
                                    />
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="text-sm font-semibold text-slate-900">Google Gemini</div>
                                            <p className="mt-2 text-sm leading-6 text-slate-600">
                                                Gemini 系モデルを既定にし、素早い応答や広い文脈を活かした対話を起点にします。
                                            </p>
                                        </div>
                                        <ConfigurationBadge configured={geminiConfigured} />
                                    </div>
                                    <div className="mt-4 text-xs font-medium text-violet-700">
                                        {provider === 'gemini' ? '現在の既定プロバイダーです。' : 'クリックすると既定プロバイダーに切り替わります。'}
                                    </div>
                                </label>

                                <label
                                    className={`cursor-pointer rounded-2xl border p-4 shadow-sm transition-all ${
                                        provider === 'openai'
                                            ? 'border-emerald-300 bg-emerald-50/80 ring-1 ring-emerald-200'
                                            : 'border-slate-200 bg-white hover:bg-slate-50'
                                    }`}
                                >
                                    <input
                                        type="radio"
                                        name="provider"
                                        value="openai"
                                        checked={provider === 'openai'}
                                        onChange={() => setProvider('openai')}
                                        className="hidden"
                                    />
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="text-sm font-semibold text-slate-900">OpenAI</div>
                                            <p className="mt-2 text-sm leading-6 text-slate-600">
                                                GPT 系モデルを既定にし、ツール呼び出しや汎用対話の選択肢を広げます。
                                            </p>
                                        </div>
                                        <ConfigurationBadge configured={openaiConfigured} />
                                    </div>
                                    <div className="mt-4 text-xs font-medium text-emerald-700">
                                        {provider === 'openai' ? '現在の既定プロバイダーです。' : 'クリックすると既定プロバイダーに切り替わります。'}
                                    </div>
                                </label>

                                <label
                                    className={`cursor-pointer rounded-2xl border p-4 shadow-sm transition-all ${
                                        provider === 'ollama'
                                            ? 'border-amber-300 bg-amber-50/80 ring-1 ring-amber-200'
                                            : 'border-slate-200 bg-white hover:bg-slate-50'
                                    }`}
                                >
                                    <input
                                        type="radio"
                                        name="provider"
                                        value="ollama"
                                        checked={provider === 'ollama'}
                                        onChange={() => setProvider('ollama')}
                                        className="hidden"
                                    />
                                    <div className="flex items-start justify-between gap-3">
                                        <div className="min-w-0">
                                            <div className="text-sm font-semibold text-slate-900">Ollama</div>
                                            <p className="mt-2 text-sm leading-6 text-slate-600">
                                                ローカル LLM を既定にし、API コストをかけずに POアシスタントを運用できます。
                                            </p>
                                        </div>
                                        <ConfigurationBadge
                                            configured={ollamaConfigured}
                                            configuredLabel="稼働中"
                                            unconfiguredLabel="未稼働"
                                        />
                                    </div>
                                    <div className="mt-4 text-xs font-medium text-amber-700">
                                        {provider === 'ollama' ? '現在の既定プロバイダーです。' : 'クリックすると既定プロバイダーに切り替わります。'}
                                    </div>
                                </label>
                            </div>
                        </div>

                        <div className="rounded-2xl border border-slate-200 bg-white p-5 shadow-sm">
                            <div className="max-w-3xl">
                                <div className="text-xs font-semibold uppercase tracking-[0.18em] text-sky-600">
                                    04 Provider Settings
                                </div>
                                <h3 className="mt-2 text-sm font-semibold text-slate-900">プロバイダー設定</h3>
                                <p className="mt-1 text-sm leading-6 text-slate-600">
                                    すべてのプロバイダーを事前に設定できます。既定プロバイダーは上で選びつつ、必要に応じて切り替えられる状態を維持できます。
                                </p>
                            </div>

                            <div className="mt-5 grid gap-4 xl:grid-cols-2">
                                <div
                                    className={`rounded-2xl border p-5 ${
                                        provider === 'anthropic'
                                            ? 'border-sky-200 bg-sky-50/60 shadow-sm'
                                            : 'border-slate-200 bg-slate-50/40'
                                    }`}
                                >
                                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                                        <div className="min-w-0">
                                            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-sky-600">
                                                Anthropic
                                            </div>
                                            <h4 className="mt-1 flex items-center gap-2 text-sm font-semibold text-slate-900">
                                                <Shield size={16} className="text-slate-500" />
                                                Claude 系設定
                                            </h4>
                                            <p className="mt-1 text-sm leading-6 text-slate-600">
                                                APIキーとモデルを設定します。モデル一覧は API から補助的に取得できます。
                                            </p>
                                        </div>
                                        <ConfigurationBadge configured={anthropicConfigured} />
                                    </div>

                                    <div className="mt-5 space-y-4">
                                        <div>
                                            <label className="mb-1 block text-sm font-medium text-slate-700">APIキー</label>
                                            <input
                                                type="password"
                                                placeholder="sk-ant-api03-..."
                                                value={anthropicKey}
                                                onChange={(e) => setAnthropicKey(e.target.value)}
                                                className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                                            />
                                        </div>

                                        <div>
                                            <div className="mb-1 flex items-center justify-between gap-3">
                                                <label className="block text-sm font-medium text-slate-700">モデル</label>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    variant="ghost"
                                                    onClick={() => fetchModels('anthropic')}
                                                    disabled={isFetchingModels || !anthropicKey}
                                                    className="border border-sky-200 bg-white text-sky-700 hover:bg-sky-50"
                                                >
                                                    <RefreshCw
                                                        size={12}
                                                        className={`mr-2 ${
                                                            isFetchingModels && fetchingModelsProvider === 'anthropic' ? 'animate-spin' : ''
                                                        }`}
                                                    />
                                                    モデル一覧を取得
                                                </Button>
                                            </div>

                                            {!isCustomAnthropic && anthropicModelsList.length > 0 ? (
                                                <select
                                                    value={anthropicModel}
                                                    onChange={(e) => setAnthropicModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                                                >
                                                    {anthropicModelsList.map((model) => (
                                                        <option key={model} value={model}>{model}</option>
                                                    ))}
                                                </select>
                                            ) : (
                                                <input
                                                    type="text"
                                                    placeholder="claude-3-5-sonnet-latest"
                                                    value={anthropicModel}
                                                    onChange={(e) => setAnthropicModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-sky-500"
                                                />
                                            )}

                                            <label className="flex items-center gap-2 text-xs text-slate-500">
                                                <input
                                                    type="checkbox"
                                                    checked={isCustomAnthropic}
                                                    onChange={(e) => setIsCustomAnthropic(e.target.checked)}
                                                    className="rounded border-slate-300 text-sky-600 focus:ring-sky-500"
                                                />
                                                カスタムモデル名を手動で入力する
                                            </label>
                                        </div>
                                    </div>
                                </div>

                                <div
                                    className={`rounded-2xl border p-5 ${
                                        provider === 'gemini'
                                            ? 'border-violet-200 bg-violet-50/60 shadow-sm'
                                            : 'border-slate-200 bg-slate-50/40'
                                    }`}
                                >
                                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                                        <div className="min-w-0">
                                            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-violet-600">
                                                Gemini
                                            </div>
                                            <h4 className="mt-1 flex items-center gap-2 text-sm font-semibold text-slate-900">
                                                <Shield size={16} className="text-slate-500" />
                                                Gemini 系設定
                                            </h4>
                                            <p className="mt-1 text-sm leading-6 text-slate-600">
                                                APIキーとモデルを設定します。未選択でも保持されるので、あとで既定に切り替えられます。
                                            </p>
                                        </div>
                                        <ConfigurationBadge configured={geminiConfigured} />
                                    </div>

                                    <div className="mt-5 space-y-4">
                                        <div>
                                            <label className="mb-1 block text-sm font-medium text-slate-700">APIキー</label>
                                            <input
                                                type="password"
                                                placeholder="AIzaSy..."
                                                value={geminiKey}
                                                onChange={(e) => setGeminiKey(e.target.value)}
                                                className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-violet-500"
                                            />
                                        </div>

                                        <div>
                                            <div className="mb-1 flex items-center justify-between gap-3">
                                                <label className="block text-sm font-medium text-slate-700">モデル</label>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    variant="ghost"
                                                    onClick={() => fetchModels('gemini')}
                                                    disabled={isFetchingModels || !geminiKey}
                                                    className="border border-violet-200 bg-white text-violet-700 hover:bg-violet-50"
                                                >
                                                    <RefreshCw
                                                        size={12}
                                                        className={`mr-2 ${
                                                            isFetchingModels && fetchingModelsProvider === 'gemini' ? 'animate-spin' : ''
                                                        }`}
                                                    />
                                                    モデル一覧を取得
                                                </Button>
                                            </div>

                                            {!isCustomGemini && geminiModelsList.length > 0 ? (
                                                <select
                                                    value={geminiModel}
                                                    onChange={(e) => setGeminiModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-violet-500"
                                                >
                                                    {geminiModelsList.map((model) => (
                                                        <option key={model} value={model}>{model}</option>
                                                    ))}
                                                </select>
                                            ) : (
                                                <input
                                                    type="text"
                                                    placeholder="gemini-2.5-flash"
                                                    value={geminiModel}
                                                    onChange={(e) => setGeminiModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-violet-500"
                                                />
                                            )}

                                            <label className="flex items-center gap-2 text-xs text-slate-500">
                                                <input
                                                    type="checkbox"
                                                    checked={isCustomGemini}
                                                    onChange={(e) => setIsCustomGemini(e.target.checked)}
                                                    className="rounded border-slate-300 text-violet-600 focus:ring-violet-500"
                                                />
                                                カスタムモデル名を手動で入力する
                                            </label>
                                        </div>
                                    </div>
                                </div>

                                <div
                                    className={`rounded-2xl border p-5 ${
                                        provider === 'openai'
                                            ? 'border-emerald-200 bg-emerald-50/60 shadow-sm'
                                            : 'border-slate-200 bg-slate-50/40'
                                    }`}
                                >
                                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                                        <div className="min-w-0">
                                            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-emerald-600">
                                                OpenAI
                                            </div>
                                            <h4 className="mt-1 flex items-center gap-2 text-sm font-semibold text-slate-900">
                                                <Shield size={16} className="text-slate-500" />
                                                GPT 系設定
                                            </h4>
                                            <p className="mt-1 text-sm leading-6 text-slate-600">
                                                APIキーとモデルを設定します。保存前の APIキーでもモデル一覧を取得できます。
                                            </p>
                                        </div>
                                        <ConfigurationBadge configured={openaiConfigured} />
                                    </div>

                                    <div className="mt-5 space-y-4">
                                        <div>
                                            <label className="mb-1 block text-sm font-medium text-slate-700">APIキー</label>
                                            <input
                                                type="password"
                                                placeholder="sk-proj-..."
                                                value={openaiKey}
                                                onChange={(e) => setOpenaiKey(e.target.value)}
                                                className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-emerald-500"
                                            />
                                        </div>

                                        <div>
                                            <div className="mb-1 flex items-center justify-between gap-3">
                                                <label className="block text-sm font-medium text-slate-700">モデル</label>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    variant="ghost"
                                                    onClick={() => fetchModels('openai')}
                                                    disabled={isFetchingModels || !openaiKey.trim()}
                                                    className="border border-emerald-200 bg-white text-emerald-700 hover:bg-emerald-50"
                                                >
                                                    <RefreshCw
                                                        size={12}
                                                        className={`mr-2 ${
                                                            isFetchingModels && fetchingModelsProvider === 'openai' ? 'animate-spin' : ''
                                                        }`}
                                                    />
                                                    モデル一覧を取得
                                                </Button>
                                            </div>

                                            {!isCustomOpenai && openaiModelsList.length > 0 ? (
                                                <select
                                                    value={openaiModel}
                                                    onChange={(e) => setOpenaiModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-emerald-500"
                                                >
                                                    {openaiModelsList.map((model) => (
                                                        <option key={model} value={model}>{model}</option>
                                                    ))}
                                                </select>
                                            ) : (
                                                <input
                                                    type="text"
                                                    placeholder="gpt-4o"
                                                    value={openaiModel}
                                                    onChange={(e) => setOpenaiModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-emerald-500"
                                                />
                                            )}

                                            <label className="flex items-center gap-2 text-xs text-slate-500">
                                                <input
                                                    type="checkbox"
                                                    checked={isCustomOpenai}
                                                    onChange={(e) => setIsCustomOpenai(e.target.checked)}
                                                    className="rounded border-slate-300 text-emerald-600 focus:ring-emerald-500"
                                                />
                                                カスタムモデル名を手動で入力する
                                            </label>
                                        </div>
                                    </div>
                                </div>

                                <div
                                    className={`rounded-2xl border p-5 ${
                                        provider === 'ollama'
                                            ? 'border-amber-200 bg-amber-50/60 shadow-sm'
                                            : 'border-slate-200 bg-slate-50/40'
                                    }`}
                                >
                                    <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                                        <div className="min-w-0">
                                            <div className="text-xs font-semibold uppercase tracking-[0.16em] text-amber-600">
                                                Ollama
                                            </div>
                                            <h4 className="mt-1 flex items-center gap-2 text-sm font-semibold text-slate-900">
                                                <Shield size={16} className="text-slate-500" />
                                                ローカル LLM 設定
                                            </h4>
                                            <p className="mt-1 text-sm leading-6 text-slate-600">
                                                エンドポイントとモデルを設定します。接続テストで稼働確認とモデル検出を行えます。
                                            </p>
                                        </div>
                                        <ConfigurationBadge
                                            configured={ollamaConfigured}
                                            configuredLabel="稼働中"
                                            unconfiguredLabel="未稼働"
                                        />
                                    </div>

                                    <div className="mt-5 space-y-4">
                                        <div>
                                            <div className="mb-1 flex items-center justify-between gap-3">
                                                <label className="block text-sm font-medium text-slate-700">エンドポイント</label>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    variant="ghost"
                                                    onClick={handleCheckOllamaConnection}
                                                    disabled={isCheckingOllamaConnection}
                                                    className="border border-amber-200 bg-white text-amber-700 hover:bg-amber-50"
                                                >
                                                    <RefreshCw
                                                        size={12}
                                                        className={`mr-2 ${isCheckingOllamaConnection ? 'animate-spin' : ''}`}
                                                    />
                                                    接続テスト
                                                </Button>
                                            </div>
                                            <input
                                                type="text"
                                                placeholder={DEFAULT_OLLAMA_ENDPOINT}
                                                value={ollamaEndpoint}
                                                onChange={(e) => setOllamaEndpoint(e.target.value)}
                                                className="w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-amber-500"
                                            />
                                            {ollamaStatus && (
                                                <div
                                                    className={`mt-2 rounded-xl border px-3 py-2 text-xs ${
                                                        ollamaStatus.running
                                                            ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
                                                            : 'border-amber-200 bg-amber-50 text-amber-700'
                                                    }`}
                                                >
                                                    {ollamaStatus.running
                                                        ? `${ollamaStatus.endpoint} に接続済みです。${ollamaStatus.models.length} モデルを検出しました。`
                                                        : ollamaStatus.message ?? 'Ollama の接続確認に失敗しました。'}
                                                </div>
                                            )}
                                        </div>

                                        <div>
                                            <div className="mb-1 flex items-center justify-between gap-3">
                                                <label className="block text-sm font-medium text-slate-700">モデル</label>
                                                <Button
                                                    type="button"
                                                    size="sm"
                                                    variant="ghost"
                                                    onClick={() => fetchModels('ollama')}
                                                    disabled={isFetchingModels || !ollamaEndpoint.trim()}
                                                    className="border border-amber-200 bg-white text-amber-700 hover:bg-amber-50"
                                                >
                                                    <RefreshCw
                                                        size={12}
                                                        className={`mr-2 ${
                                                            isFetchingModels && fetchingModelsProvider === 'ollama' ? 'animate-spin' : ''
                                                        }`}
                                                    />
                                                    モデル一覧を取得
                                                </Button>
                                            </div>

                                            {!isCustomOllama && ollamaModelsList.length > 0 ? (
                                                <select
                                                    value={ollamaModel}
                                                    onChange={(e) => setOllamaModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 bg-white px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-amber-500"
                                                >
                                                    {ollamaModelsList.map((model) => (
                                                        <option key={model} value={model}>{model}</option>
                                                    ))}
                                                </select>
                                            ) : (
                                                <input
                                                    type="text"
                                                    placeholder="llama3.2"
                                                    value={ollamaModel}
                                                    onChange={(e) => setOllamaModel(e.target.value)}
                                                    className="mb-2 w-full rounded-xl border border-slate-300 px-3 py-2 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-amber-500"
                                                />
                                            )}

                                            <label className="flex items-center gap-2 text-xs text-slate-500">
                                                <input
                                                    type="checkbox"
                                                    checked={isCustomOllama}
                                                    onChange={(e) => setIsCustomOllama(e.target.checked)}
                                                    className="rounded border-slate-300 text-amber-600 focus:ring-amber-500"
                                                />
                                                カスタムモデル名を手動で入力する
                                            </label>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                            </>
                        )}
                    </div>
                )}

                {activeTab === 'analytics' && (
                    <AnalyticsTab projectId={currentProjectId} />
                )}

                {activeTab === 'general' && (
                    <div className="space-y-6">
                        <div className="p-4 rounded-lg border border-gray-200 bg-white">
                            <h3 className="font-medium text-gray-900 mb-2">対象ディレクトリパス (Local Path)</h3>
                            <p className="text-sm text-gray-500 mb-4">
                                Dev エージェントが自動開発を行う際の作業ディレクトリを指定してください。（ローカル環境の絶対パス）
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
                        geminiModelsList={geminiModelsList}
                        cliResults={cliResults}
                        installedCliMap={installedCliMap}
                        isCliDetectionLoading={isCliDetectionLoading}
                        isFetchingAnthropicModels={isFetchingModels && fetchingModelsProvider === 'anthropic'}
                        isFetchingGeminiModels={isFetchingModels && fetchingModelsProvider === 'gemini'}
                        canFetchAnthropicModels={Boolean(anthropicKey.trim())}
                        canFetchGeminiModels={Boolean(geminiKey.trim())}
                        onChange={setTeamConfig}
                        onFetchAnthropicModels={() => fetchModels('anthropic')}
                        onFetchGeminiModels={() => fetchModels('gemini')}
                    />
                )}
            </div>

            <div className="mt-6 border-t border-gray-100 pt-4">
                {activeTab === 'team' && teamWarningMessages.length > 0 && (
                    <div className="mb-4 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
                        <p className="text-sm font-medium text-amber-800">
                            未導入の CLI を使うロールがあります。設定の保存は可能ですが、実行前にセットアップを完了してください。
                        </p>
                        <ul className="mt-2 list-disc pl-5 text-sm text-amber-700">
                            {teamWarningMessages.map((message) => (
                                <li key={message}>{message}</li>
                            ))}
                        </ul>
                    </div>
                )}

                <div className="flex justify-end gap-3">
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
            </div>
        </Modal>
    );
}
