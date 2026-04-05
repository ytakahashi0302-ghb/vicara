import { useState, useEffect } from 'react';
import { Modal } from './Modal';
import { Settings, Trash2, Shield, RefreshCw, AlertTriangle } from 'lucide-react';
import { Button } from './Button';
import { useWorkspace } from '../../context/WorkspaceContext';
import { invoke } from '@tauri-apps/api/core';
import { load } from '@tauri-apps/plugin-store';
import toast from 'react-hot-toast';

interface GlobalSettingsModalProps {
    isOpen: boolean;
    onClose: () => void;
}

export function GlobalSettingsModal({ isOpen, onClose }: GlobalSettingsModalProps) {
    const { currentProjectId, deleteProject } = useWorkspace();
    
    // Tabs state
    const [activeTab, setActiveTab] = useState<'general' | 'ai'>('ai');
    
    // AI Settings State
    const [provider, setProvider] = useState<'anthropic' | 'gemini'>('anthropic');
    const [anthropicKey, setAnthropicKey] = useState('');
    const [geminiKey, setGeminiKey] = useState('');
    const [anthropicModel, setAnthropicModel] = useState('');
    const [geminiModel, setGeminiModel] = useState('');
    
    // Custom model toggles
    const [isCustomAnthropic, setIsCustomAnthropic] = useState(false);
    const [isCustomGemini, setIsCustomGemini] = useState(false);
    
    // Models list from API
    const [anthropicModelsList, setAnthropicModelsList] = useState<string[]>([]);
    const [geminiModelsList, setGeminiModelsList] = useState<string[]>([]);
    const [isFetchingModels, setIsFetchingModels] = useState(false);

    // Initial load
    useEffect(() => {
        if (isOpen) {
            loadStore();
        }
    }, [isOpen]);

    const loadStore = async () => {
        try {
            const store = await load('settings.json');
            
            const p = await store.get<{ value: string }>('default-ai-provider');
            if (p && p.value === 'gemini') setProvider('gemini');
            else setProvider('anthropic');
            
            const ak = await store.get<{ value: string }>('anthropic-api-key');
            if (ak && ak.value) setAnthropicKey(ak.value);
            else if (typeof ak === 'string') setAnthropicKey(ak);

            const gk = await store.get<{ value: string }>('gemini-api-key');
            if (gk && gk.value) setGeminiKey(gk.value);
            else if (typeof gk === 'string') setGeminiKey(gk);

            const am = await store.get<{ value: string }>('anthropic-model');
            if (am && am.value) setAnthropicModel(am.value);
            else if (typeof am === 'string') setAnthropicModel(am);
            else setAnthropicModel('claude-3-5-sonnet-latest'); // default

            const gm = await store.get<{ value: string }>('gemini-model');
            if (gm && gm.value) setGeminiModel(gm.value);
            else if (typeof gm === 'string') setGeminiModel(gm);
            else setGeminiModel('gemini-2.5-flash'); // default
            
        } catch (e) {
            console.error('Failed to load settings', e);
        }
    };

    const fetchModels = async (targetProvider: 'anthropic' | 'gemini') => {
        setIsFetchingModels(true);
        try {
            const models = await invoke<string[]>('get_available_models', { provider: targetProvider });
            if (targetProvider === 'anthropic') {
                setAnthropicModelsList(models);
                if (models.length > 0 && !isCustomAnthropic && !models.includes(anthropicModel)) {
                    setAnthropicModel(models[0]);
                }
            } else {
                setGeminiModelsList(models);
                if (models.length > 0 && !isCustomGemini && !models.includes(geminiModel)) {
                    setGeminiModel(models[0]);
                }
            }
            toast.success(`${targetProvider === 'anthropic' ? 'Anthropic' : 'Gemini'} のモデル一覧を取得しました`);
        } catch (e) {
            console.error(`Failed to fetch ${targetProvider} models`, e);
            toast.error(`モデルの取得に失敗しました。APIキーを確認してください。\nError: ${e}`);
        } finally {
            setIsFetchingModels(false);
        }
    };

    const handleSave = async () => {
        try {
            const store = await load('settings.json');
            await store.set('default-ai-provider', { value: provider });
            await store.set('anthropic-api-key', { value: anthropicKey });
            await store.set('gemini-api-key', { value: geminiKey });
            await store.set('anthropic-model', { value: anthropicModel });
            await store.set('gemini-model', { value: geminiModel });
            await store.save();
            toast.success('設定を保存しました');
            onClose();
        } catch (e) {
            console.error('Failed to save settings', e);
            toast.error('設定の保存に失敗しました');
        }
    };

    const handleDeleteProject = async () => {
        if (!currentProjectId || currentProjectId === 'default') {
            toast.error('このプロジェクトは削除できません');
            return;
        }
        
        if (confirm('本当にこのプロジェクトを削除しますか？紐づくすべてのバックログやスプリントデータが消去されます。')) {
            try {
                await deleteProject(currentProjectId);
                onClose();
            } catch (e) {
                // Error toast is handled in WorkspaceContext
            }
        }
    };

    return (
        <Modal
            isOpen={isOpen}
            onClose={onClose}
            width="xl"
            title={
                <div className="flex items-center gap-2 text-gray-900">
                    <Settings size={20} className="text-gray-500" />
                    <span>グローバル設定</span>
                </div>
            }
        >
            <div className="flex border-b border-gray-200 mb-4">
                <button
                    onClick={() => setActiveTab('ai')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'ai' 
                            ? 'border-b-2 border-blue-500 text-blue-600' 
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    AI設定
                </button>
                <button
                    onClick={() => setActiveTab('general')}
                    className={`pb-2 px-4 text-sm font-medium transition-colors ${
                        activeTab === 'general' 
                            ? 'border-b-2 border-blue-500 text-blue-600' 
                            : 'text-gray-500 hover:text-gray-700'
                    }`}
                >
                    プロジェクト設定
                </button>
            </div>

            <div className="min-h-[350px] max-h-[60vh] overflow-y-auto px-1 custom-scrollbar">
                {activeTab === 'ai' && (
                    <div className="space-y-6">
                        {/* Provider Selection */}
                        <div>
                            <label className="block text-sm font-medium text-gray-700 mb-2">
                                デフォルト AI プロバイダー
                            </label>
                            <div className="grid grid-cols-2 gap-3">
                                <label className={`border rounded-lg p-3 cursor-pointer flex items-center transition-all ${
                                    provider === 'anthropic' ? 'border-blue-500 bg-blue-50 ring-1 ring-blue-500' : 'border-gray-200 hover:bg-gray-50'
                                }`}>
                                    <input 
                                        type="radio" 
                                        name="provider" 
                                        value="anthropic" 
                                        checked={provider === 'anthropic'} 
                                        onChange={() => setProvider('anthropic')}
                                        className="hidden"
                                    />
                                    <span className="font-semibold text-gray-800 ml-2">Anthropic (Claude)</span>
                                </label>
                                <label className={`border rounded-lg p-3 cursor-pointer flex items-center transition-all ${
                                    provider === 'gemini' ? 'border-purple-500 bg-purple-50 ring-1 ring-purple-500' : 'border-gray-200 hover:bg-gray-50'
                                }`}>
                                    <input 
                                        type="radio" 
                                        name="provider" 
                                        value="gemini" 
                                        checked={provider === 'gemini'} 
                                        onChange={() => setProvider('gemini')}
                                        className="hidden"
                                    />
                                    <span className="font-semibold text-gray-800 ml-2">Google Gemini</span>
                                </label>
                            </div>
                        </div>

                        {/* Anthropic Settings */}
                        <div className={`p-4 rounded-lg border ${provider === 'anthropic' ? 'border-blue-200 bg-blue-50/30' : 'border-gray-200 opacity-60'}`}>
                            <h3 className="font-medium text-gray-900 flex items-center gap-2 mb-4">
                                <Shield size={16} className="text-gray-500" />
                                Anthropic 設定
                            </h3>
                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm text-gray-600 mb-1">API Key</label>
                                    <input
                                        type="password"
                                        placeholder="sk-ant-api03-..."
                                        value={anthropicKey}
                                        onChange={(e) => setAnthropicKey(e.target.value)}
                                        className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <div className="flex justify-between items-center mb-1">
                                        <label className="block text-sm text-gray-600">Model</label>
                                        <button 
                                            onClick={() => fetchModels('anthropic')}
                                            disabled={isFetchingModels || !anthropicKey}
                                            className="text-xs text-blue-600 hover:text-blue-800 flex items-center gap-1 disabled:opacity-50"
                                        >
                                            <RefreshCw size={12} className={isFetchingModels && provider === 'anthropic' ? "animate-spin" : ""} />
                                            モデル一覧を取得
                                        </button>
                                    </div>
                                    
                                    {!isCustomAnthropic && anthropicModelsList.length > 0 ? (
                                        <select 
                                            value={anthropicModel}
                                            onChange={(e) => setAnthropicModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none mb-2 bg-white"
                                        >
                                            {anthropicModelsList.map(m => <option key={m} value={m}>{m}</option>)}
                                        </select>
                                    ) : (
                                        <input
                                            type="text"
                                            placeholder="claude-3-5-sonnet-latest"
                                            value={anthropicModel}
                                            onChange={(e) => setAnthropicModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500 focus:outline-none mb-2"
                                        />
                                    )}
                                    <label className="flex items-center gap-2 text-xs text-gray-500 cursor-pointer">
                                        <input type="checkbox" checked={isCustomAnthropic} onChange={(e) => setIsCustomAnthropic(e.target.checked)} className="rounded border-gray-300 text-blue-600 focus:ring-blue-500" />
                                        カスタムモデル名を手動で入力する
                                    </label>
                                </div>
                            </div>
                        </div>

                        {/* Gemini Settings */}
                        <div className={`p-4 rounded-lg border ${provider === 'gemini' ? 'border-purple-200 bg-purple-50/30' : 'border-gray-200 opacity-60'}`}>
                            <h3 className="font-medium text-gray-900 flex items-center gap-2 mb-4">
                                <Shield size={16} className="text-gray-500" />
                                Gemini 設定
                            </h3>
                            <div className="space-y-4">
                                <div>
                                    <label className="block text-sm text-gray-600 mb-1">API Key</label>
                                    <input
                                        type="password"
                                        placeholder="AIzaSy..."
                                        value={geminiKey}
                                        onChange={(e) => setGeminiKey(e.target.value)}
                                        className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-purple-500 focus:outline-none"
                                    />
                                </div>
                                <div>
                                    <div className="flex justify-between items-center mb-1">
                                        <label className="block text-sm text-gray-600">Model</label>
                                        <button 
                                            onClick={() => fetchModels('gemini')}
                                            disabled={isFetchingModels || !geminiKey}
                                            className="text-xs text-purple-600 hover:text-purple-800 flex items-center gap-1 disabled:opacity-50"
                                        >
                                            <RefreshCw size={12} className={isFetchingModels && provider === 'gemini' ? "animate-spin" : ""} />
                                            モデル一覧を取得
                                        </button>
                                    </div>

                                    {!isCustomGemini && geminiModelsList.length > 0 ? (
                                        <select 
                                            value={geminiModel}
                                            onChange={(e) => setGeminiModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-purple-500 focus:outline-none mb-2 bg-white"
                                        >
                                            {geminiModelsList.map(m => <option key={m} value={m}>{m}</option>)}
                                        </select>
                                    ) : (
                                        <input
                                            type="text"
                                            placeholder="gemini-2.5-flash"
                                            value={geminiModel}
                                            onChange={(e) => setGeminiModel(e.target.value)}
                                            className="w-full border border-gray-300 rounded-md px-3 py-2 text-sm focus:ring-2 focus:ring-purple-500 focus:outline-none mb-2"
                                        />
                                    )}
                                    <label className="flex items-center gap-2 text-xs text-gray-500 cursor-pointer">
                                        <input type="checkbox" checked={isCustomGemini} onChange={(e) => setIsCustomGemini(e.target.checked)} className="rounded border-gray-300 text-purple-600 focus:ring-purple-500" />
                                        カスタムモデル名を手動で入力する
                                    </label>
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {activeTab === 'general' && (
                    <div className="space-y-6">
                        <div className="p-4 rounded-lg border border-red-200 bg-red-50">
                            <h3 className="font-medium text-red-800 flex items-center gap-2 mb-2">
                                <AlertTriangle size={18} />
                                Danger Zone
                            </h3>
                            <p className="text-sm text-red-600 mb-4">
                                現在開いているプロジェクトを完全に削除します。この操作は取り消せません。バックログ、スプリント履歴、タスクのデータがすべて失われます。
                            </p>
                            <Button 
                                onClick={handleDeleteProject}
                                variant="secondary" 
                                className="bg-white text-red-600 border-red-200 hover:bg-red-50"
                                disabled={!currentProjectId || currentProjectId === 'default'}
                            >
                                <Trash2 size={16} className="mr-2" />
                                このプロジェクトを削除
                            </Button>
                        </div>
                    </div>
                )}
            </div>

            <div className="mt-6 flex justify-end gap-3 pt-4 border-t border-gray-100">
                <button
                    onClick={onClose}
                    className="px-4 py-2 border border-gray-300 rounded-md text-sm font-medium text-gray-700 bg-white hover:bg-gray-50 focus:outline-none transition-colors"
                >
                    キャンセル
                </button>
                <button
                    onClick={handleSave}
                    className="px-4 py-2 bg-blue-600 text-white rounded-md text-sm font-medium hover:bg-blue-700 focus:outline-none transition-colors"
                >
                    設定を保存
                </button>
            </div>
        </Modal>
    );
}
