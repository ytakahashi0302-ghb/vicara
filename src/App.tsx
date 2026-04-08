import "./App.css";
import { useEffect, useRef, useState } from "react";
import { Toaster } from "react-hot-toast";
import {
    AlertTriangle,
    Coins,
    History,
    RefreshCcw,
    Settings,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ScrumProvider } from "./context/ScrumContext";
import { WorkspaceProvider, useWorkspace } from "./context/WorkspaceContext";
import { SprintTimerProvider } from "./context/SprintTimerContext";
import { useLlmUsageSummary } from "./hooks/useLlmUsageSummary";
import { usePoAssistantAvatarImage } from "./hooks/usePoAssistantAvatarImage";
import { ProjectSelector } from "./components/ui/ProjectSelector";
import { ProjectSettings } from "./components/ui/ProjectSettings";
import { InceptionDeck } from "./components/project/InceptionDeck";
import { ScrumDashboard } from "./components/kanban/ScrumDashboard";
import { Avatar } from "./components/ai/Avatar";
import { PoAssistantSidebar } from "./components/ai/PoAssistantSidebar";
import { HistoryModal } from "./components/HistoryModal";
import { SprintTimer } from "./components/SprintTimer";
import { TerminalDock } from "./components/terminal/TerminalDock";
import { GlobalSettingsModal } from "./components/ui/GlobalSettingsModal";

type AppView = "kanban" | "inception";
type ResizeHandle = "sidebar" | "terminal" | null;

const SIDEBAR_RATIO_STORAGE_KEY = "vicara.layout.sidebarRatio";
const TERMINAL_RATIO_STORAGE_KEY = "vicara.layout.terminalRatio";
const DEFAULT_SIDEBAR_RATIO = 0.7;
const DEFAULT_TERMINAL_RATIO = 0.38;
const MIN_MAIN_PANE_WIDTH = 420;
const MIN_SIDEBAR_WIDTH = 320;
const MIN_DASHBOARD_HEIGHT = 260;
const MIN_TERMINAL_HEIGHT = 180;
const SPLITTER_SIZE_PX = 10;

interface AppHeaderProps {
    currentProjectId: string;
    currentView: AppView;
    isSidebarOpen: boolean;
    onOpenHistory: () => void;
    onOpenSettings: () => void;
    onSetView: (view: AppView) => void;
    onToggleSidebar: () => void;
    poAssistantAvatarImage: string | null;
}

function formatTokenCount(value: number) {
    return new Intl.NumberFormat("ja-JP").format(value);
}

function formatEstimatedCost(value: number) {
    return `~$${value.toFixed(value >= 100 ? 0 : value >= 10 ? 1 : value >= 1 ? 2 : 3)}`;
}

function clamp(value: number, min: number, max: number) {
    return Math.min(max, Math.max(min, value));
}

function readStoredRatio(key: string, fallback: number) {
    if (typeof window === "undefined") {
        return fallback;
    }

    const stored = Number.parseFloat(window.localStorage.getItem(key) ?? "");
    return Number.isFinite(stored) ? stored : fallback;
}

