import React, { useState, useEffect } from 'react';
import { load } from '@tauri-apps/plugin-store';
import { Modal } from './ui/Modal';
import { Input } from './ui/Input';
import { Button } from './ui/Button';

interface SettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
}

export const SettingsModal: React.FC<SettingsModalProps> = ({ isOpen, onClose }) => {
    const [anthropicKey, setAnthropicKey] = useState('');
    const [geminiKey, setGeminiKey] = useState('');
    const [defaultProvider, setDefaultProvider] = useState<'anthropic' | 'gemini'>('anthropic');
    const [sprintDuration, setSprintDuration] = useState<number>(1);
    const [isLoading, setIsLoading] = useState(false);
    const [isSaving, setIsSaving] = useState(false);
    const [message, setMessage] = useState<{ text: string; type: 'success' | 'error' } | null>(null);

    useEffect(() => {
        if (isOpen) {
            loadSettings();
            setMessage(null);
        }
    }, [isOpen]);

    const loadSettings = async () => {
        setIsLoading(true);
        try {
            const store = await load('settings.json');

            // Load Anthropic Key
            const savedAnthropic = await store.get<{ value: string }>('anthropic-api-key');
            if (savedAnthropic && typeof savedAnthropic === 'object' && 'value' in savedAnthropic) {
                setAnthropicKey(savedAnthropic.value);
            } else if (typeof savedAnthropic === 'string') {
                setAnthropicKey(savedAnthropic);
            } else {
                setAnthropicKey('');
            }

            // Load Gemini Key
            const savedGemini = await store.get<{ value: string }>('gemini-api-key');
            if (savedGemini && typeof savedGemini === 'object' && 'value' in savedGemini) {
                setGeminiKey(savedGemini.value);
            } else if (typeof savedGemini === 'string') {
                setGeminiKey(savedGemini);
            } else {
                setGeminiKey('');
            }

            // Load Default Provider
            const savedProvider = await store.get<{ value: string }>('default-ai-provider');
            if (savedProvider && typeof savedProvider === 'object' && 'value' in savedProvider) {
                setDefaultProvider(savedProvider.value as 'anthropic' | 'gemini');
            } else if (typeof savedProvider === 'string') {
                setDefaultProvider(savedProvider as 'anthropic' | 'gemini');
            } else {
                setDefaultProvider('anthropic');
            }

            // Load Sprint Duration
            const savedDuration = await store.get<{ value: number }>('sprint-duration-hours');
            if (savedDuration && typeof savedDuration === 'object' && 'value' in savedDuration) {
                setSprintDuration(Number(savedDuration.value));
            } else if (typeof savedDuration === 'number') {
                setSprintDuration(savedDuration);
            } else {
                setSprintDuration(1); // Default is 1 hour
            }

        } catch (error) {
            console.error('Failed to load settings:', error);
            setMessage({ text: 'Failed to load settings.', type: 'error' });
        } finally {
            setIsLoading(false);
        }
    };

    const handleSave = async () => {
        setIsSaving(true);
        setMessage(null);
        try {
            const store = await load('settings.json');
            await store.set('anthropic-api-key', { value: anthropicKey });
            await store.set('gemini-api-key', { value: geminiKey });
            await store.set('default-ai-provider', { value: defaultProvider });
            await store.set('sprint-duration-hours', { value: sprintDuration });

            await store.save();

            // Store保存後、アプリ全体に設定更新を通知
            window.dispatchEvent(new CustomEvent('settings-updated'));

            setMessage({ text: 'Settings saved successfully.', type: 'success' });
            setTimeout(() => {
                onClose();
            }, 1000);
        } catch (error) {
            console.error('Failed to save settings:', error);
            setMessage({ text: 'Failed to save settings.', type: 'error' });
        } finally {
            setIsSaving(false);
        }
    };

    return (
        <Modal isOpen={isOpen} onClose={onClose} title="Settings">
            <div className="space-y-6">

                {/* Provider Selection */}
                <div>
                    <label className="text-sm font-medium text-gray-700 block mb-2">Default AI Provider</label>
                    <div className="flex gap-4">
                        <label className="flex items-center gap-2">
                            <input
                                type="radio"
                                name="provider"
                                value="anthropic"
                                checked={defaultProvider === 'anthropic'}
                                onChange={() => setDefaultProvider('anthropic')}
                                disabled={isLoading || isSaving}
                                className="text-blue-600 focus:ring-blue-500"
                            />
                            <span className="text-sm text-gray-900">Anthropic (Claude)</span>
                        </label>
                        <label className="flex items-center gap-2">
                            <input
                                type="radio"
                                name="provider"
                                value="gemini"
                                checked={defaultProvider === 'gemini'}
                                onChange={() => setDefaultProvider('gemini')}
                                disabled={isLoading || isSaving}
                                className="text-blue-600 focus:ring-blue-500"
                            />
                            <span className="text-sm text-gray-900">Google Gemini</span>
                        </label>
                    </div>
                </div>

                <div className="border-t pt-4 space-y-4">
                    <div>
                        <label className="text-sm font-medium text-gray-700 block mb-2">Sprint Duration (Hours)</label>
                        <select
                            value={sprintDuration}
                            onChange={(e) => setSprintDuration(Number(e.target.value))}
                            disabled={isLoading || isSaving}
                            className="w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 bg-white"
                        >
                            <option value={1}>1 Hour (Default)</option>
                            <option value={2}>2 Hours</option>
                            <option value={4}>4 Hours</option>
                            <option value={8}>8 Hours (Max)</option>
                        </select>
                        <p className="text-xs text-gray-500 mt-1">
                            Current active sprints won't be affected. The new duration takes effect upon starting the next sprint.
                        </p>
                    </div>
                </div>

                <div className="border-t pt-4 space-y-4">
                    <div>
                        <Input
                            label="Anthropic API Key"
                            type="password"
                            placeholder="sk-ant-api03-..."
                            value={anthropicKey}
                            onChange={(e) => setAnthropicKey(e.target.value)}
                            disabled={isLoading || isSaving}
                        />
                    </div>
                    <div>
                        <Input
                            label="Gemini API Key"
                            type="password"
                            placeholder="AIzaSy..."
                            value={geminiKey}
                            onChange={(e) => setGeminiKey(e.target.value)}
                            disabled={isLoading || isSaving}
                        />
                    </div>
                    <p className="text-xs text-gray-500">
                        API keys are securely stored locally and used only for AI task generation.
                    </p>
                </div>

                {message && (
                    <div className={`text-sm p-3 rounded ${message.type === 'success' ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'}`}>
                        {message.text}
                    </div>
                )}

                <div className="flex justify-end gap-2 mt-6">
                    <Button variant="secondary" onClick={onClose} disabled={isSaving}>
                        Close
                    </Button>
                    <Button onClick={handleSave} disabled={isLoading || isSaving}>
                        {isSaving ? 'Saving...' : 'Save'}
                    </Button>
                </div>
            </div>
        </Modal>
    );
};
