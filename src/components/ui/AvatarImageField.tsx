import { useRef } from 'react';
import { ImagePlus, RotateCcw } from 'lucide-react';
import { Button } from './Button';
import { Avatar } from '../ai/Avatar';
import { AgentAvatarKind, getAvatarDefinition, resolveAvatarImageSource } from '../ai/avatarRegistry';

interface AvatarImageFieldProps {
    label: string;
    description: string;
    value: string | null | undefined;
    fallbackKind: AgentAvatarKind;
    previewMode?: 'avatar' | 'figure';
    onChange: (value: string | null) => void;
}

export function AvatarImageField({
    label,
    description,
    value,
    fallbackKind,
    previewMode = 'avatar',
    onChange,
}: AvatarImageFieldProps) {
    const inputRef = useRef<HTMLInputElement>(null);
    const defaultDefinition = getAvatarDefinition(fallbackKind);
    const resolvedPreviewImage = resolveAvatarImageSource(value) ?? defaultDefinition.src;

    const handleChooseImage = () => {
        inputRef.current?.click();
    };

    const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
        const file = event.target.files?.[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = () => {
            if (typeof reader.result === 'string') {
                onChange(reader.result);
            }
        };
        reader.readAsDataURL(file);
        event.target.value = '';
    };

    return (
        <div className="rounded-2xl border border-slate-200 bg-slate-50/70 p-4">
            <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
                <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium text-slate-800">{label}</div>
                    <p className="mt-1 text-sm leading-6 text-slate-500">{description}</p>
                </div>

                {previewMode === 'avatar' ? (
                    <Avatar
                        kind={fallbackKind}
                        size="xl"
                        imageSrc={value}
                        className="border border-white/80 shadow-sm"
                    />
                ) : (
                    <div className="relative h-40 w-28 shrink-0 overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm">
                        <img
                            src={resolvedPreviewImage}
                            alt={label}
                            className="h-full w-full object-contain p-2"
                        />
                    </div>
                )}
            </div>

            <div className="mt-4 flex flex-wrap gap-2">
                <Button type="button" variant="secondary" onClick={handleChooseImage}>
                    <ImagePlus size={15} className="mr-2" />
                    画像を選択
                </Button>
                <Button type="button" variant="ghost" onClick={() => onChange(null)}>
                    <RotateCcw size={15} className="mr-2" />
                    デフォルトに戻す
                </Button>
            </div>

            <input
                ref={inputRef}
                type="file"
                accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
                className="hidden"
                onChange={handleFileChange}
            />
        </div>
    );
}
