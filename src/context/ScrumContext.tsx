import { createContext, useContext, useEffect, ReactNode } from 'react';
import { useStories } from '../hooks/useStories';
import { useTasks } from '../hooks/useTasks';
import { Story, Task } from '../types';

interface ScrumContextType {
    stories: Story[];
    tasks: Task[];
    loading: boolean;
    addStory: (story: Omit<Story, 'created_at' | 'updated_at'>) => Promise<void>;
    updateStory: (story: Story) => Promise<void>;
    deleteStory: (id: string) => Promise<void>;
    addTask: (task: Omit<Task, 'created_at' | 'updated_at'>) => Promise<void>;
    updateTaskStatus: (taskId: string, status: Task['status']) => Promise<void>;
    updateTask: (task: Task) => Promise<void>;
    deleteTask: (id: string) => Promise<void>;
    refresh: () => Promise<void>;
}

const ScrumContext = createContext<ScrumContextType | undefined>(undefined);

export function ScrumProvider({ children }: { children: ReactNode }) {
    const {
        stories,
        loading: storiesLoading,
        fetchStories,
        addStory,
        updateStory,
        deleteStory
    } = useStories();

    const {
        tasks,
        loading: tasksLoading,
        fetchTasks,
        addTask,
        updateTaskStatus,
        updateTask,
        deleteTask
    } = useTasks();

    useEffect(() => {
        fetchStories();
        fetchTasks();
    }, [fetchStories, fetchTasks]);

    const refresh = async () => {
        await Promise.all([fetchStories(), fetchTasks()]);
    };

    const value = {
        stories,
        tasks,
        loading: storiesLoading || tasksLoading,
        addStory,
        updateStory,
        deleteStory,
        addTask,
        updateTaskStatus,
        updateTask,
        deleteTask,
        refresh
    };

    return <ScrumContext.Provider value={value}>{children}</ScrumContext.Provider>;
}

export function useScrum() {
    const context = useContext(ScrumContext);
    if (context === undefined) {
        throw new Error('useScrum must be used within a ScrumProvider');
    }
    return context;
}
