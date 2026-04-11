import { Coins } from 'lucide-react';
import { useLlmUsageSummary } from '../../hooks/useLlmUsageSummary';

interface AnalyticsTabProps {
    embedded?: boolean;
    projectId: string;
}

function formatTokenCount(value: number) {
    return new Intl.NumberFormat('ja-JP').format(value);
}

function formatEstimatedCost(value: number) {
    return `~$${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : value >= 1 ? 2 : 3)}`;
}

function shouldDisplayBreakdownItem(value: { estimated_cost_usd: number; total_tokens: number }) {
    return value.estimated_cost_usd > 0 || value.total_tokens > 0;
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

function formatProviderLabel(provider: string, model: string) {
    const normalizedProvider = provider.trim().toLowerCase();
    const normalizedModel = model.trim().toLowerCase();

    if (normalizedProvider === 'anthropic') {
        return 'ANTHROPIC';
    }

    if (normalizedProvider === 'gemini') {
        return 'GEMINI';
    }

    if (normalizedProvider === 'openai') {
        return 'OPENAI';
    }

    if (normalizedProvider === 'ollama') {
        return 'OLLAMA';
    }

    if (
        normalizedProvider === 'gemini_cli' ||
        (normalizedProvider === 'claude_cli' && normalizedModel.includes('gemini'))
    ) {
        return 'GEMINI_CLI';
    }

    if (
        normalizedProvider === 'codex_cli' ||
        (normalizedProvider === 'claude_cli' &&
            (normalizedModel === 'o3' || normalizedModel.startsWith('o4') || normalizedModel.startsWith('gpt-')))
    ) {
        return 'CODEX_CLI';
    }

    if (normalizedProvider === 'claude_cli') {
        return 'CLAUDE_CLI';
    }

    return provider.toUpperCase();
}

export function AnalyticsTab({ embedded = false, projectId }: AnalyticsTabProps) {
    const {
        summary: usageSummary,
        loading: usageLoading,
        error: usageError,
    } = useLlmUsageSummary(projectId);
    const visibleSourceBreakdown = usageSummary?.by_source.filter(shouldDisplayBreakdownItem) ?? [];
    const visibleModelBreakdown = usageSummary?.by_model.filter(shouldDisplayBreakdownItem) ?? [];

    return (
        <div className="space-y-6">
            <div className="rounded-xl border border-slate-200 bg-white p-5">
                <div className="mb-4 flex items-start justify-between gap-3">
                    <div>
                        {!embedded && (
                            <>
                                <h3 className="flex items-center gap-2 text-lg font-semibold text-slate-900">
                                    <Coins size={16} className="text-slate-500" />
                                    LLM Observability
                                </h3>
                                <p className="mt-1 text-sm text-slate-500">
                                    プロジェクト全体とアクティブスプリント内での LLM 使用量を確認できます。
                                </p>
                            </>
                        )}
                        {embedded && (
                            <div className="text-sm text-slate-500">
                                プロジェクト全体とアクティブスプリント内での LLM 使用量を確認できます。
                            </div>
                        )}
                    </div>
                    <div className="rounded-full border border-slate-200 bg-slate-50 px-3 py-1 text-xs font-semibold text-slate-600">
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
                                CLI 実行の一部は厳密 token 未計測です。現在は
                                <code className="mx-1 rounded bg-amber-100 px-1 py-0.5 text-xs">measurement_status='unavailable'</code>
                                として保存しています。
                            </div>
                        )}

                        <div className="grid gap-4 lg:grid-cols-2">
                            <div className="rounded-lg border border-white/70 bg-white p-4 shadow-sm">
                                <div className="mb-3 text-sm font-semibold text-slate-900">Source別内訳</div>
                                <div className="space-y-2">
                                    {visibleSourceBreakdown.length ? visibleSourceBreakdown.map((item) => (
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
                                    {visibleModelBreakdown.length ? visibleModelBreakdown.map((item) => (
                                        <div key={`${item.provider}:${item.model}`} className="flex items-center justify-between rounded-md bg-slate-50 px-3 py-2">
                                            <div className="min-w-0">
                                                <div className="truncate text-sm font-medium text-slate-800">
                                                    {item.model}
                                                </div>
                                                <div className="text-xs uppercase tracking-[0.14em] text-slate-500">
                                                    {formatProviderLabel(item.provider, item.model)}
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
        </div>
    );
}
