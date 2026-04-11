import { AlertTriangle, FolderOpen, Trash2 } from 'lucide-react';
import { Button } from '../../Button';
import { SettingsField } from '../SettingsField';
import { useSettings } from '../SettingsContext';

export function ProjectSection() {
    const {
        currentProject,
        currentProjectId,
        isSelectingPath,
        selectProjectFolder,
        deleteCurrentProject,
    } = useSettings();

    return (
        <div className="space-y-6">
            <div className="rounded-xl border border-slate-200 bg-white p-5">
                <h4 className="text-lg font-semibold text-slate-900">
                    {currentProject?.name ?? 'ワークスペース未選択'}
                </h4>
                <p className="mt-1 text-sm leading-6 text-slate-500">
                    Dev エージェントが作業するローカルディレクトリと、プロジェクトの安全操作をここで管理します。
                </p>
            </div>

            <SettingsField
                label="対象ディレクトリパス"
                description="Dev エージェントが自動開発を行う際の作業ディレクトリです。ローカル環境の絶対パスを設定してください。"
            >
                <div className="flex flex-col gap-3 lg:flex-row">
                    <input
                        type="text"
                        readOnly
                        value={currentProject?.local_path || '未設定'}
                        placeholder="パスが未設定です"
                        className="h-10 flex-1 rounded-xl border border-slate-300 bg-slate-50 px-3 text-sm text-slate-700"
                    />
                    <Button
                        type="button"
                        variant="secondary"
                        onClick={() => void selectProjectFolder()}
                        disabled={isSelectingPath}
                        className="border border-slate-200 bg-white text-slate-700 hover:bg-slate-50"
                    >
                        <FolderOpen size={16} className="mr-2" />
                        {isSelectingPath ? '選択中...' : 'フォルダを選択'}
                    </Button>
                </div>
            </SettingsField>

            <div className="rounded-xl border border-rose-200 bg-rose-50 p-5">
                <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                    <div className="max-w-3xl">
                        <div className="inline-flex items-center gap-2 text-xs font-semibold text-rose-700">
                            <AlertTriangle size={14} />
                            Danger Zone
                        </div>
                        <h4 className="mt-2 text-base font-semibold text-rose-900">
                            このプロジェクトを削除
                        </h4>
                        <p className="mt-1 text-sm leading-6 text-rose-800">
                            現在開いているプロジェクトを完全に削除します。バックログ、スプリント履歴、タスクなどの関連データが失われ、この操作は取り消せません。
                        </p>
                    </div>

                    <Button
                        type="button"
                        variant="secondary"
                        onClick={() => void deleteCurrentProject()}
                        disabled={!currentProjectId || currentProjectId === 'default'}
                        className="border border-rose-200 bg-white text-rose-700 hover:bg-rose-100"
                    >
                        <Trash2 size={16} className="mr-2" />
                        プロジェクトを削除
                    </Button>
                </div>
            </div>
        </div>
    );
}
