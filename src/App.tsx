import "./App.css";
import { useState } from "react";
import { Toaster } from "react-hot-toast";
import { AlertTriangle, Bot, History, RefreshCcw, Settings } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ScrumProvider } from "./context/ScrumContext";
import { WorkspaceProvider, useWorkspace } from "./context/WorkspaceContext";
import { SprintTimerProvider } from "./context/SprintTimerContext";
import { ProjectSelector } from "./components/ui/ProjectSelector";
import { ProjectSettings } from "./components/ui/ProjectSettings";
import { InceptionDeck } from "./components/project/InceptionDeck";
import { ScrumDashboard } from "./components/kanban/ScrumDashboard";
import { TeamLeaderSidebar } from "./components/ai/TeamLeaderSidebar";
import { HistoryModal } from "./components/HistoryModal";
import { SprintTimer } from "./components/SprintTimer";
import { TerminalDock } from "./components/terminal/TerminalDock";
import { GlobalSettingsModal } from "./components/ui/GlobalSettingsModal";

type AppView = "kanban" | "inception";

interface AppHeaderProps {
    currentView: AppView;
    isSidebarOpen: boolean;
    onOpenHistory: () => void;
    onOpenSettings: () => void;
    onSetView: (view: AppView) => void;
    onToggleSidebar: () => void;
}

function AppHeader({
    currentView,
    isSidebarOpen,
    onOpenHistory,
    onOpenSettings,
    onSetView,
    onToggleSidebar,
}: AppHeaderProps) {
    return (
        <header className="sticky top-0 z-30 shrink-0 border-b border-slate-200 bg-white/90 backdrop-blur-md shadow-[0_1px_0_rgba(15,23,42,0.04)]">
            <div className="w-full px-4 sm:px-6 lg:px-8">
                <div className="flex min-h-16 flex-wrap items-center justify-between gap-3 py-3">
                    <div className="flex min-w-0 flex-wrap items-center gap-3">
                        <div className="flex items-center gap-3">
                            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-blue-600 text-white shadow-sm">
                                <svg className="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                    <path
                                        strokeLinecap="round"
                                        strokeLinejoin="round"
                                        strokeWidth="2"
                                        d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
                                    />
                                </svg>
                            </div>
                            <div className="min-w-0">
                                <div className="text-lg font-bold tracking-tight text-slate-900">
                                    MicroScrum AI
                                </div>
                                <div className="text-xs font-medium uppercase tracking-[0.18em] text-slate-500">
                                    Pro Team Orchestrator
                                </div>
                            </div>
                        </div>

                        <div className="flex items-center rounded-xl border border-slate-200 bg-slate-100 p-1 shadow-sm">
                            <button
                                type="button"
                                onClick={() => onSetView("kanban")}
                                className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
                                    currentView === "kanban"
                                        ? "bg-white text-slate-900 shadow-sm"
                                        : "text-slate-500 hover:text-slate-800"
                                }`}
                            >
                                Kanban
                            </button>
                            <button
                                type="button"
                                onClick={() => onSetView("inception")}
                                className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
                                    currentView === "inception"
                                        ? "bg-white text-blue-700 shadow-sm"
                                        : "text-slate-500 hover:text-slate-800"
                                }`}
                            >
                                Inception Deck
                            </button>
                        </div>
                    </div>

                    <div className="flex flex-1 flex-wrap items-center justify-end gap-3">
                        <div className="flex items-center gap-2 rounded-xl border border-slate-200 bg-slate-50/80 px-2 py-1 shadow-sm">
                            <ProjectSelector />
                            <div className="hidden h-8 w-px bg-slate-200 sm:block" />
                            <ProjectSettings />
                        </div>

                        <div className="flex items-center gap-2">
                            <button
                                type="button"
                                onClick={onOpenHistory}
                                className="inline-flex h-10 items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 text-sm font-medium text-slate-600 shadow-sm transition-colors hover:bg-slate-50 hover:text-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                title="スプリント履歴を表示"
                            >
                                <History size={16} />
                                <span className="hidden sm:inline">履歴</span>
                            </button>

                            {currentView === "kanban" && (
                                <button
                                    type="button"
                                    onClick={onToggleSidebar}
                                    className={`inline-flex h-10 items-center gap-2 rounded-lg border px-3 text-sm font-medium shadow-sm transition-all ${
                                        isSidebarOpen
                                            ? "border-indigo-300 bg-indigo-100 text-indigo-800"
                                            : "border-indigo-200 bg-indigo-50 text-indigo-700 hover:bg-indigo-100"
                                    }`}
                                    title={isSidebarOpen ? "AI Team Leaderを閉じる" : "AI Team Leaderを開く"}
                                >
                                    <Bot size={16} />
                                    <span className="hidden sm:inline">AI Leader</span>
                                </button>
                            )}

                            <button
                                type="button"
                                onClick={onOpenSettings}
                                className="inline-flex h-10 w-10 items-center justify-center rounded-lg border border-slate-200 bg-white text-slate-500 shadow-sm transition-colors hover:bg-slate-50 hover:text-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
                                title="グローバル設定"
                            >
                                <Settings size={18} />
                            </button>
                        </div>
                    </div>
                </div>
            </div>

            {currentView === "kanban" && <SprintTimer />}
        </header>
    );
}

