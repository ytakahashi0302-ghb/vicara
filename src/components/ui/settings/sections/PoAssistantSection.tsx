import { useRef, type ChangeEvent } from 'react';
import {
    Braces,
    ImagePlus,
    MonitorSmartphone,
    RotateCcw,
    TerminalSquare,
    Zap,
} from 'lucide-react';
import { Button } from '../../Button';
import {
    getAvatarDefinition,
    resolveAvatarImageSource,
} from '../../../ai/avatarRegistry';
import {
    getDefaultModelForProvider,
    getAiProviderLabel,
    getCliTypeLabel,
    getDefaultPoAssistantCliModel,
    useSettings,
} from '../SettingsContext';

/**
 * PoAssistantSection (EPIC45 Phase ZZZZZ)
 *
 * 実行モードを大きなカードで選び、その下に実行詳細と画像設定を
 * まとめて配置する。既存要素は減らさず、視線移動を短くする。
 */

function SelectionDot({ selected }: { selected: boolean }) {
    return (
        <span
            className={`inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full border transition-colors ${selected
                    ? 'border-blue-500 bg-blue-50'
                    : 'border-slate-300 bg-white'
                }`}
            aria-hidden="true"
        >
            <span
                className={`h-2.5 w-2.5 rounded-full transition-colors ${selected ? 'bg-blue-500' : 'bg-transparent'
                    }`}
            />
        </span>
    );
}

function ExecutionModeCard({
    title,
    description,
    caption,
    icon,
    selected,
    onSelect,
}: {
    title: string;
    description: string;
    caption: string;
    icon: typeof MonitorSmartphone;
    selected: boolean;
    onSelect: () => void;
}) {
    const Icon = icon;

    return (
        <label
            className={`cursor-pointer rounded-xl border p-5 transition-colors ${selected
                    ? 'border-blue-200 bg-white shadow-sm ring-2 ring-blue-500'
                    : 'border-slate-200 bg-white hover:bg-slate-50'
                }`}
        >
            <input
                type="radio"
                checked={selected}
                onChange={onSelect}
                className="hidden"
            />
            <div className="flex items-start justify-between gap-4">
                <div className="flex h-12 w-12 shrink-0 items-center justify-center rounded-xl bg-slate-100 text-slate-600">
                    <Icon size={20} />
                </div>
                <SelectionDot selected={selected} />
            </div>

            <div className="mt-5">
                <div className="text-xl font-semibold text-slate-900">{title}</div>
                <p className="mt-2 text-sm leading-6 text-slate-500">{description}</p>
                <p className="mt-3 text-xs font-medium leading-5 text-slate-500">{caption}</p>
            </div>
        </label>
    );
}

function PoAssistantImagePanel({
    value,
    onChange,
}: {
    value: string | null;
    onChange: (value: string | null) => void;
}) {
    const inputRef = useRef<HTMLInputElement>(null);
    const defaultDefinition = getAvatarDefinition('po-assistant');
    const previewImage = resolveAvatarImageSource(value) ?? defaultDefinition.src;

    const handleChooseImage = () => {
        inputRef.current?.click();
    };

    const handleFileChange = (event: ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0];
        if (!file) {
            return;
        }

        const reader = new FileReader();
        reader.onload = () => {
            if (typeof reader.result === 'string') {
                onChange(reader.result);
            }
        };
        reader.readAsDataURL(file);
        event.target.value = '';
    };

    return (
        <div className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
            <div className="text-sm font-semibold text-slate-900">POアシスタント画像</div>
            <p className="mt-1 text-sm leading-6 text-slate-500">
                サイドバーと Inception Deck に表示するアバター / 立ち絵を設定します。
            </p>

            <div className="mt-4 rounded-[20px] border border-slate-200 bg-slate-900 p-3">
                <div className="flex min-h-[280px] items-center justify-center overflow-hidden rounded-[16px] bg-[radial-gradient(circle_at_top,#1f2937,#0f172a_65%)]">
                    <img
                        src={previewImage}
                        alt="POアシスタント画像プレビュー"
                        className="h-[260px] w-full object-contain p-3"
                    />
                </div>
            </div>

            <div className="mt-4 flex flex-wrap gap-2">
                <Button type="button" variant="secondary" onClick={handleChooseImage}>
                    <ImagePlus size={15} className="mr-2" />
                    画像を選択
                </Button>
                <Button type="button" variant="ghost" onClick={() => onChange(null)}>
                    <RotateCcw size={15} className="mr-2" />
                    デフォルトに戻す
                </Button>
            </div>

            <input
                ref={inputRef}
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                className="hidden"
                onChange={handleFileChange}
            />
        </div>
    );
}

