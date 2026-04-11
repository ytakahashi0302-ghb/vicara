import { CheckCircle2, Info, Server, XCircle } from 'lucide-react';
import { Button } from '../../Button';
import {
    getAiProviderLabel,
    useSettings,
} from '../SettingsContext';

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
            className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs font-medium ${
                configured
                    ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
                    : 'border-slate-200 bg-slate-50 text-slate-600'
            }`}
        >
            {configured ? <CheckCircle2 size={12} /> : <XCircle size={12} />}
            {configured ? configuredLabel : unconfiguredLabel}
        </span>
    );
}

function ProviderCard({
    title,
    subtitle,
    description,
    configured,
    children,
    badge,
}: {
    title: string;
    subtitle: string;
    description: string;
    configured: boolean;
    children: React.ReactNode;
    badge?: React.ReactNode;
}) {
    return (
        <div className="rounded-xl border border-slate-200 bg-white p-5">
            <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                    <div className="text-[11px] font-semibold uppercase tracking-[0.16em] text-slate-400">
                        {title}
                    </div>
                    <div className="mt-1 text-base font-semibold text-slate-900">{subtitle}</div>
                    <p className="mt-1 text-sm leading-6 text-slate-500">{description}</p>
                </div>

                <div className="shrink-0">
                    {badge ?? <ConfigurationBadge configured={configured} />}
                </div>
            </div>

            <div className="mt-5 border-t border-slate-200/80 pt-5">{children}</div>
        </div>
    );
}

export function AiProviderSection() {
    const {
        draft,
        ollamaStatus,
        ollamaStatusError,
        isCheckingOllamaConnection,
        setApiKey,
        setOllamaEndpoint,
        checkOllamaConnection,
    } = useSettings();

    const anthropicConfigured = Boolean(draft.apiKeys.anthropic.trim());
    const geminiConfigured = Boolean(draft.apiKeys.gemini.trim());
    const openaiConfigured = Boolean(draft.apiKeys.openai.trim());
    const ollamaConfigured = Boolean(ollamaStatus?.running);
    const inputClass =
        'h-10 w-full max-w-xl rounded-xl border border-slate-300 bg-white px-3 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500';

    return (
        <div className="space-y-6">
            <div className="rounded-xl border border-slate-200 bg-slate-50 p-4">
                <div className="flex items-start gap-3">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-white text-slate-500 shadow-sm">
                        <Info size={16} />
                    </div>
                    <div className="min-w-0">
                        <div className="text-sm font-semibold text-slate-900">補足</div>
                        <ul className="mt-2 space-y-1 text-sm leading-6 text-slate-600">
                            <li>この画面では各 AI プロバイダーの API キーや接続先だけを登録します。</li>
                            <li>どのプロバイダーやモデルを実際に使うかは、POアシスタント設定や各画面のクイックスイッチャー側で選択します。</li>
                            <li>Ollama はローカル接続先の登録と稼働確認までここで行えます。</li>
                        </ul>
                    </div>
                </div>
            </div>

            <div className="space-y-4">
                <ProviderCard
                    title="Google Gemini"
                    subtitle={getAiProviderLabel('gemini')}
                    description="Gemini 系モデルを使うための API キーを登録します。"
                    configured={geminiConfigured}
                >
                    <div>
                        <label className="mb-2 block text-sm font-medium text-slate-700">API キー</label>
                        <input
                            type="password"
                            placeholder="AIza..."
                            value={draft.apiKeys.gemini}
                            onChange={(event) => setApiKey('gemini', event.target.value)}
                            className={inputClass}
                        />
                    </div>
                </ProviderCard>

                <ProviderCard
                    title="Anthropic"
                    subtitle={getAiProviderLabel('anthropic')}
                    description="Claude 系モデルを使うための API キーを登録します。"
                    configured={anthropicConfigured}
                >
                    <div>
                        <label className="mb-2 block text-sm font-medium text-slate-700">API キー</label>
                        <input
                            type="password"
                            placeholder="sk-ant-..."
                            value={draft.apiKeys.anthropic}
                            onChange={(event) => setApiKey('anthropic', event.target.value)}
                            className={inputClass}
                        />
                    </div>
                </ProviderCard>

                <ProviderCard
                    title="OpenAI"
                    subtitle={getAiProviderLabel('openai')}
                    description="OpenAI 系モデルを使うための API キーを登録します。"
                    configured={openaiConfigured}
                >
                    <div>
                        <label className="mb-2 block text-sm font-medium text-slate-700">API キー</label>
                        <input
                            type="password"
                            placeholder="sk-proj-..."
                            value={draft.apiKeys.openai}
                            onChange={(event) => setApiKey('openai', event.target.value)}
                            className={inputClass}
                        />
                    </div>
                </ProviderCard>

                <ProviderCard
                    title="Ollama"
                    subtitle="Ollama (ローカル LLM)"
                    description="ローカル LLM の接続先を登録し、接続テストで稼働確認を行います。"
                    configured={ollamaConfigured}
                    badge={
                        <ConfigurationBadge
                            configured={ollamaConfigured}
                            configuredLabel="稼働中"
                            unconfiguredLabel="未稼働"
                        />
                    }
                >
                    <div>
                        <div className="mb-2 flex flex-wrap items-center justify-between gap-3">
                            <label className="text-sm font-medium text-slate-700">エンドポイント</label>
                            <Button
                                type="button"
                                size="sm"
                                variant="ghost"
                                onClick={() => void checkOllamaConnection()}
                                disabled={isCheckingOllamaConnection}
                                className="border border-slate-200 bg-white text-slate-600 hover:bg-slate-50"
                            >
                                <Server size={12} className="mr-2" />
                                {isCheckingOllamaConnection ? '確認中...' : '接続テスト'}
                            </Button>
                        </div>
                        <input
                            type="text"
                            placeholder="http://localhost:11434"
                            value={draft.ollamaEndpoint}
                            onChange={(event) => setOllamaEndpoint(event.target.value)}
                            className={inputClass}
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
                        {ollamaStatusError && (
                            <div className="mt-2 rounded-xl border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-700">
                                {ollamaStatusError}
                            </div>
                        )}
                    </div>
                </ProviderCard>
            </div>
        </div>
    );
}
