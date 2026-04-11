import { ChevronRight } from 'lucide-react';
import { cn } from '../Modal';
import type { SettingsSectionId } from './SettingsContext';

export interface SettingsSidebarSection {
    id: SettingsSectionId;
    label: string;
    description: string;
}

export interface SettingsSidebarCategory {
    label: string;
    sections: SettingsSidebarSection[];
}

interface SettingsSidebarProps {
    categories: SettingsSidebarCategory[];
    activeSection: SettingsSectionId;
    onSelect: (sectionId: SettingsSectionId) => void;
    className?: string;
}

export function SettingsSidebar({
    categories,
    activeSection,
    onSelect,
    className,
}: SettingsSidebarProps) {
    return (
        <nav
            className={cn(
                'h-full overflow-y-auto border-r border-slate-200 bg-white/90 px-3 py-4',
                className,
            )}
            aria-label="設定セクション"
        >
            <div className="space-y-5">
                {categories.map((category) => (
                    <div key={category.label}>
                        <div className="px-3 text-xs font-semibold uppercase tracking-[0.22em] text-slate-400">
                            {category.label}
                        </div>
                        <div className="mt-2 space-y-1">
                            {category.sections.map((section) => {
                                const selected = section.id === activeSection;

                                return (
                                    <button
                                        key={section.id}
                                        type="button"
                                        onClick={() => onSelect(section.id)}
                                        className={cn(
                                            'flex w-full items-start gap-3 rounded-2xl px-3 py-3 text-left transition-colors',
                                            selected
                                                ? 'bg-slate-900 text-white shadow-sm'
                                                : 'text-slate-600 hover:bg-slate-100 hover:text-slate-900',
                                        )}
                                    >
                                        <div className="min-w-0 flex-1">
                                            <div className="text-sm font-semibold">
                                                {section.label}
                                            </div>
                                            <div
                                                className={cn(
                                                    'mt-1 text-xs leading-5',
                                                    selected ? 'text-slate-200' : 'text-slate-400',
                                                )}
                                            >
                                                {section.description}
                                            </div>
                                        </div>
                                        <ChevronRight
                                            size={16}
                                            className={cn(
                                                'mt-0.5 shrink-0',
                                                selected ? 'text-slate-300' : 'text-slate-300',
                                            )}
                                        />
                                    </button>
                                );
                            })}
                        </div>
                    </div>
                ))}
            </div>
        </nav>
    );
}
