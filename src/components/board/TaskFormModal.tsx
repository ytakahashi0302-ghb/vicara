import React, { useState, useEffect } from 'react';
import { Modal } from '../ui/Modal';
import { Input } from '../ui/Input';
import { Button } from '../ui/Button';

import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

export interface TaskFormData {
    title: string;
    description: string;
    status: 'TODO' | 'IN_PROGRESS' | 'DONE';
}

interface TaskFormModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSave: (data: TaskFormData) => Promise<void>;
    onDelete?: () => Promise<void>;
    initialData?: Partial<TaskFormData>;
    title: string;
}

export const TaskFormModal: React.FC<TaskFormModalProps> = ({
    isOpen,
    onClose,
    onSave,
    onDelete,
    initialData,
    title
}) => {
    const [formData, setFormData] = useState<TaskFormData>({
        title: '',
        description: '',
        status: 'TODO'
    });
    const [errors, setErrors] = useState<Partial<Record<keyof TaskFormData, string>>>({});
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [mode, setMode] = useState<'edit' | 'preview'>('edit');

    useEffect(() => {
        if (isOpen) {
            setFormData({
                title: initialData?.title || '',
                description: initialData?.description || '',
                status: initialData?.status || 'TODO'
            });
            setErrors({});
            // Set default tab based on whether description exists
            setMode(initialData?.description ? 'preview' : 'edit');
        }
    }, [isOpen, initialData]);

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
        if (!validate()) return;

        setIsSubmitting(true);
        try {
            await onSave(formData);
            onClose();
        } catch (error) {
            console.error('Failed to save task:', error);
            // In a real app, handle global error state here
        } finally {
            setIsSubmitting(false);
        }
    };

    return (
        <Modal isOpen={isOpen} onClose={onClose} title={title}>
            <form onSubmit={handleSubmit} className="flex flex-col gap-4">
                <Input
                    label="Title"
                    value={formData.title}
                    onChange={(e) => setFormData(p => ({ ...p, title: e.target.value }))}
                    error={errors.title}
                    placeholder="What needs to be done?"
                    autoFocus
                />

                <div className="flex flex-col gap-2">
                    <div className="flex items-center justify-between">
                        <label className="text-sm font-medium text-gray-700">Description</label>
                        <div className="flex bg-gray-100 p-0.5 rounded-md">
                            <button
                                type="button"
                                onClick={() => setMode('edit')}
                                className={`px-3 py-1 text-sm rounded-sm transition-colors ${mode === 'edit' ? 'bg-white shadow-sm font-medium text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
                            >
                                Edit
                            </button>
                            <button
                                type="button"
                                onClick={() => setMode('preview')}
                                className={`px-3 py-1 text-sm rounded-sm transition-colors ${mode === 'preview' ? 'bg-white shadow-sm font-medium text-gray-900' : 'text-gray-500 hover:text-gray-700'}`}
                            >
                                Preview
                            </button>
                        </div>
                    </div>

                    {mode === 'edit' ? (
                        <textarea
                            value={formData.description}
                            onChange={(e) => setFormData(p => ({ ...p, description: e.target.value }))}
                            placeholder="Add Markdown details here..."
                            className="font-mono text-sm min-h-[200px] w-full p-3 rounded-md border border-gray-300 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors bg-white resize-y"
                        />
                    ) : (
                        <div className="min-h-[200px] w-full p-3 rounded-md border border-gray-300 bg-gray-50 overflow-y-auto prose prose-slate max-w-none text-sm">
                            {formData.description ? (
                                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                                    {formData.description}
                                </ReactMarkdown>
                            ) : (
                                <p className="text-gray-400 italic font-sans m-0">Nothing to preview</p>
                            )}
                        </div>
                    )}
                </div>

                <div className="flex flex-col gap-1 w-full">
                    <label className="text-sm font-medium text-gray-700">Status</label>
                    <select
                        value={formData.status}
                        onChange={(e) => setFormData(p => ({ ...p, status: e.target.value as TaskFormData['status'] }))}
                        className="flex h-10 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors"
                    >
                        <option value="TODO">To Do</option>
                        <option value="IN_PROGRESS">In Progress</option>
                        <option value="DONE">Done</option>
                    </select>
                </div>

                <div className="flex justify-between items-center mt-4 pt-4 border-t">
                    <div>
                        {onDelete && (
                            <Button
                                type="button"
                                variant="danger"
                                onClick={async () => {
                                    if (window.confirm("Are you sure you want to delete this task?")) {
                                        await onDelete();
                                        onClose();
                                    }
                                }}
                            >
                                Delete
                            </Button>
                        )}
                    </div>
                    <div className="flex gap-2">
                        <Button type="button" variant="ghost" onClick={onClose} disabled={isSubmitting}>
                            Cancel
                        </Button>
                        <Button type="submit" variant="primary" disabled={isSubmitting}>
                            {isSubmitting ? 'Saving...' : 'Save'}
                        </Button>
                    </div>
                </div>
            </form>
        </Modal>
    );
};
