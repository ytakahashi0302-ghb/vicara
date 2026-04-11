/**
 * SettingsShell — EPIC45 Phase Z スタイルガイド（実装指針）
 *
 * 共通スタイルトークン:
 *  - プライマリ:   blue-600 / 選択状態 = ring-2 ring-blue-500 のみ
 *  - ベース背景:   bg-white / border-slate-200
 *  - 角丸:         rounded-xl に統一（2xl や 3xl は使わない）
 *  - パディング:   カード p-5 / 内部 p-4 / セクション間 space-y-6
 *  - タイポ階層:   タイトル text-lg font-semibold text-slate-900
 *                 説明     text-sm text-slate-500
 *  - セマンティック色: 赤=危険 / 琥珀=注意 / 翠=成功 のみ
 *  - 情報表示にカラーバッジ / プロバイダー色分け(sky/violet/orange/cyan) / グラデーション禁止
 *  - アイコンはモノクロで識別する
 */
import { Menu } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { AnalyticsTab } from '../AnalyticsTab';
import { Button } from '../Button';
import { SetupStatusTab } from '../SetupStatusTab';
import { TeamSettingsTab } from '../TeamSettingsTab';
import { WarningBanner } from '../WarningBanner';
import { cn } from '../Modal';
import { SettingsSection } from './SettingsSection';
import { SettingsSidebar, type SettingsSidebarCategory } from './SettingsSidebar';
import { type SettingsSectionId, useSettings } from './SettingsContext';
import { AiProviderSection } from './sections/AiProviderSection';
import { PoAssistantSection } from './sections/PoAssistantSection';
import { ProjectSection } from './sections/ProjectSection';

interface SettingsShellProps {
    onClose: () => void;
    mode?: 'modal' | 'page';
    closeLabel?: string;
}

const SETTINGS_CATEGORIES: SettingsSidebarCategory[] = [
    {
        label: '開始準備',
        sections: [
            {
                id: 'project',
                label: 'プロジェクト',
                description: 'ローカルパスと基本前提',
            },
            {
                id: 'setup',
                label: 'セットアップ状況',
                description: 'Git / CLI / API キーの確認',
            },
        ],
    },
    {
        label: 'AI運用',
        sections: [
            {
                id: 'ai-provider',
                label: 'AIプロバイダー設定',
                description: 'APIキーと接続情報を管理',
            },
            {
                id: 'po-assistant',
                label: 'POアシスタント',
                description: '実行方式と見た目',
            },
            {
                id: 'team',
                label: 'チーム設定',
                description: 'ロールと並列実行構成',
            },
        ],
    },
    {
        label: '観測',
        sections: [
            {
                id: 'analytics',
                label: 'アナリティクス',
                description: 'LLM 使用量と概算コスト',
            },
        ],
    },
];

