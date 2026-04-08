import { convertFileSrc } from '@tauri-apps/api/core';

export const PO_ASSISTANT_ROLE_NAME = 'POアシスタント';
export const DEV_AGENT_LABEL = '開発エージェント';

export type AgentAvatarKind = 'po-assistant' | 'dev-agent';

type AvatarDefinition = {
    kind: AgentAvatarKind;
    label: string;
    src: string;
    backgroundClassName: string;
    iconClassName: string;
    ringClassName: string;
    imageSurfaceClassName: string;
    imageClassName: string;
};

const AVATAR_DEFINITIONS: Record<AgentAvatarKind, AvatarDefinition> = {
    'po-assistant': {
        kind: 'po-assistant',
        label: PO_ASSISTANT_ROLE_NAME,
        src: '/avatars/po-assistant.png',
        backgroundClassName: 'bg-gradient-to-br from-emerald-100 via-lime-50 to-amber-100',
        iconClassName: 'text-emerald-700',
        ringClassName: 'ring-emerald-200/80',
        imageSurfaceClassName: 'bg-transparent p-0.5',
        imageClassName: 'object-contain',
    },
    'dev-agent': {
        kind: 'dev-agent',
        label: DEV_AGENT_LABEL,
        src: '/avatars/dev-agent.png',
        backgroundClassName: 'bg-gradient-to-br from-sky-100 via-cyan-50 to-blue-100',
        iconClassName: 'text-sky-700',
        ringClassName: 'ring-sky-200/80',
        imageSurfaceClassName: 'bg-white/96 p-1 shadow-inner shadow-slate-300/60',
        imageClassName: 'object-contain',
    },
};

export function normalizeRoleName(roleName: string | null | undefined): string {
    return roleName?.trim() ?? '';
}

export function isPoAssistantRole(roleName: string | null | undefined): boolean {
    return normalizeRoleName(roleName) === PO_ASSISTANT_ROLE_NAME;
}

export function getAvatarDefinition(kind: AgentAvatarKind): AvatarDefinition {
    return AVATAR_DEFINITIONS[kind];
}

export function resolveAvatarImageSource(imageSource: string | null | undefined): string | null {
    if (!imageSource) {
        return null;
    }

    if (
        imageSource.startsWith('data:') ||
        imageSource.startsWith('blob:') ||
        imageSource.startsWith('http://') ||
        imageSource.startsWith('https://') ||
        imageSource.startsWith('asset:') ||
        imageSource.startsWith('/')
    ) {
        return imageSource;
    }

    if (/^[A-Za-z]:[\\/]/.test(imageSource) || imageSource.startsWith('\\\\')) {
        return convertFileSrc(imageSource);
    }

    return imageSource;
}

export function resolveAvatarForRoleName(roleName: string | null | undefined): AvatarDefinition {
    return isPoAssistantRole(roleName)
        ? AVATAR_DEFINITIONS['po-assistant']
        : AVATAR_DEFINITIONS['dev-agent'];
}
