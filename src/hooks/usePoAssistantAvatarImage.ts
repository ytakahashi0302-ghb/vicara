import { useEffect, useState } from 'react';
import { load } from '@tauri-apps/plugin-store';

export const VICARA_SETTINGS_UPDATED_EVENT = 'vicara:settings-updated';
export const PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY = 'po-assistant-avatar-image';

export function normalizeStoredStringValue(value: unknown): string | null {
    if (typeof value === 'string') {
        const trimmed = value.trim();
        return trimmed.length > 0 ? trimmed : null;
    }

    if (
        value &&
        typeof value === 'object' &&
        'value' in value &&
        typeof (value as { value?: unknown }).value === 'string'
    ) {
        const trimmed = ((value as { value: string }).value ?? '').trim();
        return trimmed.length > 0 ? trimmed : null;
    }

    return null;
}

export function usePoAssistantAvatarImage() {
    const [avatarImage, setAvatarImage] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;

        const loadAvatarImage = async () => {
            try {
                const store = await load('settings.json');
                const storedValue = await store.get(PO_ASSISTANT_AVATAR_IMAGE_STORE_KEY);
                if (!cancelled) {
                    setAvatarImage(normalizeStoredStringValue(storedValue));
                }
            } catch (error) {
                console.error('Failed to load PO assistant avatar image', error);
                if (!cancelled) {
                    setAvatarImage(null);
                }
            }
        };

        const handleSettingsUpdated = () => {
            void loadAvatarImage();
        };

        void loadAvatarImage();
        window.addEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);

        return () => {
            cancelled = true;
            window.removeEventListener(VICARA_SETTINGS_UPDATED_EVENT, handleSettingsUpdated);
        };
    }, []);

    return avatarImage;
}