export function SettingsShell({
    onClose,
    mode = 'modal',
    closeLabel = 'キャンセル',
}: SettingsShellProps) {
    const settings = useSettings();
    const [activeSection, setActiveSection] = useState<SettingsSectionId>('project');
    const [hasInitializedSection, setHasInitializedSection] = useState(false);
    const [isMobileSidebarOpen, setIsMobileSidebarOpen] = useState(false);

    const resolvedActiveSection = !hasInitializedSection && settings.isInitialSectionReady
        ? settings.recommendedInitialSection
        : activeSection;

    useEffect(() => {
        if (mode !== 'modal') {
            return;
        }

        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                event.preventDefault();
                onClose();
            }
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => {
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, [mode, onClose]);

    const activeSectionMeta = useMemo(
        () =>
            SETTINGS_CATEGORIES.flatMap((category) => category.sections).find(
                (section) => section.id === resolvedActiveSection,
            ),
        [resolvedActiveSection],
    );

    const handleSelectSection = (sectionId: SettingsSectionId) => {
        setActiveSection(sectionId);
        setHasInitializedSection(true);
        setIsMobileSidebarOpen(false);
    };

    const renderSection = () => {
        switch (resolvedActiveSection) {
            case 'project':
                return (
                    <SettingsSection
                        title="プロジェクト"
                        description="まず作業対象のローカルディレクトリを整え、このプロジェクトに紐づく前提条件を確認します。"
                    >
                        <ProjectSection />
                    </SettingsSection>
                );
            case 'setup':
                return (
                    <SettingsSection
                        title="セットアップ状況"
                        description="Git、各 CLI、API キー、Ollama の準備状態をまとめて確認し、その場で再検出できます。"
                    >
                        <SetupStatusTab
                            embedded
                            gitStatus={settings.gitStatus}
                            cliResults={settings.cliResults}
                            cliLoading={settings.isCliDetectionLoading}
                            cliError={settings.cliDetectionError}
                            apiKeyStatuses={settings.apiKeyStatuses}
                            apiLoading={settings.isLoadingApiKeyStatus}
                            apiError={settings.apiKeyStatusError}
                            ollamaStatus={settings.ollamaStatus}
                            ollamaLoading={settings.isLoadingOllamaStatus}
                            ollamaError={settings.ollamaStatusError}
                            isRefreshing={settings.isRefreshingSetupStatus}
                            onRefresh={() => void settings.refreshSetupStatus()}
                        />
                    </SettingsSection>
                );
            case 'ai-selection':
            case 'po-assistant':
                if (resolvedActiveSection === 'po-assistant') {
                    return (
                        <SettingsSection
                            title="POアシスタント"
                            description="AI 利用先の準備が済んだ後に、POアシスタント固有の見た目や実行方式を調整します。"
                        >
                            <PoAssistantSection />
                        </SettingsSection>
                    );
                }
                return (
                    <SettingsSection
                        title="AIプロバイダー設定"
                        description="各 AI プロバイダーの API キーや接続情報をまとめて登録します。"
                    >
                        <AiProviderSection />
                    </SettingsSection>
                );
            case 'ai-provider':
                return (
                    <SettingsSection
                        title="AIプロバイダー設定"
                        description="日常的に使う既定の AI プロバイダーを選び、そのまま API キーや接続情報もまとめて整えます。"
                    >
                        <AiProviderSection />
                    </SettingsSection>
                );
            case 'team':
                return (
                    <SettingsSection
                        title="チーム設定"
                        description="マルチ CLI エージェントチームのロール定義、モデル参照、最大並列数を調整します。"
                    >
                        <TeamSettingsTab
                            embedded
                            config={settings.draft.teamConfig}
                            validationMessages={settings.teamValidationMessages}
                            isLoading={settings.isLoadingTeamConfig}
                            anthropicModelsList={settings.modelOptions.anthropic}
                            geminiModelsList={settings.modelOptions.gemini}
                            cliResults={settings.cliResults}
                            installedCliMap={settings.installedCliMap}
                            isCliDetectionLoading={settings.isCliDetectionLoading}
                            isFetchingAnthropicModels={
                                settings.isFetchingModels && settings.fetchingModelsProvider === 'anthropic'
                            }
                            isFetchingGeminiModels={
                                settings.isFetchingModels && settings.fetchingModelsProvider === 'gemini'
                            }
                            canFetchAnthropicModels={Boolean(settings.draft.apiKeys.anthropic.trim())}
                            canFetchGeminiModels={Boolean(settings.draft.apiKeys.gemini.trim())}
                            onChange={settings.setTeamConfig}
                            onFetchAnthropicModels={() => void settings.fetchModels('anthropic')}
                            onFetchGeminiModels={() => void settings.fetchModels('gemini')}
                        />
                    </SettingsSection>
                );
            case 'analytics':
                return (
                    <SettingsSection
                        title="アナリティクス"
                        description="プロジェクト全体とアクティブスプリントにおける LLM 利用量、概算コストを確認します。"
                    >
                        <AnalyticsTab embedded projectId={settings.currentProjectId} />
                    </SettingsSection>
                );
        }
    };

    return (
        <div className={cn(
            'flex min-h-0 flex-1 flex-col bg-slate-50/80',
            mode === 'modal' ? '-m-4 h-[76vh] min-h-[620px]' : 'h-full',
        )}>
            <div className="border-b border-slate-200 bg-white px-4 py-3 md:hidden">
                <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0">
                        <div className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">
                            Current Section
                        </div>
                        <div className="truncate text-sm font-semibold text-slate-900">
                            {activeSectionMeta?.label ?? '設定'}
                        </div>
                    </div>
                    <Button
                        type="button"
                        variant="secondary"
                        onClick={() => setIsMobileSidebarOpen((prev) => !prev)}
                        className="border border-slate-200 bg-white text-slate-700 hover:bg-slate-50"
                    >
                        <Menu size={16} className="mr-2" />
                        セクション
                    </Button>
                </div>
            </div>

            <div className="relative min-h-0 flex-1 overflow-hidden">
                <div className="flex h-full min-h-0">
                    <div className="hidden w-[180px] shrink-0 md:block">
                        <SettingsSidebar
                            categories={SETTINGS_CATEGORIES}
                            activeSection={resolvedActiveSection}
                            onSelect={handleSelectSection}
                        />
                    </div>

                    <div className="relative min-w-0 flex-1">
                        {isMobileSidebarOpen && (
                            <div className="absolute inset-0 z-20 md:hidden">
                                <button
                                    type="button"
                                    aria-label="サイドバーを閉じる"
                                    className="absolute inset-0 bg-slate-950/20"
                                    onClick={() => setIsMobileSidebarOpen(false)}
                                />
                                <div className="absolute inset-y-0 left-0 w-[280px] max-w-[85vw] shadow-xl">
                                    <SettingsSidebar
                                        categories={SETTINGS_CATEGORIES}
                                        activeSection={resolvedActiveSection}
                                        onSelect={handleSelectSection}
                                        className="h-full"
                                    />
                                </div>
                            </div>
                        )}

                        <div className="flex h-full min-h-0 flex-col">
                            <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4 md:px-6 md:py-6">
                                {renderSection()}
                            </div>

                            <div className="border-t border-slate-200 bg-white/95 px-4 py-4 md:px-6">
                                {resolvedActiveSection === 'team' && settings.teamWarningMessages.length > 0 && (
                                    <div className="mb-4">
                                        <WarningBanner
                                            message="未導入の CLI を使うロールがあります。"
                                            details="設定の保存は可能ですが、実行前にセットアップ状況を完了してください。"
                                        >
                                            <div className="text-xs leading-5 text-amber-900">
                                                {settings.teamWarningMessages.join(' / ')}
                                            </div>
                                        </WarningBanner>
                                    </div>
                                )}

                                <div className="flex flex-col gap-3 sm:flex-row sm:justify-end">
                                    <Button
                                        type="button"
                                        variant="ghost"
                                        onClick={onClose}
                                        className="border border-slate-200 bg-white text-slate-700 hover:bg-slate-50"
                                    >
                                        {closeLabel}
                                    </Button>
                                    <Button
                                        type="button"
                                        onClick={() => void settings.saveSettings()}
                                        disabled={settings.isSaveDisabled}
                                        className={cn(settings.isSaveDisabled && 'opacity-60')}
                                    >
                                        {settings.isSaving ? '保存中...' : '設定を保存'}
                                    </Button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
}
