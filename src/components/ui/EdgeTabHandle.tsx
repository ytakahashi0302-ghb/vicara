import type { ComponentType, ReactNode } from 'react';
import { cn } from './Modal';

/**
 * EdgeTabHandle (EPIC45 v2)
 *
 * 画面端に貼り付く統一デザインのタブハンドル。設定ドロワー (左端)、
 * POアシスタント (右端)、チームの稼働状況 (下端) の開閉トリガを
 * 共通パターンとして提供する。
 *
 * スタイルルール:
 *  - base: bg-slate-50 / text-slate-600 / border-slate-200 / shadow-md
 *  - active: ring-2 ring-blue-500 + border-slate-300
 *  - hover: text-blue-600
 *  - radius: rounded-xl 統一
 */

type EdgeSide = 'left' | 'right' | 'bottom';

interface EdgeTabHandleProps {
    side: EdgeSide;
    label: string;
    icon: ComponentType<{ size?: number; className?: string }>;
    active?: boolean;
    onClick: () => void;
    title?: string;
    badge?: ReactNode;
    className?: string;
}

function sideContainerClasses(side: EdgeSide, active: boolean): string {
    // 画面端に固定する絶対配置 + 角丸の向きを 1 箇所に集約
    const base = 'pointer-events-auto shadow-md transition-colors';
    const radius =
        side === 'left'
            ? 'rounded-r-xl'
            : side === 'right'
              ? 'rounded-l-xl'
              : 'rounded-t-xl';
    const border =
        side === 'left'
            ? 'border border-l-0 border-slate-200'
            : side === 'right'
              ? 'border border-r-0 border-slate-200'
              : 'border border-b-0 border-slate-200';
    const activeState = active
        ? 'bg-slate-50 text-slate-700 ring-2 ring-blue-500 border-slate-300'
        : 'bg-slate-50 text-slate-600 hover:text-blue-600 hover:border-slate-300';
    return cn(base, radius, border, activeState);
}

function sideLayoutClasses(side: EdgeSide): string {
    if (side === 'bottom') {
        return 'flex h-9 items-center gap-2 px-4 text-xs font-semibold';
    }
    // left / right: 縦書き。高さを稼ぎ、文字は縦方向に並べる
    return 'flex w-9 flex-col items-center justify-center gap-2 py-4 text-xs font-semibold';
}

export function EdgeTabHandle({
    side,
    label,
    icon: Icon,
    active = false,
    onClick,
    title,
    badge,
    className,
}: EdgeTabHandleProps) {
    const isVertical = side === 'left' || side === 'right';

    return (
        <button
            type="button"
            onClick={onClick}
            title={title ?? label}
            aria-pressed={active}
            className={cn(
                sideContainerClasses(side, active),
                sideLayoutClasses(side),
                'focus:outline-none focus:ring-2 focus:ring-blue-500',
                className,
            )}
        >
            <Icon size={16} className="shrink-0" />
            <span
                className={cn(
                    'whitespace-nowrap tracking-[0.12em] uppercase',
                    isVertical && '[writing-mode:vertical-rl] [text-orientation:mixed]',
                )}
            >
                {label}
            </span>
            {badge && (
                <span className="inline-flex h-4 min-w-[16px] items-center justify-center rounded-full bg-blue-600 px-1 text-[10px] font-semibold text-white">
                    {badge}
                </span>
            )}
        </button>
    );
}
