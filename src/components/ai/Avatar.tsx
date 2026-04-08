import { useEffect, useState } from 'react';
import { Bot, Sparkles } from 'lucide-react';
import { AgentAvatarKind, getAvatarDefinition, resolveAvatarImageSource } from './avatarRegistry';

type AvatarSize = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'xxl';

interface AvatarProps {
    kind: AgentAvatarKind;
    size?: AvatarSize;
    alt?: string;
    className?: string;
    imageSrc?: string | null;
}

const SIZE_CLASS_MAP: Record<AvatarSize, string> = {
    xs: 'h-6 w-6',
    sm: 'h-8 w-8',
    md: 'h-10 w-10',
    lg: 'h-14 w-14',
    xl: 'h-24 w-24',
    xxl: 'h-32 w-32',
};

const ICON_SIZE_MAP: Record<AvatarSize, number> = {
    xs: 12,
    sm: 16,
    md: 18,
    lg: 26,
    xl: 36,
    xxl: 44,
};

export function Avatar({ kind, size = 'md', alt, className = '', imageSrc }: AvatarProps) {
    const definition = getAvatarDefinition(kind);
    const [imageFailed, setImageFailed] = useState(false);
    const FallbackIcon = kind === 'po-assistant' ? Sparkles : Bot;
    const resolvedImageSrc = resolveAvatarImageSource(imageSrc) ?? definition.src;

    useEffect(() => {
        setImageFailed(false);
    }, [resolvedImageSrc]);

    return (
        <div
            className={`relative flex shrink-0 items-center justify-center overflow-hidden rounded-full ring-2 ${definition.ringClassName} ${SIZE_CLASS_MAP[size]} ${className}`.trim()}
            aria-label={alt ?? definition.label}
            title={alt ?? definition.label}
        >
            {!imageFailed ? (
                <div className={`flex h-full w-full items-center justify-center rounded-full ${definition.imageSurfaceClassName}`}>
                    <img
                        src={resolvedImageSrc}
                        alt={alt ?? definition.label}
                        className={`h-full w-full ${definition.imageClassName}`}
                        onError={() => setImageFailed(true)}
                    />
                </div>
            ) : (
                <div className={`flex h-full w-full items-center justify-center ${definition.backgroundClassName}`}>
                    <FallbackIcon size={ICON_SIZE_MAP[size]} className={definition.iconClassName} />
                </div>
            )}
        </div>
    );
}