function LlmUsagePill({ projectId }: { projectId: string }) {
    const { summary, loading, error } = useLlmUsageSummary(projectId);

    if (!projectId) {
        return null;
    }

    const title = error
        ? `LLM usage の取得に失敗しました: ${error}`
        : summary
          ? [
                `今日の消費: ${formatTokenCount(summary.today_totals.total_tokens)} token / ${formatEstimatedCost(summary.today_totals.estimated_cost_usd)}`,
                `未計測イベント: ${summary.project_totals.unavailable_event_count}件`,
            ].join("\n")
          : "LLM usage を読み込み中です";

    return (
        <div
            className="flex min-w-[280px] items-center gap-3 rounded-xl border border-emerald-200 bg-emerald-50/80 px-3 py-2 shadow-sm"
            title={title}
        >
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-emerald-100 text-emerald-700">
                <Coins size={16} />
            </div>
            <div className="grid min-w-0 flex-1 grid-cols-2 gap-3">
                <div className="min-w-0">
                    <div className="text-[11px] font-semibold uppercase tracking-[0.16em] text-emerald-700">
                        Project
                    </div>
                    <div className="truncate text-sm font-semibold text-slate-900">
                        {loading && !summary
                            ? "読み込み中..."
                            : summary
                              ? `${formatTokenCount(summary.project_totals.total_tokens)} token / ${formatEstimatedCost(summary.project_totals.estimated_cost_usd)}`
                              : "0 token / ~$0.000"}
                    </div>
                </div>
                <div className="min-w-0">
                    <div className="text-[11px] font-semibold uppercase tracking-[0.16em] text-emerald-700">
                        Sprint
                    </div>
                    <div className="truncate text-sm font-semibold text-slate-900">
                        {loading && !summary
                            ? "読み込み中..."
                            : summary
                              ? `${formatTokenCount(summary.active_sprint_totals.total_tokens)} token / ${formatEstimatedCost(summary.active_sprint_totals.estimated_cost_usd)}`
                              : "0 token / ~$0.000"}
                    </div>
                </div>
            </div>
        </div>
    );
}