export function PoAssistantSection() {
    const {
        draft,
        configuredAiProviderCount,
        defaultAiProviderLabel,
        modelOptions,
        poAssistantCliInstalled,
        setProvider,
        setProviderModel,
        setPoAssistantAvatarImage,
        setPoAssistantTransport,
        setPoAssistantCliType,
        setPoAssistantCliModel,
    } = useSettings();

    const inputClass =
        'h-10 w-full max-w-md rounded-xl border border-slate-300 bg-white px-3 text-sm text-slate-700 focus:outline-none focus:ring-2 focus:ring-blue-500';
    const apiModelSuggestions = modelOptions[draft.provider];
    const apiModelSuggestionsId = `po-assistant-api-models-${draft.provider}`;

    return (
        <div className="space-y-6">
            <div className="rounded-xl border border-slate-200 bg-white p-5">
                <div className="flex flex-col gap-3">
                    <div className="flex flex-wrap items-center gap-3">
                        <h4 className="text-lg font-semibold text-slate-900">POアシスタントの見た目と実行方式</h4>
                        <span className="inline-flex items-center rounded-full border border-slate-200 bg-slate-50 px-3 py-1 text-xs font-medium text-slate-700">
                            {draft.poAssistantTransport === 'api'
                                ? `${defaultAiProviderLabel} / ${draft.providerModels[draft.provider]}`
                                : `${getCliTypeLabel(draft.poAssistantCliType)} / ${draft.poAssistantCliModel}`}
                        </span>
                    </div>
                    <p className="max-w-3xl text-sm leading-6 text-slate-500">
                    </p>
                </div>
            </div>

            <div className="grid gap-6 xl:grid-cols-[minmax(0,1.7fr)_340px]">
                <div className="rounded-xl border border-slate-200 bg-white p-5">
                    <div className="flex items-center gap-2 text-slate-900">
                        <Zap size={18} className="text-blue-600" />
                        <h5 className="text-lg font-semibold">実行方式</h5>
                    </div>
                    <p className="mt-1 text-sm leading-6 text-slate-500">
                        API と CLI のどちらで動かすかを選び、そのまま直下で必要な詳細設定を完了できます。
                    </p>

                    <div className="mt-5 grid gap-4 lg:grid-cols-2">
                        <ExecutionModeCard
                            title="APIモード"
                            description="クラウドで実行する AI API エンドポイント経由でタスクを実行します。サーバーレスのスケーリングに適しています。"
                            caption={`現在: ${getAiProviderLabel(draft.provider)} / ${draft.providerModels[draft.provider]}`}
                            icon={MonitorSmartphone}
                            selected={draft.poAssistantTransport === 'api'}
                            onSelect={() => setPoAssistantTransport('api')}
                        />

                        <ExecutionModeCard
                            title="CLIモード"
                            description="ローカル環境で直接コマンドを実行します。CLI/CD パイプラインや手元の開発フローに馴染みやすい構成です。"
                            caption={`現在: ${getCliTypeLabel(draft.poAssistantCliType)} / ${draft.poAssistantCliModel}`}
                            icon={TerminalSquare}
                            selected={draft.poAssistantTransport === 'cli'}
                            onSelect={() => setPoAssistantTransport('cli')}
                        />
                    </div>

                    <div className="mt-6 rounded-xl border border-slate-200 bg-slate-50 p-4">
                        <div className="flex items-center gap-2 text-slate-900">
                            <Braces size={18} className="text-blue-600" />
                            <h6 className="text-base font-semibold">実行詳細</h6>
                        </div>

                        {draft.poAssistantTransport === 'api' ? (
                            <div className="mt-4 space-y-4">
                                <div className="grid gap-4 sm:grid-cols-2">
                                    <div>
                                        <label className="mb-2 block text-sm font-medium text-slate-700">
                                            利用する AI
                                        </label>
                                        <select
                                            value={draft.provider}
                                            onChange={(event) =>
                                                setProvider(
                                                    event.target.value as 'anthropic' | 'gemini' | 'openai' | 'ollama',
                                                )
                                            }
                                            className={inputClass}
                                        >
                                            <option value="anthropic">Anthropic (Claude)</option>
                                            <option value="gemini">Google Gemini</option>
                                            <option value="openai">OpenAI</option>
                                            <option value="ollama">Ollama</option>
                                        </select>
                                    </div>

                                    <div>
                                        <label className="mb-2 block text-sm font-medium text-slate-700">
                                            モデル選択
                                        </label>
                                        <input
                                            list={apiModelSuggestions.length > 0 ? apiModelSuggestionsId : undefined}
                                            type="text"
                                            value={draft.providerModels[draft.provider]}
                                            onChange={(event) => setProviderModel(draft.provider, event.target.value)}
                                            placeholder={getDefaultModelForProvider(draft.provider)}
                                            className={inputClass}
                                        />
                                        {apiModelSuggestions.length > 0 && (
                                            <datalist id={apiModelSuggestionsId}>
                                                {apiModelSuggestions.map((model) => (
                                                    <option key={model} value={model} />
                                                ))}
                                            </datalist>
                                        )}
                                    </div>
                                </div>

                                <div className="grid gap-3 lg:grid-cols-2">
                                    <div className="rounded-xl border border-slate-200 bg-white px-4 py-3">
                                        <div className="text-xs font-semibold uppercase tracking-[0.14em] text-slate-400">
                                            Current Default
                                        </div>
                                        <div className="mt-1 text-sm font-semibold text-slate-900">
                                            {defaultAiProviderLabel}
                                        </div>
                                        <div className="mt-1 text-xs text-slate-500">
                                            {draft.providerModels[draft.provider]}
                                        </div>
                                    </div>

                                    <div className="rounded-xl border border-slate-200 bg-white px-4 py-3">
                                        <div className="text-xs font-semibold uppercase tracking-[0.14em] text-slate-400">
                                            Available Providers
                                        </div>
                                        <div className="mt-1 text-sm font-semibold text-slate-900">
                                            {configuredAiProviderCount} 件
                                        </div>
                                        <div className="mt-1 text-xs text-slate-500">
                                            接続確認やモデル候補取得は「AIプロバイダー設定」で行います。
                                        </div>
                                    </div>
                                </div>
                            </div>
                        ) : (
                            <div className="mt-4 space-y-4">
                                <div className="grid gap-4 sm:grid-cols-2">
                                    <div>
                                        <label className="mb-2 block text-sm font-medium text-slate-700">
                                            CLI種別
                                        </label>
                                        <select
                                            value={draft.poAssistantCliType}
                                            onChange={(event) =>
                                                setPoAssistantCliType(
                                                    event.target.value as 'claude' | 'gemini' | 'codex',
                                                )
                                            }
                                            className={inputClass}
                                        >
                                            <option value="claude">Claude Code CLI</option>
                                            <option value="gemini">Gemini CLI</option>
                                            <option value="codex">Codex CLI</option>
                                        </select>
                                    </div>

                                    <div>
                                        <label className="mb-2 block text-sm font-medium text-slate-700">
                                            モデル選択
                                        </label>
                                        <input
                                            type="text"
                                            value={draft.poAssistantCliModel}
                                            onChange={(event) => setPoAssistantCliModel(event.target.value)}
                                            placeholder={getDefaultPoAssistantCliModel(draft.poAssistantCliType)}
                                            className={inputClass}
                                        />
                                    </div>
                                </div>

                                <div className="rounded-xl border border-slate-200 bg-white px-4 py-3">
                                    <div className="text-xs font-semibold uppercase tracking-[0.14em] text-slate-400">
                                        CLI Status
                                    </div>
                                    <div className="mt-1 text-sm font-semibold text-slate-900">
                                        {poAssistantCliInstalled ? '検出済み' : '未検出'}
                                    </div>
                                    <div
                                        className={`mt-2 text-sm leading-6 ${poAssistantCliInstalled ? 'text-emerald-700' : 'text-amber-700'
                                            }`}
                                    >
                                        {poAssistantCliInstalled
                                            ? '現在の CLI は検出済みです。次回リクエストからこの CLI / モデル設定が使われます。'
                                            : '現在の CLI は未検出です。保存はできますが、実行前にセットアップ状況を確認してください。'}
                                    </div>
                                </div>
                            </div>
                        )}
                    </div>
                </div>

                <PoAssistantImagePanel
                    value={draft.poAssistantAvatarImage}
                    onChange={setPoAssistantAvatarImage}
                />
            </div>
        </div>
    );
}
