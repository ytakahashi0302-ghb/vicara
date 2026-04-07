import React, { useState, useEffect } from 'react';
import { Modal } from '../ui/Modal';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';
import { invoke } from '@tauri-apps/api/core';

import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import toast from 'react-hot-toast';
import { Task, TeamConfiguration, TeamRoleSetting } from '../../types';

export interface TaskFormData {
    title: string;
    description: string;
    status: 'TODO' | 'IN_PROGRESS' | 'REVIEW' | 'DONE';
    priority: number;
    assigned_role_id: string;
    blocked_by_task_ids: string[];
}

interface TaskFormModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSave: (data: TaskFormData) => Promise<void>;
    onDelete?: () => Promise<void>;
    initialData?: Partial<TaskFormData>;
    title: string;
    availableTasks?: Task[];
}

export const TaskFormModal: React.FC<TaskFormModalProps> = ({
    isOpen,
    onClose,
    onSave,
    onDelete,
    initialData,
    title,
    availableTasks = []
}) => {
    const [availableRoles, setAvailableRoles] = useState<TeamRoleSetting[]>([]);
    const [isLoadingRoles, setIsLoadingRoles] = useState(false);
    const [formData, setFormData] = useState<TaskFormData>({
        title: '',
        description: '',
        status: 'TODO',
        priority: 3,
        assigned_role_id: '',
        blocked_by_task_ids: []
    });
    const [errors, setErrors] = useState<Partial<Record<keyof TaskFormData, string>>>({});
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [mode, setMode] = useState<'edit' | 'preview'>('edit');

    useEffect(() => {
        if (isOpen) {
            setFormData({
                title: initialData?.title || '',
                description: initialData?.description || '',
                status: initialData?.status || 'TODO',
                priority: initialData?.priority ?? 3,
                assigned_role_id: initialData?.assigned_role_id || '',
                blocked_by_task_ids: initialData?.blocked_by_task_ids || []
            });
            setErrors({});
            setMode(initialData?.description ? 'preview' : 'edit');
        }
    }, [isOpen, initialData]);

    useEffect(() => {
        if (!isOpen) return;

        let cancelled = false;

        const loadRoles = async () => {
            setIsLoadingRoles(true);
            try {
                const config = await invoke<TeamConfiguration>('get_team_configuration');
                if (!cancelled) {
                    setAvailableRoles(config.roles);
                }
            } catch (error) {
                console.error('Failed to load team roles', error);
                if (!cancelled) {
                    toast.error(`担当ロール一覧の取得に失敗しました: ${error}`);
                    setAvailableRoles([]);
                }
            } finally {
                if (!cancelled) {
                    setIsLoadingRoles(false);
                }
            }
        };

        loadRoles();

        return () => {
            cancelled = true;
        };
    }, [isOpen]);

    const validate = () => {
        const newErrors: Partial<Record<keyof TaskFormData, string>> = {};
        if (!formData.title.trim()) {
            newErrors.title = 'Title is required';
        }
        setErrors(newErrors);
        return Object.keys(newErrors).length === 0;
    };

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!validate()) {
            toast.error('タイトルを入力してください (Title is required)');
            return;
        }

        setIsSubmitting(true);
        try {
            await onSave(formData);
            onClose();
        } catch (error) {
            console.error('Failed to save task:', error);
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <Modal 
            isOpen={isOpen} 
            onClose={onClose} 
            width="lg" 
            title={title}
        >
            <form
                id="task-form"
                onSubmit={handleSubmit}
                onKeyDownCapture={(e) => e.stopPropagation()}
                className="flex flex-col gap-4"
            >
                <Input
                    label="タイトル"
                    value={formData.title}
                    onChange={(e) => setFormData(p => ({ ...p, title: e.target.value }))}
                    error={errors.title}
                    placeholder="何をすべきですか？"
                    autoFocus
                />

                <div className="flex flex-col gap-2">
                    <div className="flex items-center justify-between">
                        <label className="text-sm font-medium text-gray-700">詳細・詳細設定</label>
                        <div className="flex bg-gray-100 p-0.5 rounded-md">
                            <button
                                type="button"
                                onClick={() => setMode('edit')}
                                className={`px-3 py-1 text-sm rounded-sm transition-colors ${mode === 'edit' ? 'bg-white shadow-sm font-medium text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
                            >
                                編集
                            </button>
                            <button
                                type="button"
                                onClick={() => setMode('preview')}
                                className={`px-3 py-1 text-sm rounded-sm transition-colors ${mode === 'preview' ? 'bg-white shadow-sm font-medium text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
                            >
                                プレビュー
                            </button>
                        </div>
                    </div>

                    {mode === 'edit' ? (
                        <textarea
                            value={formData.description}
                            onChange={(e) => setFormData(p => ({ ...p, description: e.target.value }))}
                            placeholder="Markdownで詳細を記述できます..."
                            className="font-mono text-sm min-h-[200px] w-full p-3 rounded-md border border-gray-300 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors bg-white resize-y"
                        />
                    ) : (
                        <div className="min-h-[200px] w-full p-3 rounded-md border border-gray-300 bg-gray-50 overflow-y-auto prose prose-slate max-w-none text-sm">
                            {formData.description ? (
                                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                    {formData.description}
                                </ReactMarkdown>
                            ) : (
                                <p className="text-gray-400 italic font-sans m-0">プレビューする内容がありません</p>
                            )}
                        </div>
                    )}
                </div>

                <div className="flex gap-3">
                    <div className="flex flex-col gap-1 flex-1">
                        <label className="text-sm font-medium text-gray-700">ステータス</label>
                        <select
                            value={formData.status}
                            onChange={(e) => setFormData(p => ({ ...p, status: e.target.value as TaskFormData['status'] }))}
                            className="flex h-10 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors"
                        >
                            <option value="TODO">未着手 (To Do)</option>
                            <option value="IN_PROGRESS">進行中 (In Progress)</option>
                            <option value="REVIEW">レビュー (Review)</option>
                            <option value="DONE">完了 (Done)</option>
                        </select>
                    </div>

                    <div className="flex flex-col gap-1 flex-1">
                        <label className="text-sm font-medium text-gray-700">優先度（小さいほど高い）</label>
                        <select
                            value={formData.priority}
                            onChange={(e) => setFormData(p => ({ ...p, priority: Number(e.target.value) }))}
                            className="flex h-10 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors"
                        >
                            <option value={1}>1（最重要）</option>
                            <option value={2}>2（高）</option>
                            <option value={3}>3（中・デフォルト）</option>
                            <option value={4}>4（低）</option>
                            <option value={5}>5（最低）</option>
                        </select>
                    </div>
                </div>

                <div className="flex flex-col gap-1">
                    <label className="text-sm font-medium text-gray-700">担当ロール</label>
                    <select
                        value={formData.assigned_role_id}
                        onChange={(e) => setFormData(p => ({ ...p, assigned_role_id: e.target.value }))}
                        className="flex h-10 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors"
                        disabled={isLoadingRoles}
                    >
                        <option value="">未設定</option>
                        {availableRoles.map(role => (
                            <option key={role.id} value={role.id}>
                                {role.name} ({role.model})
                            </option>
                        ))}
                    </select>
                    <p className="text-xs text-gray-500">
                        {isLoadingRoles
                            ? '担当ロールを読み込んでいます...'
                            : 'Claude 実行時に使用するロールを選択します。'}
                    </p>
                </div>

                {availableTasks.length > 0 && (
                    <div className="flex flex-col gap-2">
                        <label className="text-sm font-medium text-gray-700">
                            依存関係（先行タスク）
                            <span className="text-xs text-gray-400 ml-1">— 完了が必要なタスクを選択</span>
                        </label>
                        <div className="border border-gray-200 rounded-md max-h-36 overflow-y-auto divide-y divide-gray-100">
                            {availableTasks.map(t => (
                                <label
                                    key={t.id}
                                    className="flex items-center gap-3 px-3 py-2 hover:bg-gray-50 cursor-pointer"
                                >
                                    <input
                                        type="checkbox"
                                        checked={formData.blocked_by_task_ids.includes(t.id)}
                                        onChange={(e) => {
                                            setFormData(p => ({
                                                ...p,
                                                blocked_by_task_ids: e.target.checked
                                                    ? [...p.blocked_by_task_ids, t.id]
                                                    : p.blocked_by_task_ids.filter(id => id !== t.id)
                                            }));
                                        }}
                                        className="h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
                                    />
                                    <span className="text-sm text-gray-700 truncate">{t.title}</span>
                                    <span className={`ml-auto text-xs px-1.5 py-0.5 rounded border shrink-0 ${
                                        t.status === 'Done' ? 'bg-green-50 text-green-600 border-green-200' :
                                        t.status === 'Review' ? 'bg-amber-50 text-amber-700 border-amber-200' :
                                        t.status === 'In Progress' ? 'bg-blue-50 text-blue-600 border-blue-200' :
                                        'bg-slate-50 text-slate-500 border-slate-200'
                                    }`}>{t.status}</span>
                                </label>
                            ))}
                        </div>
                    </div>
                )}

                <div className="flex justify-between items-center mt-4 pt-4 border-t">
                    <div>
                        {onDelete && (
                            <Button
                                type="button"
                                variant="danger"
                                onClick={async () => {
                                    if (window.confirm("このタスクを削除してもよろしいですか？")) {
                                        await onDelete();
                                        onClose();
                                    }
                                }}
                            >
                                削除
                            </Button>
                        )}
                    </div>
                    <div className="flex gap-2">
                        <Button type="button" variant="ghost" onClick={onClose} disabled={isSubmitting}>
                            キャンセル
                        </Button>
                        <Button type="submit" form="task-form" variant="primary" disabled={isSubmitting}>
                            {isSubmitting ? '保存中...' : '保存'}
                        </Button>
                    </div>
                </div>
            </form>
        </Modal>
    );
};