function AppContent() {
    const { gitStatus, refreshGitStatus } = useWorkspace();
    const [isHistoryOpen, setIsHistoryOpen] = useState(false);
    const [currentView, setCurrentView] = useState<AppView>("kanban");
    const [isSidebarOpen, setIsSidebarOpen] = useState(false);
    const [isTerminalMinimized, setIsTerminalMinimized] = useState(true);
    const [isSettingsOpen, setIsSettingsOpen] = useState(false);

    if (gitStatus.checked && !gitStatus.installed) {
        return (
            <div className="flex h-screen items-center justify-center bg-slate-100 px-6 py-10">
                <div className="w-full max-w-2xl rounded-3xl border border-red-200 bg-white p-8 shadow-[0_20px_80px_-30px_rgba(15,23,42,0.35)]">
                    <div className="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-red-100 text-red-600">
                        <AlertTriangle size={28} />
                    </div>
                    <h1 className="text-2xl font-bold tracking-tight text-slate-900">
                        MicroScrum AI の利用には Git のインストールが必要です
                    </h1>
                    <p className="mt-3 text-sm leading-6 text-slate-600">
                        Epic 31 以降の MicroScrum AI は、Git Worktree を前提として AI 開発環境を隔離します。
                        この PC に Git が見つからないため、安全のため処理を中断しています。
                    </p>
                    {gitStatus.message && (
                        <div className="mt-5 rounded-2xl border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700">
                            {gitStatus.message}
                        </div>
                    )}
                    <div className="mt-6 rounded-2xl border border-slate-200 bg-slate-50 px-4 py-3 text-sm text-slate-600">
                        Git をインストール後に「再チェック」を押してください。
                    </div>
                    <div className="mt-6 flex flex-wrap gap-3">
                        <button
                            type="button"
                            onClick={() => void openUrl("https://git-scm.com/")}
                            className="inline-flex items-center justify-center rounded-xl bg-blue-600 px-4 py-2.5 text-sm font-semibold text-white shadow-sm transition-colors hover:bg-blue-700"
                        >
                            Git 公式サイトを開く
                        </button>
                        <button
                            type="button"
                            onClick={() => void refreshGitStatus()}
                            className="inline-flex items-center justify-center rounded-xl border border-slate-200 bg-white px-4 py-2.5 text-sm font-semibold text-slate-700 shadow-sm transition-colors hover:bg-slate-50"
                        >
                            <RefreshCcw size={15} className="mr-2" />
                            再チェック
                        </button>
                    </div>
                </div>
            </div>
        );
    }

    return (
        <div className="flex h-screen flex-col overflow-hidden bg-slate-100 font-sans">
            <AppHeader
                currentView={currentView}
                isSidebarOpen={isSidebarOpen}
                onOpenHistory={() => setIsHistoryOpen(true)}
                onOpenSettings={() => setIsSettingsOpen(true)}
                onSetView={setCurrentView}
                onToggleSidebar={() => setIsSidebarOpen((prev) => !prev)}
            />

            {currentView === "inception" ? (
                <div className="min-h-0 flex-1 overflow-hidden">
                    <InceptionDeck />
                </div>
            ) : (
                <div className="relative flex min-h-0 flex-1 flex-row overflow-hidden bg-gray-50/50 backdrop-blur-sm">
                    <div
                        className={`relative z-10 flex min-h-0 flex-col overflow-hidden border-r border-gray-200/60 transition-all duration-500 ease-[cubic-bezier(0.2,0.8,0.2,1)] ${
                            isSidebarOpen ? "w-[70%]" : "w-full"
                        }`}
                    >
                        <div className="flex min-h-0 flex-1 flex-col overflow-hidden bg-transparent">
                            <ScrumDashboard />
                        </div>

                        <div
                            className={`z-20 flex flex-col border-t border-black/60 bg-[#111318] text-gray-300 shadow-[0_-12px_40px_-15px_rgba(0,0,0,0.55)] transition-all duration-500 ease-[cubic-bezier(0.2,0.8,0.2,1)] ${
                                isTerminalMinimized ? "h-[34px]" : "h-[40%]"
                            }`}
                        >
                            <TerminalDock
                                isMinimized={isTerminalMinimized}
                                onToggleMinimize={() => setIsTerminalMinimized((prev) => !prev)}
                            />
                        </div>
                    </div>

                    <div
                        className={`relative z-20 flex min-h-0 flex-col overflow-hidden bg-white/80 backdrop-blur-md shadow-[-12px_0_40px_-15px_rgba(0,0,0,0.15)] transition-all duration-500 ease-[cubic-bezier(0.2,0.8,0.2,1)] ${
                            isSidebarOpen ? "w-[30%] min-w-[320px]" : "w-0 min-w-0"
                        }`}
                    >
                        <TeamLeaderSidebar isOpen={isSidebarOpen} onClose={() => setIsSidebarOpen(false)} />
                    </div>
                </div>
            )}

            <HistoryModal isOpen={isHistoryOpen} onClose={() => setIsHistoryOpen(false)} />
            <GlobalSettingsModal isOpen={isSettingsOpen} onClose={() => setIsSettingsOpen(false)} />
        </div>
    );
}

function App() {
    return (
        <WorkspaceProvider>
            <SprintTimerProvider>
                <ScrumProvider>
                    <Toaster position="bottom-right" />
                    <AppContent />
                </ScrumProvider>
            </SprintTimerProvider>
        </WorkspaceProvider>
    );
}

export default App;
