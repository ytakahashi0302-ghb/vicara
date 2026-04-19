import { invoke } from '@tauri-apps/api/core';

export const PROJECT_ROOT_PREVIEW_INVALIDATED_EVENT = 'vicara:project-root-preview-invalidated';

export type PreviewPreset = {
    kind: 'command' | 'static';
    command?: 'npm run dev' | 'npm run serve';
    label: string;
};

function readPackageScripts(packageJsonContent: string | null): { dev: boolean; serve: boolean } {
    if (!packageJsonContent) {
        return { dev: false, serve: false };
    }

    try {
        const parsed = JSON.parse(packageJsonContent) as { scripts?: Record<string, unknown> };
        return {
            dev: typeof parsed.scripts?.dev === 'string',
            serve: typeof parsed.scripts?.serve === 'string',
        };
    } catch (error) {
        console.error('Failed to parse package.json for preview detection', error);
        return { dev: false, serve: false };
    }
}

export function resolvePreviewPreset(
    architectureContent: string | null,
    packageJsonContent: string | null,
    hasIndexHtml: boolean,
): PreviewPreset | null {
    if (!architectureContent) {
        return null;
    }

    const content = architectureContent.toLowerCase();
    const scripts = readPackageScripts(packageJsonContent);

    const staticKeywords = [
        'vanilla js',
        'vanilla javascript',
        'plain javascript',
        'static site',
        'static html',
        'html/css/javascript',
        'html, css, javascript',
        '静的サイト',
        '静的 html',
        'バニラjs',
        'バニラ javascript',
        'vanilla',
    ];

    const devKeywords = [
        'react',
        'vite',
        'vue',
        'svelte',
        'astro',
        'next.js',
        'nextjs',
        'nuxt',
        'frontend framework',
    ];

    if (devKeywords.some((keyword) => content.includes(keyword)) && scripts.dev) {
        return {
            kind: 'command',
            command: 'npm run dev',
            label: '開発サーバープレビュー',
        };
    }

    if (staticKeywords.some((keyword) => content.includes(keyword))) {
        if (scripts.serve) {
            return {
                kind: 'command',
                command: 'npm run serve',
                label: '静的サイト向けプレビュー',
            };
        }
        if (scripts.dev) {
            return {
                kind: 'command',
                command: 'npm run dev',
                label: '開発サーバープレビュー',
            };
        }
        if (hasIndexHtml) {
            return {
                kind: 'static',
                label: '静的ファイルプレビュー',
            };
        }
    }

    return null;
}

export async function detectPreviewPresetForProject(projectPath: string): Promise<PreviewPreset | null> {
    const [architecture, packageJson, indexHtml] = await Promise.all([
        invoke<string | null>('read_inception_file', {
            localPath: projectPath,
            filename: 'ARCHITECTURE.md',
        }),
        invoke<string | null>('read_inception_file', {
            localPath: projectPath,
            filename: 'package.json',
        }),
        invoke<string | null>('read_inception_file', {
            localPath: projectPath,
            filename: 'index.html',
        }),
    ]);

    return resolvePreviewPreset(architecture, packageJson, Boolean(indexHtml));
}
