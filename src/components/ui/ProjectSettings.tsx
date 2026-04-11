import { useState } from 'react';
import { useWorkspace } from '../../context/WorkspaceContext';
import { open } from '@tauri-apps/plugin-dialog';
import { Folder, FolderOpen, AlertCircle } from 'lucide-react';
import toast from 'react-hot-toast';

/**
 * ProjectSettings (EPIC45 Phase Z)
 *
 * ヘッダー右のプロジェクトパス表示/切替ボタン。
 * Scaffold ボタンは常設から撤去され、Inception Deck Phase5 内に集約された。
 */
export function ProjectSettings() {
    const { projects, currentProjectId, updateProjectPath } = useWorkspace();
    const currentProject = projects.find(p => p.id === currentProjectId);
    const [isSelecting, setIsSelecting] = useState(false);

    if (!currentProject) return null;

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
        <button
            onClick={handleSelectFolder}
            disabled={isSelecting}
            className={`inline-flex h-10 items-center gap-2 rounded-xl border px-3 text-sm font-medium shadow-sm transition-colors ${
                currentProject.local_path
                    ? 'border-slate-200 bg-white text-slate-700 hover:bg-slate-50'
                    : 'border-amber-200 bg-amber-50 text-amber-700 hover:bg-amber-100'
            }`}
            title="ローカルディレクトリ設定"
        >
            {currentProject.local_path ? (
                <>
                    <FolderOpen size={16} className="text-slate-500" />
                    <span className="max-w-[140px] truncate" title={currentProject.local_path}>
                        {currentProject.local_path.split(/[\\/]/).pop()}
                    </span>
                </>
            ) : (
                <>
                    <Folder size={16} />
                    <span>フォルダ未設定</span>
                    <AlertCircle size={14} className="text-amber-500" />
                </>
            )}
        </button>
    );
}
