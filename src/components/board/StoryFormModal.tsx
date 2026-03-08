import React, { useState, useEffect } from 'react';
import { Modal } from '../ui/Modal';
import { Input } from '../ui/Input';
import { Textarea } from '../ui/Textarea';
import { Button } from '../ui/Button';

export interface StoryFormData {
    title: string;
    description: string;
    acceptance_criteria: string;
}

interface StoryFormModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSave: (data: StoryFormData) => Promise<void>;
    onDelete?: () => Promise<void>;
    initialData?: Partial<StoryFormData>;
    title: string;
}

export const StoryFormModal: React.FC<StoryFormModalProps> = ({
    isOpen,
    onClose,
    onSave,
    onDelete,
    initialData,
    title
}) => {
    const [formData, setFormData] = useState<StoryFormData>({
        title: '',
        description: '',
        acceptance_criteria: ''
    });
    const [errors, setErrors] = useState<Partial<Record<keyof StoryFormData, string>>>({});
    const [isSubmitting, setIsSubmitting] = useState(false);

    useEffect(() => {
        if (isOpen) {
            setFormData({
                title: initialData?.title || '',
                description: initialData?.description || '',
                acceptance_criteria: initialData?.acceptance_criteria || ''
            });
            setErrors({});
        }
    }, [isOpen, initialData]);

    const validate = () => {
        const newErrors: Partial<Record<keyof StoryFormData, string>> = {};
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
            console.error('Failed to save story:', error);
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
                    placeholder="As a user, I want to..."
                    autoFocus
                />

                <Textarea
                    label="Description"
                    value={formData.description}
                    onChange={(e) => setFormData(p => ({ ...p, description: e.target.value }))}
                    placeholder="Detailed description of the story..."
                    rows={4}
                />

                <Textarea
                    label="Acceptance Criteria"
                    value={formData.acceptance_criteria}
                    onChange={(e) => setFormData(p => ({ ...p, acceptance_criteria: e.target.value }))}
                    placeholder="- The user can...&#10;- The system should..."
                    rows={4}
                />

                <div className="flex justify-between items-center mt-4 pt-4 border-t">
                    <div>
                        {onDelete && (
                            <Button
                                type="button"
                                variant="danger"
                                onClick={async () => {
                                    if (window.confirm("Are you sure you want to delete this story?")) {
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
