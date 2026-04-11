import { useId, useState } from 'react';
import { Sparkles } from 'lucide-react';
import { Button } from '../Button';
import { cn } from '../Modal';
import {
    type AiProvider,
    getAiProviderLabel,
    getQuickSwitchModelSuggestions,
    useAiQuickSwitcher,
} from './SettingsContext';

interface AiQuickSwitcherProps {
    className?: string;
    compact?: boolean;
    forceApiMode?: boolean;
}

const PROVIDER_OPTIONS: AiProvider[] = ['anthropic', 'gemini', 'openai', 'ollama'];

export function AiQuickSwitcher({
    className,
    compact = false,
    forceApiMode = true,
}: AiQuickSwitcherProps) {
    const { provider, transport, providerModels, isLoading, isSaving, updateProvider, applyModel } =
        useAiQuickSwitcher({ forceApiMode });
    const [draftModelsByProvider, setDraftModelsByProvider] = useState<Record<string, string>>({});
    const datalistId = useId();

    const draftModel = draftModelsByProvider[provider] ?? providerModels[provider] ?? '';
    const suggestions = getQuickSwitchModelSuggestions(provider, providerModels[provider]);
    const isDirty = draftModel.trim() !== (providerModels[provider] ?? '').trim();

    return (
        <div
            className={cn(
                'rounded-xl border border-slate-200 bg-white',
                compact ? 'p-3' : 'p-4',
                className,
            )}
        >
            <div
                className={cn(
                    'gap-3',
                    compact ? 'flex flex-col' : 'flex flex-col',
                )}
            >
                <div className="flex items-center gap-2">
                    <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-slate-100 text-slate-500">
                        <Sparkles size={15} />
                    </div>
                    <div className="min-w-0">
                        <div className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-400">
                            Quick Switch
                        </div>
                        <div className="text-sm font-semibold text-slate-900">
                            AI プロバイダー / モデル
                        </div>
                    </div>
                </div>

                <div
                    className={cn(
                        'grid gap-3',
                        compact ? 'md:grid-cols-[minmax(0,180px)_minmax(0,1fr)_auto]' : 'lg:grid-cols-[220px_minmax(0,1fr)_auto]',
                    )}
                >
                    <label className="grid gap-1">
                        <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-500">
                            Provider
                        </span>
                        <select
                            value={provider}
                            onChange={(event) => void updateProvider(event.target.value as AiProvider)}
                            disabled={isLoading || isSaving}
                            className="h-10 rounded-xl border border-slate-300 bg-white px-3 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                        >
                            {PROVIDER_OPTIONS.map((option) => (
                                <option key={option} value={option}>
                                    {getAiProviderLabel(option)}
                                </option>
                            ))}
                        </select>
                    </label>

                    <label className="grid gap-1">
                        <span className="text-[11px] font-semibold uppercase tracking-[0.14em] text-slate-500">
                            Model
                        </span>
                        <input
                            list={datalistId}
                            value={draftModel}
                            onChange={(event) =>
                                setDraftModelsByProvider((prev) => ({
                                    ...prev,
                                    [provider]: event.target.value,
                                }))
                            }
                            onKeyDown={(event) => {
                                if (event.key === 'Enter' && isDirty && !isSaving) {
                                    event.preventDefault();
                                    void applyModel(draftModel);
                                }
                            }}
                            disabled={isLoading || isSaving}
                            className="h-10 rounded-xl border border-slate-300 bg-white px-3 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500"
                        />
                        <datalist id={datalistId}>
                            {suggestions.map((item) => (
                                <option key={item} value={item} />
                            ))}
                        </datalist>
                    </label>

                    <div className="flex items-end">
                        <Button
                            type="button"
                            variant="secondary"
                            onClick={() => void applyModel(draftModel)}
                            disabled={isLoading || isSaving || !draftModel.trim() || !isDirty}
                            className="w-full whitespace-nowrap border border-slate-200 bg-slate-50 text-slate-700 hover:bg-slate-100"
                        >
                            {isSaving ? '適用中...' : 'モデルを適用'}
                        </Button>
                    </div>
                </div>

                <p className="text-xs leading-5 text-slate-500">
                    {transport === 'cli' && forceApiMode
                        ? '現在は CLI モードです。ここで切り替えると PO アシスタント系画面は API モードへ戻して即時反映します。'
                        : '変更は次の AI リクエストから即時反映されます。詳細な API キーやモデル取得は設定画面で管理できます。'}
                </p>
            </div>
        </div>
    );
}