function AppHeader({
    currentProjectId,
    currentView,
    isSidebarOpen,
    onOpenHistory,
    onOpenSettings,
    onSetView,
    onToggleSidebar,
    poAssistantAvatarImage,
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
                                <div className="app-brand-wordmark text-lg text-slate-900">
                                    vicara
                                </div>
                                <div className="text-xs font-medium uppercase tracking-[0.18em] text-slate-500">
                                    人間中心のAIチーム開発
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
                        <LlmUsagePill projectId={currentProjectId} />

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
                                    title={isSidebarOpen ? "POアシスタントを閉じる" : "POアシスタントを開く"}
                                >
                                    <Avatar kind="po-assistant" size="xs" imageSrc={poAssistantAvatarImage} />
                                    <span className="hidden sm:inline">POアシスタント</span>
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
    const { currentProjectId, gitStatus, refreshGitStatus } = useWorkspace();
    const poAssistantAvatarImage = usePoAssistantAvatarImage();
    const [isHistoryOpen, setIsHistoryOpen] = useState(false);
    const [currentView, setCurrentView] = useState<AppView>("kanban");
    const [isSidebarOpen, setIsSidebarOpen] = useState(false);
    const [isTerminalMinimized, setIsTerminalMinimized] = useState(true);
    const [isSettingsOpen, setIsSettingsOpen] = useState(false);
    const [sidebarRatio, setSidebarRatio] = useState(() =>
        readStoredRatio(SIDEBAR_RATIO_STORAGE_KEY, DEFAULT_SIDEBAR_RATIO),
    );
    const [terminalRatio, setTerminalRatio] = useState(() =>
        readStoredRatio(TERMINAL_RATIO_STORAGE_KEY, DEFAULT_TERMINAL_RATIO),
    );
    const [activeResizeHandle, setActiveResizeHandle] = useState<ResizeHandle>(null);
    const kanbanContainerRef = useRef<HTMLDivElement | null>(null);
    const mainPaneRef = useRef<HTMLDivElement | null>(null);

    useEffect(() => {
        window.localStorage.setItem(SIDEBAR_RATIO_STORAGE_KEY, sidebarRatio.toString());
    }, [sidebarRatio]);

    useEffect(() => {
        window.localStorage.setItem(TERMINAL_RATIO_STORAGE_KEY, terminalRatio.toString());
    }, [terminalRatio]);

    useEffect(() => {
        if (!isSidebarOpen && activeResizeHandle === "sidebar") {
            setActiveResizeHandle(null);
        }
        if (isTerminalMinimized && activeResizeHandle === "terminal") {
            setActiveResizeHandle(null);
        }
    }, [activeResizeHandle, isSidebarOpen, isTerminalMinimized]);

    useEffect(() => {
        if (currentView !== "kanban" && activeResizeHandle !== null) {
            setActiveResizeHandle(null);
        }
    }, [activeResizeHandle, currentView]);

    useEffect(() => {
        if (activeResizeHandle === null) {
            document.body.style.removeProperty("cursor");
            document.body.style.removeProperty("user-select");
            return;
        }

        document.body.style.cursor =
            activeResizeHandle === "sidebar" ? "col-resize" : "row-resize";
        document.body.style.userSelect = "none";

        const handlePointerMove = (event: PointerEvent) => {
            if (activeResizeHandle === "sidebar" && kanbanContainerRef.current && isSidebarOpen) {
                const rect = kanbanContainerRef.current.getBoundingClientRect();
                const maxLeft = rect.width - MIN_SIDEBAR_WIDTH;
                const minRatio = MIN_MAIN_PANE_WIDTH / rect.width;
                const maxRatio = maxLeft / rect.width;

                if (rect.width <= MIN_MAIN_PANE_WIDTH + MIN_SIDEBAR_WIDTH || maxRatio <= minRatio) {
                    return;
                }

                const rawRatio = (event.clientX - rect.left) / rect.width;
                setSidebarRatio(clamp(rawRatio, minRatio, maxRatio));
                return;
            }

            if (
                activeResizeHandle === "terminal" &&
                mainPaneRef.current &&
                !isTerminalMinimized
            ) {
                const rect = mainPaneRef.current.getBoundingClientRect();
                const availableHeight = rect.height - SPLITTER_SIZE_PX;
                const maxTerminalHeight = availableHeight - MIN_DASHBOARD_HEIGHT;

                if (availableHeight <= MIN_DASHBOARD_HEIGHT + MIN_TERMINAL_HEIGHT) {
                    return;
                }

                const terminalHeight = rect.bottom - event.clientY;
                const minRatio = MIN_TERMINAL_HEIGHT / availableHeight;
                const maxRatio = maxTerminalHeight / availableHeight;
                const rawRatio = terminalHeight / availableHeight;
                setTerminalRatio(clamp(rawRatio, minRatio, maxRatio));
            }
        };

        const handlePointerUp = () => {
            setActiveResizeHandle(null);
        };

        window.addEventListener("pointermove", handlePointerMove);
        window.addEventListener("pointerup", handlePointerUp);

        return () => {
            window.removeEventListener("pointermove", handlePointerMove);
            window.removeEventListener("pointerup", handlePointerUp);
            document.body.style.removeProperty("cursor");
            document.body.style.removeProperty("user-select");
        };
    }, [activeResizeHandle, isSidebarOpen, isTerminalMinimized]);

    const isDraggingSidebar = activeResizeHandle === "sidebar";
    const isDraggingTerminal = activeResizeHandle === "terminal";
    const mainPaneWidthStyle = isSidebarOpen
        ? { width: `calc(${(sidebarRatio * 100).toFixed(4)}% - ${SPLITTER_SIZE_PX / 2}px)` }
        : { width: "100%" };
    const sidebarWidthStyle = isSidebarOpen
        ? { width: `calc(${((1 - sidebarRatio) * 100).toFixed(4)}% - ${SPLITTER_SIZE_PX / 2}px)` }
        : { width: "0px" };
    const dashboardHeightStyle = isTerminalMinimized
        ? undefined
        : {
              height: `calc((100% - ${SPLITTER_SIZE_PX}px) * ${(1 - terminalRatio).toFixed(4)})`,
          };
    const terminalHeightStyle = isTerminalMinimized
        ? { height: "34px" }
        : {
              height: `calc((100% - ${SPLITTER_SIZE_PX}px) * ${terminalRatio.toFixed(4)})`,
          };

    if (gitStatus.checked && !gitStatus.installed) {
        return (
            <div className="flex h-screen items-center justify-center bg-slate-100 px-6 py-10">
                <div className="w-full max-w-2xl rounded-3xl border border-red-200 bg-white p-8 shadow-[0_20px_80px_-30px_rgba(15,23,42,0.35)]">
                    <div className="mb-5 flex h-14 w-14 items-center justify-center rounded-2xl bg-red-100 text-red-600">
                        <AlertTriangle size={28} />
                    </div>
                    <h1 className="text-2xl font-bold tracking-tight text-slate-900">
                        vicara の利用には Git のインストールが必要です
                    </h1>
                    <p className="mt-3 text-sm leading-6 text-slate-600">
                        vicara は Git Worktree を前提として AI 開発環境を隔離します。
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
                currentProjectId={currentProjectId}
                currentView={currentView}
                isSidebarOpen={isSidebarOpen}
                onOpenHistory={() => setIsHistoryOpen(true)}
                onOpenSettings={() => setIsSettingsOpen(true)}
                onSetView={setCurrentView}
                onToggleSidebar={() => setIsSidebarOpen((prev) => !prev)}
                poAssistantAvatarImage={poAssistantAvatarImage}
            />

            {currentView === "inception" ? (
                <div className="min-h-0 flex-1 overflow-hidden">
                    <InceptionDeck />
                </div>
            ) : (
                <div
                    ref={kanbanContainerRef}
                    className="relative flex min-h-0 flex-1 flex-row overflow-hidden bg-gray-50/50 backdrop-blur-sm"
                >
                    <div
                        ref={mainPaneRef}
                        style={mainPaneWidthStyle}
                        className={`relative z-10 flex min-h-0 flex-col overflow-hidden ${
                            isSidebarOpen ? "border-r border-gray-200/60" : ""
                        } ${
                            isDraggingSidebar || isDraggingTerminal
                                ? "transition-none"
                                : "transition-[width] duration-300 ease-[cubic-bezier(0.2,0.8,0.2,1)]"
                        }`}
                    >
                        <div
                            style={dashboardHeightStyle}
                            className={`min-h-0 overflow-hidden bg-transparent ${
                                isTerminalMinimized ? "flex-1" : ""
                            }`}
                        >
                            <ScrumDashboard />
                        </div>

                        {!isTerminalMinimized && (
                            <div
                                role="separator"
                                aria-orientation="horizontal"
                                aria-label="terminal height resize handle"
                                className="app-splitter app-splitter-y"
                                onPointerDown={(event) => {
                                    event.preventDefault();
                                    setActiveResizeHandle("terminal");
                                }}
                            />
                        )}

                        <div
                            style={terminalHeightStyle}
                            className={`z-20 flex flex-col border-t border-black/60 bg-[#111318] text-gray-300 shadow-[0_-12px_40px_-15px_rgba(0,0,0,0.55)] ${
                                isDraggingTerminal
                                    ? "transition-none"
                                    : "transition-[height] duration-300 ease-[cubic-bezier(0.2,0.8,0.2,1)]"
                            }`}
                        >
                            <TerminalDock
                                isMinimized={isTerminalMinimized}
                                onToggleMinimize={() => setIsTerminalMinimized((prev) => !prev)}
                            />
                        </div>
                    </div>

                    {isSidebarOpen && (
                        <div
                            role="separator"
                            aria-orientation="vertical"
                            aria-label="sidebar width resize handle"
                            className="app-splitter app-splitter-x"
                            onPointerDown={(event) => {
                                event.preventDefault();
                                setActiveResizeHandle("sidebar");
                            }}
                        />
                    )}

                    <div
                        style={sidebarWidthStyle}
                        className={`relative z-20 flex min-h-0 flex-col overflow-hidden bg-white/80 backdrop-blur-md shadow-[-12px_0_40px_-15px_rgba(0,0,0,0.15)] ${
                            isDraggingSidebar
                                ? "transition-none"
                                : "transition-[width] duration-300 ease-[cubic-bezier(0.2,0.8,0.2,1)]"
                        } ${
                            isSidebarOpen ? "min-w-[320px]" : "min-w-0"
                        }`}
                    >
                        <PoAssistantSidebar isOpen={isSidebarOpen} onClose={() => setIsSidebarOpen(false)} />
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

