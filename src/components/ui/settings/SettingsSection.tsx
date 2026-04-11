import type { ReactNode } from 'react';
import { cn } from '../Modal';

interface SettingsSectionProps {
    title: string;
    description: ReactNode;
    children: ReactNode;
    actions?: ReactNode;
    className?: string;
}

export function SettingsSection({
    title,
    description,
    children,
    actions,
    className,
}: SettingsSectionProps) {
    return (
        <section className={cn('space-y-6', className)}>
            <div className="flex flex-col gap-4 border-b border-slate-200 pb-5 lg:flex-row lg:items-start lg:justify-between">
                <div className="min-w-0">
                    <h3 className="text-xl font-semibold text-slate-950">{title}</h3>
                    <p className="mt-2 max-w-3xl text-sm leading-6 text-slate-600">
                        {description}
                    </p>
                </div>
                {actions && (
                    <div className="flex shrink-0 flex-wrap gap-2">
                        {actions}
                    </div>
                )}
            </div>

            <div className="space-y-4">{children}</div>
        </section>
    );
}
