import "./App.css";
import { ScrumProvider } from "./context/ScrumContext";
import { Board } from "./components/kanban/Board";
import { Button } from "./components/ui/Button";
import { useScrum } from "./context/ScrumContext";
import { Toaster } from 'react-hot-toast';
import "./App.css";

// 開発用の初期データ投入ボタン等を含むコンポーネント（今後整理）
function DeveloperTools() {
  const { addStory, addTask, refresh } = useScrum();

  const handleCreateMockData = async () => {
    try {
      console.log('Start adding mock data...');
      const storyId = `story-${Date.now()}`;
      await addStory({
        id: storyId,
        title: "As a PO, I want a Kanban board to visualize tasks",
        description: "MVP sprint item.",
        acceptance_criteria: "- Build UI\n- Test DnD",
        status: "In Progress"
      });
      console.log('Mock story added:', storyId);

      await addTask({
        id: `task-${Date.now()}-1`,
        story_id: storyId,
        title: "Setup DnD kit",
        description: "Install and configure dnd-kit",
        status: "Done"
      });

      await addTask({
        id: `task-${Date.now()}-2`,
        story_id: storyId,
        title: "Implement Swimlanes",
        description: "Create horizontal layout for stories",
        status: "In Progress"
      });

      await addTask({
        id: `task-${Date.now()}-3`,
        story_id: storyId,
        title: "Write documentation",
        description: "Update README and architecture docs",
        status: "To Do"
      });

      console.log('Mock tasks added');
      await refresh();
      console.log('Refresh completed');
    } catch (e) {
      console.error('Error adding mock data:', e);
    }
  };

  return (
    <div className="fixed bottom-4 right-4 bg-white p-4 rounded-lg shadow-lg border border-gray-200 z-50">
      <h3 className="text-sm font-bold mb-2">Dev Tools</h3>
      <Button onClick={handleCreateMockData} size="sm" variant="secondary">
        Add Mock Data
      </Button>
    </div>
  );
}

import { SprintTimer } from "./components/SprintTimer";

function AppContent() {
  return (
    <div className="min-h-screen bg-gray-100 font-sans">
      <header className="bg-white border-b border-gray-200 sticky top-0 z-20">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex justify-between h-16 items-center">
            <div className="flex items-center">
              <span className="text-lg font-bold text-blue-600 tracking-tight flex items-center gap-2">
                <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
                </svg>
                MicroScrum AI
              </span>
            </div>
          </div>
        </div>
        <SprintTimer />
      </header>

      <main className="max-w-7xl mx-auto lg:h-[calc(100vh-120px)] overflow-hidden pt-4">
        <div className="h-full overflow-y-auto">
          <Board />
        </div>
      </main>

      <DeveloperTools />
    </div>
  );
}

function App() {
  return (
    <ScrumProvider>
      <Toaster position="bottom-right" />
      <AppContent />
    </ScrumProvider>
  );
}

export default App;
