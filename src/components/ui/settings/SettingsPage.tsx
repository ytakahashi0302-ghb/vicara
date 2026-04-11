import { useEffect } from 'react';
import { X } from 'lucide-react';
import { SettingsProvider } from './SettingsContext';
import { SettingsShell } from './SettingsShell';

interface SettingsPageProps {
    onClose: () => void;
}

/**
 * SettingsPage (EPIC45 Phase ZZ)
 *
 * 設定内容自体は維持しつつ、独立ページとして表示するラッパー。
 * ヘッダー右上の歯車から遷移し、閉じると直前のビューへ戻る。
 */
export function SettingsPage({ onClose }: SettingsPageProps) {
    useEffect(() => {
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key === 'Escape') {
                event.preventDefault();
                onClose();
            }
        };
        window.addEventListener('keydown', handleKeyDown);
        return () => {
            window.removeEventListener('keydown', handleKeyDown);
        };
    }, [onClose]);

    return (
        <div className="flex h-full flex-col bg-slate-100">
            <div className="flex min-h-0 flex-1 flex-col px-4 py-4 sm:px-6 lg:px-8">
                <section className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-[0_24px_60px_-32px_rgba(15,23,42,0.24)]">
                    <header className="flex items-start justify-between gap-3 border-b border-slate-200 bg-white px-6 py-4">
                        <div className="min-w-0">
                            <div className="text-[11px] font-semibold uppercase tracking-[0.18em] text-slate-400">
                                Workspace Settings
                            </div>
                            <h2 className="mt-1 text-lg font-semibold text-slate-900">設定</h2>
                        </div>
                        <button
                            type="button"
                            onClick={onClose}
                            aria-label="設定画面を閉じる"
                            className="inline-flex h-8 w-8 items-center justify-center rounded-xl border border-slate-200 bg-white text-slate-500 transition-colors hover:bg-slate-50 hover:text-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
                        >
                            <X size={16} />
                        </button>
                    </header>

                    <div className="min-h-0 flex-1">
                        <SettingsProvider onClose={onClose} closeOnSave={false}>
                            <SettingsShell mode="page" onClose={onClose} closeLabel="前の画面へ戻る" />
                        </SettingsProvider>
                    </div>
                </section>
            </div>
        </div>
    );
}
