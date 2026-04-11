import {
    createContext,
    useCallback,
    useContext,
    useEffect,
    useMemo,
    useState,
    type ReactNode,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { confirm, open } from '@tauri-apps/plugin-dialog';
import { load } from '@tauri-apps/plugin-store';
import toast from 'react-hot-toast';
import { useWorkspace } from '../../../context/WorkspaceContext';
import { type CliDetectionResult, useCliDetection } from '../../../hooks/useCliDetection';
import {
    normalizeStoredStringValue,
    PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY,
    VICARA_SETTINGS_UPDATED_EVENT,
} from '../../../hooks/usePoAssistantAvatarImage';
import type { Project, TeamConfiguration } from '../../../types';
import type { ApiKeyStatus, OllamaStatus } from '../SetupStatusTab';

export type SettingsSectionId =
    | 'project'
    | 'setup'
    | 'ai-selection'
    | 'ai-provider'
    | 'po-assistant'
    | 'team'
    | 'analytics';

export type AiProvider = 'anthropic' | 'gemini' | 'openai' | 'ollama';
export type PoAssistantTransport = 'api' | 'cli';
export type SupportedCliType = 'claude' | 'gemini' | 'codex';
type ApiKeyProvider = Exclude<AiProvider, 'ollama'>;
export type InstalledCliMap = Record<SupportedCliType, boolean>;

type SettingsStore = Awaited<ReturnType<typeof load>>;

interface SettingsDraft {
    provider: AiProvider;
    poAssistantTransport: PoAssistantTransport;
    poAssistantCliType: SupportedCliType;
    poAssistantCliModel: string;
    apiKeys: Record<ApiKeyProvider, string>;
    providerModels: Record<AiProvider, string>;
    ollamaEndpoint: string;
    poAssistantAvatarImage: string | null;
    teamConfig: TeamConfiguration;
}

interface PersistedAiPreferences {
    provider: AiProvider;
    transport: PoAssistantTransport;
    providerModels: Record<AiProvider, string>;
}

interface SettingsProviderProps {
    children: ReactNode;
    onClose: () => void;
    closeOnSave?: boolean;
}

interface SettingsContextValue {
    currentProjectId: string;
    currentProject: Project | undefined;
    gitStatus: {
        checked: boolean;
        installed: boolean;
        version: string | null;
        message: string | null;
    };
    draft: SettingsDraft;
    cliResults: CliDetectionResult[];
    isCliDetectionLoading: boolean;
    cliDetectionError: string | null;
    installedCliMap: InstalledCliMap;
    modelOptions: Record<AiProvider, string[]>;
    customModelToggles: Record<AiProvider, boolean>;
    apiKeyStatuses: ApiKeyStatus[];
    isLoadingApiKeyStatus: boolean;
    apiKeyStatusError: string | null;
    ollamaStatus: OllamaStatus | null;
    isLoadingOllamaStatus: boolean;
    ollamaStatusError: string | null;
    isRefreshingSetupStatus: boolean;
    isCheckingOllamaConnection: boolean;
    isFetchingModels: boolean;
    fetchingModelsProvider: AiProvider | null;
    isSelectingPath: boolean;
    isLoadingTeamConfig: boolean;
    isSaving: boolean;
    isSaveDisabled: boolean;
    teamValidationMessages: string[];
    teamWarningMessages: string[];
    configuredAiProviderCount: number;
    defaultAiProviderLabel: string;
    poAssistantCliInstalled: boolean;
    poAssistantExecutionLabel: string;
    poAssistantExecutionCaption: string;
    recommendedInitialSection: SettingsSectionId;
    isInitialSectionReady: boolean;
    setProvider: (provider: AiProvider) => void;
    setPoAssistantTransport: (transport: PoAssistantTransport) => void;
    setPoAssistantCliType: (cliType: SupportedCliType) => void;
    setPoAssistantCliModel: (model: string) => void;
    setApiKey: (provider: ApiKeyProvider, value: string) => void;
    setProviderModel: (provider: AiProvider, model: string) => void;
    setOllamaEndpoint: (endpoint: string) => void;
    setPoAssistantAvatarImage: (value: string | null) => void;
    setCustomModelToggle: (provider: AiProvider, enabled: boolean) => void;
    setTeamConfig: (config: TeamConfiguration) => void;
    fetchModels: (provider: AiProvider) => Promise<void>;
    checkOllamaConnection: () => Promise<void>;
    refreshSetupStatus: () => Promise<void>;
    saveSettings: () => Promise<void>;
    selectProjectFolder: () => Promise<void>;
    deleteCurrentProject: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextValue | undefined>(undefined);

const DEFAULT_OLLAMA_ENDPOINT = 'http://localhost:11434';
const DEFAULT_PO_ASSISTANT_CLI_MODELS: Record<SupportedCliType, string> = {
    claude: 'claude-haiku-4-5',
    gemini: 'gemini-3-flash-preview',
    codex: 'gpt-5.4-mini',
};
const DEFAULT_PROVIDER_MODELS: Record<AiProvider, string> = {
    anthropic: 'claude-haiku-4-5',
    gemini: 'gemini-3-flash-preview',
    openai: 'gpt-5.4-mini',
    ollama: 'llama3.2',
};
const PROVIDER_MODEL_STORE_KEYS: Record<AiProvider, string> = {
    anthropic: 'anthropic-model',
    gemini: 'gemini-model',
    openai: 'openai-model',
    ollama: 'ollama-model',
};
const PROVIDER_API_KEY_STORE_KEYS: Record<ApiKeyProvider, string> = {
    anthropic: 'anthropic-api-key',
    gemini: 'gemini-api-key',
    openai: 'openai-api-key',
};
const DEFAULT_TEAM_CONFIGURATION: TeamConfiguration = {
    max_concurrent_agents: 5,
    roles: [],
};
const EMPTY_INSTALLED_CLI_MAP: InstalledCliMap = {
    claude: false,
    gemini: false,
    codex: false,
};
const EMPTY_MODEL_OPTIONS: Record<AiProvider, string[]> = {
    anthropic: [],
    gemini: [],
    openai: [],
    ollama: [],
};
const DEFAULT_CUSTOM_MODEL_TOGGLES: Record<AiProvider, boolean> = {
    anthropic: false,
    gemini: false,
    openai: false,
    ollama: false,
};

const DEFAULT_SETTINGS_DRAFT: SettingsDraft = {
    provider: 'anthropic',
    poAssistantTransport: 'api',
    poAssistantCliType: 'claude',
    poAssistantCliModel: DEFAULT_PO_ASSISTANT_CLI_MODELS.claude,
    apiKeys: {
        anthropic: '',
        gemini: '',
        openai: '',
    },
    providerModels: { ...DEFAULT_PROVIDER_MODELS },
    ollamaEndpoint: DEFAULT_OLLAMA_ENDPOINT,
    poAssistantAvatarImage: null,
    teamConfig: DEFAULT_TEAM_CONFIGURATION,
};

function dispatchSettingsUpdatedEvent() {
    window.dispatchEvent(new Event(VICARA_SETTINGS_UPDATED_EVENT));
}

async function getStoredStringValue(store: SettingsStore, key: string) {
    return normalizeStoredStringValue(await store.get(key));
}

function normalizeAiProvider(value: string | null): AiProvider {
    if (value === 'gemini' || value === 'openai' || value === 'ollama') {
        return value;
    }
    return 'anthropic';
}

function normalizeTransport(value: string | null): PoAssistantTransport {
    return value === 'cli' ? 'cli' : 'api';
}

function normalizeCliType(value: string | null): SupportedCliType {
    if (value === 'gemini' || value === 'codex') {
        return value;
    }
    return 'claude';
}

function buildInstalledCliMap(cliResults: CliDetectionResult[]): InstalledCliMap {
    const nextMap = { ...EMPTY_INSTALLED_CLI_MAP };

    cliResults.forEach((result) => {
        if (result.name === 'claude' || result.name === 'gemini' || result.name === 'codex') {
            nextMap[result.name] = result.installed;
        }
    });

    return nextMap;
}

export function getAiProviderLabel(provider: AiProvider) {
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

export function getCliTypeLabel(cliType: SupportedCliType) {
    switch (cliType) {
        case 'claude':
            return 'Claude Code CLI';
        case 'gemini':
            return 'Gemini CLI';
        case 'codex':
            return 'Codex CLI';
    }
}

export function getDefaultPoAssistantCliModel(cliType: SupportedCliType) {
    return DEFAULT_PO_ASSISTANT_CLI_MODELS[cliType];
}

export function getDefaultModelForProvider(provider: AiProvider) {
    return DEFAULT_PROVIDER_MODELS[provider];
}

export function getQuickSwitchModelSuggestions(provider: AiProvider, currentModel?: string | null) {
    const suggestions = [currentModel?.trim(), DEFAULT_PROVIDER_MODELS[provider]];
    return Array.from(new Set(suggestions.filter((value): value is string => Boolean(value))));
}

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
) {
    if (isCliDetectionLoading) {
        return [];
    }

    const messages: string[] = [];

    config.roles.forEach((role, index) => {
        const cliType = normalizeCliType(role.cli_type.trim());
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

function shouldOpenSetupSection(
    cliResults: { installed: boolean }[],
    apiKeyStatuses: ApiKeyStatus[],
    ollamaStatus: OllamaStatus | null,
) {
    const hasAnyCli = cliResults.some((result) => result.installed);
    const hasAnyApiKey = apiKeyStatuses.some((status) => status.configured);
    const hasAnyPoAssistantProvider = hasAnyApiKey || Boolean(ollamaStatus?.running);
    return !hasAnyCli || !hasAnyPoAssistantProvider;
}

async function loadPersistedAiPreferencesFromStore(store: SettingsStore): Promise<PersistedAiPreferences> {
    const provider = normalizeAiProvider(await getStoredStringValue(store, 'default-ai-provider'));
    const transport = normalizeTransport(await getStoredStringValue(store, 'po-assistant-transport'));

    return {
        provider,
        transport,
        providerModels: {
            anthropic:
                (await getStoredStringValue(store, PROVIDER_MODEL_STORE_KEYS.anthropic)) ??
                DEFAULT_PROVIDER_MODELS.anthropic,
            gemini:
                (await getStoredStringValue(store, PROVIDER_MODEL_STORE_KEYS.gemini)) ??
                DEFAULT_PROVIDER_MODELS.gemini,
            openai:
                (await getStoredStringValue(store, PROVIDER_MODEL_STORE_KEYS.openai)) ??
                DEFAULT_PROVIDER_MODELS.openai,
            ollama:
                (await getStoredStringValue(store, PROVIDER_MODEL_STORE_KEYS.ollama)) ??
                DEFAULT_PROVIDER_MODELS.ollama,
        },
    };
}

export async function readPersistedAiPreferences() {
    const store = await load('settings.json');
    return loadPersistedAiPreferencesFromStore(store);
}

export async function persistQuickSwitch({
    provider,
    model,
    forceApiMode = true,
}: {
    provider: AiProvider;
    model?: string;
    forceApiMode?: boolean;
}) {
    const store = await load('settings.json');
    const current = await loadPersistedAiPreferencesFromStore(store);
    const resolvedModel =
        model?.trim() || current.providerModels[provider] || DEFAULT_PROVIDER_MODELS[provider];

    await store.set('default-ai-provider', { value: provider });
    await store.set(PROVIDER_MODEL_STORE_KEYS[provider], { value: resolvedModel });

    if (forceApiMode) {
        await store.set('po-assistant-transport', { value: 'api' });
    }

    await store.save();
    dispatchSettingsUpdatedEvent();

    return {
        provider,
        model: resolvedModel,
        transport: forceApiMode ? 'api' : current.transport,
    } satisfies {
        provider: AiProvider;
        model: string;
        transport: PoAssistantTransport;
    };
}

export function useAiQuickSwitcher({ forceApiMode = true }: { forceApiMode?: boolean } = {}) {
    const [preferences, setPreferences] = useState<PersistedAiPreferences | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [isSaving, setIsSaving] = useState(false);

    const refresh = useCallback(async () => {
        setIsLoading(true);
        try {
            const nextPreferences = await readPersistedAiPreferences();
            setPreferences(nextPreferences);
        } catch (error) {
            console.error('Failed to load quick switcher settings', error);
            toast.error('現在の AI 設定を読み込めませんでした');
        } finally {
            setIsLoading(false);
        }
    }, []);

    useEffect(() => {
        void refresh();

        const handleSettingsUpdated = () => {
            void refresh();
        };

        window.addEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
        return () => {
            window.removeEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
        };
    }, [refresh]);

    const updateProvider = useCallback(
        async (provider: AiProvider) => {
            setIsSaving(true);
            try {
                const result = await persistQuickSwitch({ provider, forceApiMode });
                setPreferences((prev) => ({
                    provider,
                    transport: result.transport,
                    providerModels: {
                        anthropic: prev?.providerModels.anthropic ?? DEFAULT_PROVIDER_MODELS.anthropic,
                        gemini: prev?.providerModels.gemini ?? DEFAULT_PROVIDER_MODELS.gemini,
                        openai: prev?.providerModels.openai ?? DEFAULT_PROVIDER_MODELS.openai,
                        ollama: prev?.providerModels.ollama ?? DEFAULT_PROVIDER_MODELS.ollama,
                        [provider]: result.model,
                    },
                }));
                toast.success(`${getAiProviderLabel(provider)} に切り替えました`);
            } catch (error) {
                console.error('Failed to switch AI provider', error);
                toast.error(`AIプロバイダーの切り替えに失敗しました: ${error}`);
            } finally {
                setIsSaving(false);
            }
        },
        [forceApiMode],
    );

    const applyModel = useCallback(
        async (model: string) => {
            if (!preferences) {
                return;
            }

            const normalizedModel = model.trim();
            if (!normalizedModel) {
                toast.error('モデル名を入力してください');
                return;
            }

            setIsSaving(true);
            try {
                await persistQuickSwitch({
                    provider: preferences.provider,
                    model: normalizedModel,
                    forceApiMode,
                });
                setPreferences((prev) =>
                    prev
                        ? {
                              ...prev,
                              transport: forceApiMode ? 'api' : prev.transport,
                              providerModels: {
                                  ...prev.providerModels,
                                  [prev.provider]: normalizedModel,
                              },
                          }
                        : prev,
                );
                toast.success(`モデルを ${normalizedModel} に更新しました`);
            } catch (error) {
                console.error('Failed to switch AI model', error);
                toast.error(`モデルの切り替えに失敗しました: ${error}`);
            } finally {
                setIsSaving(false);
            }
        },
        [forceApiMode, preferences],
    );

    return {
        provider: preferences?.provider ?? 'anthropic',
        transport: preferences?.transport ?? 'api',
        providerModels: preferences?.providerModels ?? { ...DEFAULT_PROVIDER_MODELS },
        isLoading,
        isSaving,
        refresh,
        updateProvider,
        applyModel,
    };
}

export function SettingsProvider({ children, onClose, closeOnSave = true }: SettingsProviderProps) {
    const { currentProjectId, deleteProject, projects, updateProjectPath, gitStatus, refreshGitStatus } = useWorkspace();
    const {
        results: cliResults,
        loading: isCliDetectionLoading,
        error: cliDetectionError,
        refresh: refreshCliDetection,
    } = useCliDetection();
    const currentProject = useMemo(
        () => projects.find((project) => project.id === currentProjectId),
        [currentProjectId, projects],
    );

    const [draft, setDraft] = useState<SettingsDraft>(DEFAULT_SETTINGS_DRAFT);
    const [modelOptions, setModelOptions] = useState<Record<AiProvider, string[]>>(EMPTY_MODEL_OPTIONS);
    const [customModelToggles, setCustomModelToggles] = useState<Record<AiProvider, boolean>>(
        DEFAULT_CUSTOM_MODEL_TOGGLES,
    );
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
    const [isRefreshingSetupStatus, setIsRefreshingSetupStatus] = useState(false);
    const [isSelectingPath, setIsSelectingPath] = useState(false);
    const [isFetchingModels, setIsFetchingModels] = useState(false);
    const [fetchingModelsProvider, setFetchingModelsProvider] = useState<AiProvider | null>(null);

    const installedCliMap = useMemo(() => buildInstalledCliMap(cliResults), [cliResults]);

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

    const loadSettings = useCallback(async () => {
        setIsLoadingTeamConfig(true);
        try {
            const [store, loadedTeamConfig, persistedAi] = await Promise.all([
                load('settings.json'),
                invoke<TeamConfiguration>('get_team_configuration'),
                readPersistedAiPreferences(),
            ]);

            const nextCliType = normalizeCliType(await getStoredStringValue(store, 'po-assistant-cli-type'));

            setDraft({
                provider: persistedAi.provider,
                poAssistantTransport: persistedAi.transport,
                poAssistantCliType: nextCliType,
                poAssistantCliModel:
                    (await getStoredStringValue(store, 'po-assistant-cli-model')) ??
                    getDefaultPoAssistantCliModel(nextCliType),
                apiKeys: {
                    anthropic: (await getStoredStringValue(store, PROVIDER_API_KEY_STORE_KEYS.anthropic)) ?? '',
                    gemini: (await getStoredStringValue(store, PROVIDER_API_KEY_STORE_KEYS.gemini)) ?? '',
                    openai: (await getStoredStringValue(store, PROVIDER_API_KEY_STORE_KEYS.openai)) ?? '',
                },
                providerModels: persistedAi.providerModels,
                ollamaEndpoint:
                    (await getStoredStringValue(store, 'ollama-endpoint')) ?? DEFAULT_OLLAMA_ENDPOINT,
                poAssistantAvatarImage: await getStoredStringValue(store, PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY),
                teamConfig: loadedTeamConfig,
            });
            setModelOptions(EMPTY_MODEL_OPTIONS);
            setCustomModelToggles(DEFAULT_CUSTOM_MODEL_TOGGLES);
        } catch (error) {
            console.error('Failed to load settings', error);
            toast.error(`設定の読み込みに失敗しました: ${error}`);
        } finally {
            setIsLoadingTeamConfig(false);
        }
    }, []);

    const refreshSetupStatus = useCallback(
        async (showSpinner = true) => {
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
        },
        [loadApiKeyStatus, loadOllamaStatus, refreshCliDetection, refreshGitStatus],
    );

    useEffect(() => {
        void loadSettings();
        void refreshSetupStatus(false);
    }, [loadSettings, refreshSetupStatus]);

    const setProvider = useCallback((provider: AiProvider) => {
        setDraft((prev) => ({ ...prev, provider }));
    }, []);

    const setPoAssistantTransport = useCallback((transport: PoAssistantTransport) => {
        setDraft((prev) => ({ ...prev, poAssistantTransport: transport }));
    }, []);

    const setPoAssistantCliType = useCallback((cliType: SupportedCliType) => {
        setDraft((prev) => ({
            ...prev,
            poAssistantCliType: cliType,
            poAssistantCliModel: getDefaultPoAssistantCliModel(cliType),
        }));
    }, []);

    const setPoAssistantCliModel = useCallback((model: string) => {
        setDraft((prev) => ({ ...prev, poAssistantCliModel: model }));
    }, []);

    const setApiKey = useCallback((provider: ApiKeyProvider, value: string) => {
        setDraft((prev) => ({
            ...prev,
            apiKeys: {
                ...prev.apiKeys,
                [provider]: value,
            },
        }));
    }, []);

    const setProviderModel = useCallback((provider: AiProvider, model: string) => {
        setDraft((prev) => ({
            ...prev,
            providerModels: {
                ...prev.providerModels,
                [provider]: model,
            },
        }));
    }, []);

    const setOllamaEndpoint = useCallback((endpoint: string) => {
        setDraft((prev) => ({ ...prev, ollamaEndpoint: endpoint }));
    }, []);

    const setPoAssistantAvatarImage = useCallback((value: string | null) => {
        setDraft((prev) => ({ ...prev, poAssistantAvatarImage: value }));
    }, []);

    const setCustomModelToggle = useCallback(
        (provider: AiProvider, enabled: boolean) => {
            setCustomModelToggles((prev) => ({
                ...prev,
                [provider]: enabled,
            }));

            if (!enabled) {
                setDraft((prev) => {
                    const suggestions = modelOptions[provider];
                    if (suggestions.length === 0 || suggestions.includes(prev.providerModels[provider])) {
                        return prev;
                    }

                    return {
                        ...prev,
                        providerModels: {
                            ...prev.providerModels,
                            [provider]: suggestions[0],
                        },
                    };
                });
            }
        },
        [modelOptions],
    );

    const setTeamConfig = useCallback((config: TeamConfiguration) => {
        setDraft((prev) => ({ ...prev, teamConfig: config }));
    }, []);

    const fetchModels = useCallback(
        async (targetProvider: AiProvider) => {
            setIsFetchingModels(true);
            setFetchingModelsProvider(targetProvider);

            const currentModel = draft.providerModels[targetProvider];
            const isCustom = customModelToggles[targetProvider];
            const apiKeyOverride =
                targetProvider === 'anthropic'
                    ? draft.apiKeys.anthropic
                    : targetProvider === 'gemini'
                      ? draft.apiKeys.gemini
                      : targetProvider === 'openai'
                        ? draft.apiKeys.openai
                        : undefined;
            const endpointOverride = targetProvider === 'ollama' ? draft.ollamaEndpoint : undefined;

            try {
                const models = await invoke<string[]>('get_available_models', {
                    provider: targetProvider,
                    apiKeyOverride,
                    endpointOverride,
                });

                setModelOptions((prev) => ({
                    ...prev,
                    [targetProvider]: models,
                }));

                if (models.length > 0 && !isCustom && !models.includes(currentModel)) {
                    setProviderModel(targetProvider, models[0]);
                }

                if (targetProvider === 'ollama') {
                    setOllamaStatus({
                        running: true,
                        models,
                        endpoint: draft.ollamaEndpoint.trim() || DEFAULT_OLLAMA_ENDPOINT,
                        message: null,
                    });
                    setOllamaStatusError(null);
                }

                toast.success(`${getAiProviderLabel(targetProvider)} のモデル一覧を取得しました`);
            } catch (error) {
                console.error(`Failed to fetch ${targetProvider} models`, error);
                toast.error(`モデルの取得に失敗しました。設定値を確認してください。\nError: ${error}`);
            } finally {
                setIsFetchingModels(false);
                setFetchingModelsProvider(null);
            }
        },
        [customModelToggles, draft.apiKeys, draft.ollamaEndpoint, draft.providerModels, setProviderModel],
    );

    const checkOllamaConnection = useCallback(async () => {
        setIsCheckingOllamaConnection(true);
        try {
            const status = await loadOllamaStatus(draft.ollamaEndpoint);
            if (!status) {
                setModelOptions((prev) => ({ ...prev, ollama: [] }));
                toast.error('Ollama の接続確認に失敗しました');
                return;
            }

            setModelOptions((prev) => ({ ...prev, ollama: status.models }));

            if (status.running) {
                if (
                    status.models.length > 0 &&
                    !customModelToggles.ollama &&
                    !status.models.includes(draft.providerModels.ollama)
                ) {
                    setProviderModel('ollama', status.models[0]);
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
    }, [
        customModelToggles.ollama,
        draft.ollamaEndpoint,
        draft.providerModels.ollama,
        loadOllamaStatus,
        setProviderModel,
    ]);

    const selectProjectFolder = useCallback(async () => {
        setIsSelectingPath(true);
        try {
            const selectedPath = await open({
                directory: true,
                multiple: false,
                title: 'プロジェクトのディレクトリを選択してください',
            });

            if (selectedPath && typeof selectedPath === 'string') {
                const result = await updateProjectPath(currentProjectId, selectedPath);
                if (result.success) {
                    toast.success('ワークスペースのディレクトリを設定しました');
                }
            }
        } catch (error) {
            console.error('Failed to select directory', error);
            toast.error('ディレクトリの選択に失敗しました');
        } finally {
            setIsSelectingPath(false);
        }
    }, [currentProjectId, updateProjectPath]);

    const deleteCurrentProject = useCallback(async () => {
        if (!currentProjectId || currentProjectId === 'default') {
            toast.error('このプロジェクトは削除できません');
            return;
        }

        const confirmed = await confirm(
            '本当にこのプロジェクトを削除しますか？\n紐づくすべてのバックログやスプリントデータが消去されます。',
            { title: 'プロジェクトの削除確認', kind: 'warning' },
        );
        if (!confirmed) {
            return;
        }

        try {
            await deleteProject(currentProjectId);
            onClose();
        } catch {
            // Error toast is handled in WorkspaceContext.
        }
    }, [currentProjectId, deleteProject, onClose]);

    const teamValidationMessages = useMemo(
        () => validateTeamConfiguration(draft.teamConfig),
        [draft.teamConfig],
    );
    const teamWarningMessages = useMemo(
        () => collectTeamConfigurationWarnings(draft.teamConfig, installedCliMap, isCliDetectionLoading),
        [draft.teamConfig, installedCliMap, isCliDetectionLoading],
    );
    const configuredAiProviderCount = useMemo(
        () =>
            Number(Boolean(draft.apiKeys.anthropic.trim())) +
            Number(Boolean(draft.apiKeys.gemini.trim())) +
            Number(Boolean(draft.apiKeys.openai.trim())) +
            Number(Boolean(ollamaStatus?.running)),
        [draft.apiKeys.anthropic, draft.apiKeys.gemini, draft.apiKeys.openai, ollamaStatus?.running],
    );
    const defaultAiProviderLabel = useMemo(
        () => getAiProviderLabel(draft.provider),
        [draft.provider],
    );
    const poAssistantCliInstalled = installedCliMap[draft.poAssistantCliType];
    const poAssistantExecutionLabel =
        draft.poAssistantTransport === 'api'
            ? defaultAiProviderLabel
            : getCliTypeLabel(draft.poAssistantCliType);
    const poAssistantExecutionCaption =
        draft.poAssistantTransport === 'api' ? 'API モード' : 'CLI モード';
    const isSaveDisabled =
        isSaving || isLoadingTeamConfig || teamValidationMessages.length > 0;
    const isInitialSectionReady =
        !isCliDetectionLoading && hasLoadedApiKeyStatus && hasLoadedOllamaStatus;
    const recommendedInitialSection: SettingsSectionId = !currentProject?.local_path
        ? 'project'
        : isInitialSectionReady && shouldOpenSetupSection(cliResults, apiKeyStatuses, ollamaStatus)
            ? 'setup'
            : 'ai-provider';

    const saveSettings = useCallback(async () => {
        if (teamValidationMessages.length > 0) {
            toast.error(teamValidationMessages[0]);
            return;
        }

        setIsSaving(true);
        try {
            const store = await load('settings.json');
            await store.set('default-ai-provider', { value: draft.provider });
            await store.set('po-assistant-transport', { value: draft.poAssistantTransport });
            await store.set('po-assistant-cli-type', { value: draft.poAssistantCliType });
            await store.set('po-assistant-cli-model', {
                value: draft.poAssistantCliModel.trim() || getDefaultPoAssistantCliModel(draft.poAssistantCliType),
            });
            await store.set(PROVIDER_API_KEY_STORE_KEYS.anthropic, { value: draft.apiKeys.anthropic });
            await store.set(PROVIDER_API_KEY_STORE_KEYS.gemini, { value: draft.apiKeys.gemini });
            await store.set(PROVIDER_API_KEY_STORE_KEYS.openai, { value: draft.apiKeys.openai });
            await store.set('ollama-endpoint', {
                value: draft.ollamaEndpoint.trim() || DEFAULT_OLLAMA_ENDPOINT,
            });
            await store.set(PROVIDER_MODEL_STORE_KEYS.anthropic, {
                value: draft.providerModels.anthropic.trim() || DEFAULT_PROVIDER_MODELS.anthropic,
            });
            await store.set(PROVIDER_MODEL_STORE_KEYS.gemini, {
                value: draft.providerModels.gemini.trim() || DEFAULT_PROVIDER_MODELS.gemini,
            });
            await store.set(PROVIDER_MODEL_STORE_KEYS.openai, {
                value: draft.providerModels.openai.trim() || DEFAULT_PROVIDER_MODELS.openai,
            });
            await store.set(PROVIDER_MODEL_STORE_KEYS.ollama, {
                value: draft.providerModels.ollama.trim() || DEFAULT_PROVIDER_MODELS.ollama,
            });
            await store.set(PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY, {
                value: draft.poAssistantAvatarImage ?? '',
            });
            await store.save();
            await invoke('save_team_configuration', { config: draft.teamConfig });
            dispatchSettingsUpdatedEvent();
            toast.success('設定を保存しました');
            if (closeOnSave) {
                onClose();
            }
        } catch (error) {
            console.error('Failed to save settings', error);
            toast.error('設定の保存に失敗しました');
        } finally {
            setIsSaving(false);
        }
    }, [closeOnSave, draft, onClose, teamValidationMessages]);

    const value = useMemo<SettingsContextValue>(
        () => ({
            currentProjectId,
            currentProject,
            gitStatus,
            draft,
            cliResults,
            isCliDetectionLoading,
            cliDetectionError,
            installedCliMap,
            modelOptions,
            customModelToggles,
            apiKeyStatuses,
            isLoadingApiKeyStatus,
            apiKeyStatusError,
            ollamaStatus,
            isLoadingOllamaStatus,
            ollamaStatusError,
            isRefreshingSetupStatus,
            isCheckingOllamaConnection,
            isFetchingModels,
            fetchingModelsProvider,
            isSelectingPath,
            isLoadingTeamConfig,
            isSaving,
            isSaveDisabled,
            teamValidationMessages,
            teamWarningMessages,
            configuredAiProviderCount,
            defaultAiProviderLabel,
            poAssistantCliInstalled,
            poAssistantExecutionLabel,
            poAssistantExecutionCaption,
            recommendedInitialSection,
            isInitialSectionReady,
            setProvider,
            setPoAssistantTransport,
            setPoAssistantCliType,
            setPoAssistantCliModel,
            setApiKey,
            setProviderModel,
            setOllamaEndpoint,
            setPoAssistantAvatarImage,
            setCustomModelToggle,
            setTeamConfig,
            fetchModels,
            checkOllamaConnection,
            refreshSetupStatus: async () => {
                try {
                    await refreshSetupStatus();
                    toast.success('セットアップ状況を更新しました');
                } catch (error) {
                    console.error('Failed to refresh setup status', error);
                    toast.error(`セットアップ状況の更新に失敗しました: ${error}`);
                }
            },
            saveSettings,
            selectProjectFolder,
            deleteCurrentProject,
        }),
        [
            apiKeyStatusError,
            apiKeyStatuses,
            checkOllamaConnection,
            cliDetectionError,
            cliResults,
            configuredAiProviderCount,
            currentProject,
            currentProjectId,
            customModelToggles,
            defaultAiProviderLabel,
            deleteCurrentProject,
            draft,
            fetchModels,
            fetchingModelsProvider,
            gitStatus,
            installedCliMap,
            isCheckingOllamaConnection,
            isCliDetectionLoading,
            isFetchingModels,
            isInitialSectionReady,
            isLoadingApiKeyStatus,
            isLoadingOllamaStatus,
            isLoadingTeamConfig,
            isRefreshingSetupStatus,
            isSaveDisabled,
            isSaving,
            isSelectingPath,
            modelOptions,
            ollamaStatus,
            ollamaStatusError,
            poAssistantCliInstalled,
            poAssistantExecutionCaption,
            poAssistantExecutionLabel,
            recommendedInitialSection,
            refreshSetupStatus,
            saveSettings,
            selectProjectFolder,
            setApiKey,
            setCustomModelToggle,
            setOllamaEndpoint,
            setPoAssistantAvatarImage,
            setPoAssistantCliModel,
            setPoAssistantCliType,
            setPoAssistantTransport,
            setProvider,
            setProviderModel,
            setTeamConfig,
            teamValidationMessages,
            teamWarningMessages,
        ],
    );

    return (
        <SettingsContext.Provider value={value}>
            {children}
        </SettingsContext.Provider>
    );
}

export function useSettings() {
    const context = useContext(SettingsContext);
    if (!context) {
        throw new Error('useSettings must be used within a SettingsProvider');
    }
    return context;
}
