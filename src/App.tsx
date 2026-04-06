import "./App.css";
import { ScrumProvider } from "./context/ScrumContext";
import { WorkspaceProvider } from "./context/WorkspaceContext";
import { SprintTimerProvider } from "./context/SprintTimerContext";
import { ProjectSelector } from "./components/ui/ProjectSelector";
import { ProjectSettings } from "./components/ui/ProjectSettings";
import { InceptionDeck } from "./components/project/InceptionDeck";
import { ScrumDashboard } from "./components/kanban/ScrumDashboard";
import { TeamLeaderSidebar } from "./components/ai/TeamLeaderSidebar";
import { Toaster } from 'react-hot-toast';
import { useState } from 'react';
import { History, Bot, Settings } from 'lucide-react';
import { HistoryModal } from './components/HistoryModal';
import { SprintTimer } from "./components/SprintTimer";
import { TerminalDock } from './components/terminal/TerminalDock';
import { GlobalSettingsModal } from './components/ui/GlobalSettingsModal';
import "./App.css";

function AppContent() {
  const [isHistoryOpen, setIsHistoryOpen] = useState(false);
  const [currentView, setCurrentView] = useState<'kanban' | 'inception'>('kanban');
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [isTerminalMinimized, setIsTerminalMinimized] = useState(true);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

  if (currentView === 'inception') {
      return (
          <div className="min-h-screen bg-gray-100 font-sans flex flex-col">
              <header className="bg-white border-b border-gray-200 sticky top-0 z-20">
                <div className="w-full mx-auto px-4 sm:px-6 lg:px-8">
                  <div className="flex justify-between h-16 items-center">
                    <div className="flex items-center">
                      <span className="text-lg font-bold text-blue-600 tracking-tight flex items-center gap-2">
                        MicroScrum AI / Inception Deck
                      </span>
                    </div>
                    <div className="flex items-center gap-4">
                      <button
                        onClick={() => setCurrentView('kanban')}
                        className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-gray-600 bg-white border border-gray-200 rounded-md hover:bg-gray-50 transition-colors"
                      >
                        カンバンに戻る
                      </button>
                    </div>
                  </div>
                </div>
              </header>
              <InceptionDeck />
          </div>
      );
  }

  return (
    <div className="h-screen bg-gray-100 font-sans flex flex-col overflow-hidden">
      <header className="bg-white border-b border-gray-200 sticky top-0 z-20 shrink-0">
        <div className="w-full mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex justify-between h-16 items-center">
            <div className="flex items-center">
              <span className="text-lg font-bold text-blue-600 tracking-tight flex items-center gap-2">
                <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
                </svg>
                MicroScrum AI
              </span>
              <ProjectSelector />
              <ProjectSettings />
              <button
                onClick={() => setIsSettingsOpen(true)}
                className="ml-2 p-1.5 text-gray-500 hover:text-gray-700 hover:bg-gray-100 rounded-md transition-colors"
                title="Global Settings"
              >
                <Settings size={20} />
              </button>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setCurrentView('inception')}
                className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-blue-600 bg-blue-50 border border-blue-200 rounded-md hover:bg-blue-100 transition-colors"
                title="AI Inception Deckを起動"
              >
                Inception Deck
              </button>
              <button
                onClick={() => setIsHistoryOpen(true)}
                className="flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-gray-600 bg-white border border-gray-200 rounded-md hover:bg-gray-50 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 transition-colors"
                title="View Sprint History"
              >
                <History size={16} />
                <span className="hidden sm:inline">履歴</span>
              </button>
              <button
                onClick={() => setIsSidebarOpen(prev => !prev)}
                className={`flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md transition-all duration-200 ${
                  isSidebarOpen
                    ? 'text-indigo-700 bg-indigo-100 border border-indigo-300 shadow-sm'
                    : 'text-indigo-600 bg-indigo-50 border border-indigo-200 hover:bg-indigo-100 hover:shadow-sm'
                }`}
                title={isSidebarOpen ? 'AI Team Leaderを閉じる' : 'AI Team Leaderを開く'}
              >
                <Bot size={16} />
                <span className="hidden sm:inline">AI Leader</span>
              </button>
            </div>
          </div>
        </div>
        <SprintTimer />
      </header>

      {/* Main content area: 3-pane layout */}
      <div className="flex-1 flex flex-row overflow-hidden bg-gray-50/50 backdrop-blur-sm relative">
        {/* Left Pane (70% or 100%) */}
        <div className={`flex flex-col overflow-hidden border-r border-gray-200/60 transition-all duration-500 ease-[cubic-bezier(0.2,0.8,0.2,1)] relative z-10 ${isSidebarOpen ? 'w-[70%]' : 'w-full'}`}>
          {/* Left-Top (flex-1) - Kanban */}
          <div className="flex-1 overflow-hidden bg-transparent flex flex-col">
            <ScrumDashboard />
          </div>
          
          {/* Left-Bottom (40% or min-height) - Terminal Dock */}
          <div className={`bg-[#18181b] text-gray-300 flex flex-col border-t border-gray-800 shadow-[0_-12px_40px_-15px_rgba(0,0,0,0.5)] z-20 transition-all duration-500 ease-[cubic-bezier(0.2,0.8,0.2,1)] ${isTerminalMinimized ? 'h-[36px]' : 'h-[40%]'}`}>
            <div className="px-3 py-1.5 h-[36px] text-xs font-mono font-medium border-b border-black/60 bg-[#1e1e24] text-gray-400 flex items-center justify-between shrink-0 select-none group cursor-pointer" onClick={() => setIsTerminalMinimized(!isTerminalMinimized)}>
              <span className="flex items-center gap-2 group-hover:text-gray-300 transition-colors duration-300">
                <svg className="w-4 h-4 text-gray-500 group-hover:text-blue-400 transition-colors duration-300 shadow-sm" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M8 9l3 3-3 3m5 0h3M4 15V9a2 2 0 012-2h12a2 2 0 012 2v6a2 2 0 01-2 2H6a2 2 0 01-2-2z"></path></svg>
                Dev Agent Terminal
              </span>
              <button 
                onClick={(e) => { e.stopPropagation(); setIsTerminalMinimized(!isTerminalMinimized); }}
                className="hover:text-white p-1 rounded-md hover:bg-white/10 transition-all duration-200 active:scale-95 flex items-center justify-center w-6 h-6"
                title={isTerminalMinimized ? 'ターミナルを展開' : 'ターミナルを最小化'}
              >
                <div className={`transform transition-transform duration-500 ease-[cubic-bezier(0.2,0.8,0.2,1)] ${isTerminalMinimized ? '-rotate-180' : 'rotate-0'}`}>
                  ▼
                </div>
              </button>
            </div>
            <div className={`flex-1 p-2 overflow-hidden relative bg-[#1e1e1e] animate-in fade-in duration-500 ${isTerminalMinimized ? 'hidden' : 'block'}`}>
              <TerminalDock />
            </div>
          </div>
        </div>

        {/* Right Pane (30% or 0%) */}
        <div className={`flex flex-col overflow-hidden bg-white/80 backdrop-blur-md transition-all duration-500 ease-[cubic-bezier(0.2,0.8,0.2,1)] shadow-[-12px_0_40px_-15px_rgba(0,0,0,0.15)] relative z-20 ${isSidebarOpen ? 'w-[30%] min-w-[320px]' : 'w-0 min-w-0'}`}>
          <TeamLeaderSidebar
            isOpen={isSidebarOpen}
            onClose={() => setIsSidebarOpen(false)}
          />
        </div>
      </div>

      <HistoryModal
        isOpen={isHistoryOpen}
        onClose={() => setIsHistoryOpen(false)}
      />
      
      <GlobalSettingsModal 
        isOpen={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
      />
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
