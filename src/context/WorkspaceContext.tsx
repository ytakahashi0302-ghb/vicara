import { createContext, useContext, useEffect, useState, ReactNode, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Project } from '../types';
import toast from 'react-hot-toast';

interface WorkspaceContextType {
    projects: Project[];
    currentProjectId: string;
    setCurrentProjectId: (id: string) => void;
    fetchProjects: () => Promise<void>;
    addProject: (id: string, name: string, description: string | null) => Promise<void>;
    updateProjectPath: (id: string, localPath: string | null) => Promise<{ success: boolean; has_product_context: boolean; has_architecture: boolean; has_rule: boolean }>;
    deleteProject: (id: string) => Promise<void>;
}

const WorkspaceContext = createContext<WorkspaceContextType | undefined>(undefined);

export function WorkspaceProvider({ children }: { children: ReactNode }) {
    const [projects, setProjects] = useState<Project[]>([]);
    const [currentProjectId, setCurrentProjectIdState] = useState<string>('default');

    const fetchProjects = useCallback(async () => {
        try {
            const result = await invoke<Project[]>('get_projects');
            setProjects(result);
            
            // Fallback selection if current project is no longer valid or is the default
            setCurrentProjectIdState(prev => {
                if (!result.find(p => p.id === prev)) {
                    return result.length > 0 ? result[0].id : 'default';
                }
                if (prev === 'default' && result.length > 0) {
                    return result[0].id;
                }
                return prev;
            });
        } catch (err) {
            console.error('Failed to fetch projects', err);
            toast.error(`プロジェクトの取得に失敗しました: ${err}`);
        }
    }, []);

    useEffect(() => {
        // eslint-disable-next-line react-hooks/set-state-in-effect
        fetchProjects();
    }, [fetchProjects]);

    const addProject = useCallback(async (id: string, name: string, description: string | null) => {
        try {
            await invoke('create_project', {
                id,
                name,
                description
            });
            await fetchProjects();
            setCurrentProjectIdState(id);
            toast.success('ワークスペースを作成しました');
        } catch (err) {
            console.error('Failed to create project', err);
            toast.error(`ワークスペースの作成に失敗しました: ${err}`);
            throw err;
        }
    }, [fetchProjects]);

    const setCurrentProjectId = useCallback((id: string) => {
        setCurrentProjectIdState(id);
    }, []);

    const updateProjectPath = useCallback(async (id: string, localPath: string | null) => {
        try {
            const result = await invoke<{ success: boolean; has_product_context: boolean; has_architecture: boolean; has_rule: boolean }>('update_project_path', {
                id,
                localPath
            });
            await fetchProjects();
            return result;
        } catch (err) {
            console.error('Failed to update project path', err);
            toast.error(`パスの保存に失敗しました: ${err}`);
            throw err;
        }
    }, [fetchProjects]);

    const deleteProject = useCallback(async (id: string) => {
        try {
            await invoke('delete_project', { id });
            await fetchProjects();
            toast.success('プロジェクトを削除しました');
        } catch (err) {
            console.error('Failed to delete project', err);
            toast.error(`プロジェクトの削除に失敗しました: ${err}`);
            throw err;
        }
    }, [fetchProjects]);

    const value = {
        projects,
        currentProjectId,
        setCurrentProjectId,
        fetchProjects,
        addProject,
        updateProjectPath,
        deleteProject
    };

    return <WorkspaceContext.Provider value={value}>{children}</WorkspaceContext.Provider>;
}

export function useWorkspace() {
    const context = useContext(WorkspaceContext);
    if (context === undefined) {
        throw new Error('useWorkspace must be used within a WorkspaceProvider');
    }
    return context;
}
