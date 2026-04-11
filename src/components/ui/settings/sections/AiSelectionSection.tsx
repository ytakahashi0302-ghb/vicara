import { Info } from 'lucide-react';
import { SettingsField } from '../SettingsField';
import { getAiProviderLabel, useSettings } from '../SettingsContext';

/**
 * AiSelectionSection (EPIC45 Phase Z)
 *
 * - グラデーション背景を撤去
 * - 選択状態は ring-2 ring-blue-500 のみ
 */
function ProviderSelectionCard({
    provider,
    selected,
    model,
    onSelect,
}: {
    provider: 'anthropic' | 'gemini' | 'openai' | 'ollama';
    selected: boolean;
    model: string;
    onSelect: (provider: 'anthropic' | 'gemini' | 'openai' | 'ollama') => void;
}) {
    return (
        <button
            type="button"
            onClick={() => onSelect(provider)}
            className={`rounded-xl border p-4 text-left transition-colors ${
                selected
                    ? 'border-slate-200 bg-white ring-2 ring-blue-500'
                    : 'border-slate-200 bg-white hover:bg-slate-50'
            }`}
        >
            <div className="text-sm font-semibold text-slate-900">{getAiProviderLabel(provider)}</div>
            <div className="mt-1 text-xs text-slate-500">
                {selected ? '現在の既定値' : '既定値へ切り替える'}
            </div>
            <div className="mt-3 text-xs font-medium text-slate-500">Model: {model}</div>
        </button>
    );
}

export function AiSelectionSection() {
    const { draft, setProvider } = useSettings();

    return (
        <div className="space-y-6">
            <div className="rounded-xl border border-slate-200 bg-white p-5">
                <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                    <div className="max-w-3xl">
                        <h4 className="text-lg font-semibold text-slate-900">
                            ふだん使う AI を選ぶ
                        </h4>
                        <p className="mt-1 text-sm leading-6 text-slate-500">
                            POアシスタントの API モードや Inception Deck など、AiProvider を使う画面の既定ルーティング先をここで決めます。
                        </p>
                    </div>
                    <div className="rounded-xl border border-slate-200 bg-slate-50 px-4 py-3">
                        <div className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-400">
                            Current Default
                        </div>
                        <div className="mt-1 text-sm font-semibold text-slate-900">
                            {getAiProviderLabel(draft.provider)}
                        </div>
                        <div className="mt-1 text-xs text-slate-500">
                            {draft.providerModels[draft.provider]}
                        </div>
                    </div>
                </div>
            </div>

            <div className="rounded-xl border border-slate-200 bg-slate-50 p-4">
                <div className="flex items-start gap-3">
                    <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-white text-slate-500 shadow-sm">
                        <Info size={16} />
                    </div>
                    <div className="min-w-0">
                        <div className="text-sm font-semibold text-slate-900">補足</div>
                        <ul className="mt-2 space-y-1 text-sm leading-6 text-slate-600">
                            <li>API キーや Ollama 接続先の準備は「APIキー / 接続情報」でまとめて行います。</li>
                            <li>ここでは、日常的に使う AI の系統と既定モデルを決める役割に絞っています。</li>
                        </ul>
                    </div>
                </div>
            </div>

            <SettingsField
                label="既定の AI プロバイダー"
                description="ここで選んだプロバイダーが、API モード時の標準利用先になります。"
            >
                <div className="grid gap-3 lg:grid-cols-2 xl:grid-cols-4">
                    {(['anthropic', 'gemini', 'openai', 'ollama'] as const).map((provider) => (
                        <ProviderSelectionCard
                            key={provider}
                            provider={provider}
                            selected={draft.provider === provider}
                            model={draft.providerModels[provider]}
                            onSelect={setProvider}
                        />
                    ))}
                </div>
            </SettingsField>
        </div>
    );
}
