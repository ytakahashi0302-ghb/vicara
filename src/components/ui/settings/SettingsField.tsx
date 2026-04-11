import type { ReactNode } from 'react';
import { cn } from '../Modal';

interface SettingsFieldProps {
    label: string;
    description?: ReactNode;
    children: ReactNode;
    className?: string;
    htmlFor?: string;
}

export function SettingsField({
    label,
    description,
    children,
    className,
    htmlFor,
}: SettingsFieldProps) {
    return (
        <div
            className={cn(
                'grid gap-4 rounded-2xl border border-slate-200 bg-white/90 px-4 py-4 shadow-sm lg:grid-cols-[minmax(0,220px)_1fr] lg:items-start',
                className,
            )}
        >
            <div className="min-w-0">
                <label
                    htmlFor={htmlFor}
                    className="block text-sm font-semibold text-slate-900"
                >
                    {label}
                </label>
                {description && (
                    <div className="mt-1 text-sm leading-6 text-slate-500">
                        {description}
                    </div>
                )}
            </div>

            <div className="min-w-0">{children}</div>
        </div>
    );
}
