# Epic 40: 設定画面 - チーム設定 CLI 種別対応 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 37, 38 完了
- 作成日: 2026-04-09

## Epic の目的

ユーザーがチーム内の各ロール（Lead Engineer、Frontend Dev など）にそれぞれ異なる CLI ツールを割り当てられるようにする。例えば Lead Engineer は Claude Code、Frontend Dev は Gemini CLI というチーム編成が可能になる。

## スコープ

### 対象ファイル（変更）
- `src/components/ui/TeamSettingsTab.tsx` — CLI 種別ドロップダウン追加、モデル選択連動
- `src-tauri/src/db.rs` — シードデータ更新（必要に応じて）

### 対象外
- バックエンドの CLI Runner ロジック（Epic 37, 38 で完了済み）
- `frontend-core` 配下（CLAUDE.md ルールに従い変更しない）

## 実装方針

### 1. ロールカード UI の拡張

現在のロールカード構成:
```
┌──────────────────────────────┐
│ [アバター] ロール名           │
│ Claude モデル: [ドロップダウン]│
│ システムプロンプト: [テキスト] │
└──────────────────────────────┘
```

変更後:
```
┌──────────────────────────────────┐
│ [アバター] ロール名               │
│ CLI: [Claude Code ▾]             │  ← 新規追加
│ モデル: [claude-sonnet-4 ▾]      │  ← ラベル変更 + 連動
│ システムプロンプト: [テキスト]      │
└──────────────────────────────────┘
```

### 2. CLI 種別ドロップダウンの実装

```typescript
const CLI_OPTIONS = [
    { value: 'claude', label: 'Claude Code', icon: '🟠' },
    { value: 'gemini', label: 'Gemini CLI', icon: '🔵' },
    { value: 'codex', label: 'Codex CLI', icon: '🟢' },
];

const DEFAULT_MODELS: Record<string, string> = {
    claude: 'claude-sonnet-4-20250514',
    gemini: 'gemini-2.5-pro',
    codex: 'o3',
};
```

### 3. CLI 変更時のモデルリセットロジック

```typescript
const handleCliTypeChange = (roleIndex: number, newCliType: string) => {
    const updated = [...roles];
    updated[roleIndex] = {
        ...updated[roleIndex],
        cli_type: newCliType,
        model: DEFAULT_MODELS[newCliType] || '',
    };
    setRoles(updated);
};
```

### 4. 未インストール CLI の警告表示

`useCliDetection()` の結果を利用:

```typescript
const { results: cliResults } = useCliDetection();

// ドロップダウン内の各オプションに (未検出) を付与
const getCliLabel = (cliValue: string) => {
    const detection = cliResults.find(r => r.name === cliValue);
    const option = CLI_OPTIONS.find(o => o.value === cliValue);
    if (!detection?.installed) {
        return `${option.label} (未検出)`;
    }
    return `${option.label} v${detection.version}`;
};
```

ロール保存時にも追加バリデーション:
```typescript
const uninstalledRoles = roles.filter(role => {
    const detection = cliResults.find(r => r.name === role.cli_type);
    return !detection?.installed;
});
if (uninstalledRoles.length > 0) {
    // 警告を表示（保存はブロックしない — CLI は後からインストール可能）
}
```

### 5. 保存処理の更新

`save_team_configuration` の引数に `cli_type` を含める:

```typescript
await invoke('save_team_configuration', {
    config: {
        max_concurrent_agents: maxAgents,
        roles: roles.map((role, index) => ({
            id: role.id,
            name: role.name,
            system_prompt: role.system_prompt,
            cli_type: role.cli_type,    // 追加
            model: role.model,
            avatar_image: role.avatar_image,
            sort_order: index,
        })),
    },
});
```

## テスト方針

- CLI 種別ドロップダウンが全3選択肢を表示すること
- CLI 変更 → モデルがデフォルト値にリセットされること
- 保存 → 再読み込みで `cli_type` が維持されること
- 未インストール CLI に `(未検出)` ラベルが表示されること
- 未インストール CLI のロールがあっても保存自体は可能であること
- タスク実行時に正しい CLI が選択されること（Epic 37 との結合テスト）
