import { useState } from 'react';
import { useWorkspace } from '../../context/WorkspaceContext';
import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { Folder, FolderOpen, AlertCircle, Hammer } from 'lucide-react';
import toast from 'react-hot-toast';

interface ProjectSettingsProps {
    onOpenScaffolding?: () => void;
}

export function ProjectSettings({ onOpenScaffolding }: ProjectSettingsProps) {
    const { projects, currentProjectId, updateProjectPath } = useWorkspace();
    const currentProject = projects.find(p => p.id === currentProjectId);
    const [isSelecting, setIsSelecting] = useState(false);

    if (!currentProject) return null;

    const handleScaffolding = async () => {
        if (!currentProject.local_path) {
            toast.error('先にローカルディレクトリを設定してください');
            return;
        }
        if (onOpenScaffolding) {
            onOpenScaffolding();
        } else {
            // フォールバック: AGENT.md + .claude/settings.json のみ生成
            try {
                await Promise.all([
                    invoke<string>('generate_agent_md', {
                        localPath: currentProject.local_path,
                        projectName: currentProject.name,
                    }),
                    invoke<void>('generate_claude_settings', {
                        localPath: currentProject.local_path,
                    }),
                ]);
                toast.success('AGENT.md と .claude/settings.json を生成しました');
            } catch (error) {
                toast.error(`生成失敗: ${error}`);
            }
        }
    };

    const handleSelectFolder = async () => {
        setIsSelecting(true);
        try {
            const selectedPath = await open({
                directory: true,
                multiple: false,
                title: 'プロジェクトのディレクトリを選択してください'
            });

            if (selectedPath && typeof selectedPath === 'string') {
                const result = await updateProjectPath(currentProjectId, selectedPath);
                if (result.success) {
                    toast.success('ワークスペースのディレクトリを設定しました');
                    if (result.has_product_context || result.has_architecture || result.has_rule) {
                        toast('既存のInception Deckファイルが見つかりました', { icon: 'ℹ️' });
                    }
                }
            }
        } catch (error) {
            console.error('Failed to select directory:', error);
            toast.error('ディレクトリの選択に失敗しました');
        } finally {
            setIsSelecting(false);
        }
    };

    return (
        <div className="flex items-center ml-4 gap-1">
            <button
                onClick={handleSelectFolder}
                disabled={isSelecting}
                className={`flex items-center gap-2 px-3 py-1.5 text-sm font-medium rounded-md transition-colors ${
                    currentProject.local_path
                        ? 'text-green-700 bg-green-50 border border-green-200 hover:bg-green-100'
                        : 'text-amber-700 bg-amber-50 border border-amber-200 hover:bg-amber-100'
                }`}
                title="ローカルディレクトリ設定"
            >
                {currentProject.local_path ? (
                    <>
                        <FolderOpen size={16} />
                        <span className="hidden sm:inline max-w-[120px] truncate" title={currentProject.local_path}>
                            {currentProject.local_path.split(/[\\/]/).pop()}
                        </span>
                    </>
                ) : (
                    <>
                        <Folder size={16} />
                        <span className="hidden sm:inline">フォルダ未設定</span>
                        <AlertCircle size={14} className="text-amber-500" />
                    </>
                )}
            </button>
            {currentProject.local_path && (
                <button
                    onClick={handleScaffolding}
                    className="flex items-center gap-1 px-2 py-1.5 text-sm font-medium text-indigo-700 bg-indigo-50 border border-indigo-200 rounded-md hover:bg-indigo-100 transition-colors"
                    title="Scaffolding: AGENT.md と .claude/settings.json を生成"
                >
                    <Hammer size={14} />
                </button>
            )}
        </div>
    );
}
