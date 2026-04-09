# Epic 36: Git ブロッキング修正 + CLI 検出基盤 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: PO 承認済み
- 作成日: 2026-04-09

## Epic の目的

現在 Git がインストールされていない環境ではアプリが完全にブロックされ、PO アシスタントやカンバン操作すら行えない。Git はDev エージェントの worktree 機能にのみ必要であるため、ブロッキングを解除してワーニングに変更する。また、後続 Epic で複数 CLI をサポートするための検出基盤を本 Epic で整備する。

## スコープ

### 対象ファイル（変更）
- `src/App.tsx` — Git 未インストール時のフルスクリーンブロック削除、ワーニングバナー追加
- `src-tauri/src/lib.rs` — 新コマンド登録

### 対象ファイル（新規）
- `src-tauri/src/cli_detection.rs` — CLI 検出ロジック
- `src/hooks/useCliDetection.ts` — フロントエンド検出フック

### 対象外
- `src/context/WorkspaceContext.tsx` — `gitStatus` 管理は既存のまま維持（削除しない）
- `src-tauri/src/git.rs` — 変更なし
- `src-tauri/src/worktree.rs` — Git 未インストール時のガードは既存で十分

## 実装方針

### 1. Git ブロッキング解除

`App.tsx` L366-408 のブロッキング分岐:

```typescript
// 現状: アプリ全体をブロック
if (gitStatus.checked && !gitStatus.installed) {
    return (<div>Gitが見つかりません...</div>);
}
```

変更後:
```typescript
// ブロックせず、バナー警告のみ表示
{gitStatus.checked && !gitStatus.installed && (
    <WarningBanner message="Git が検出されません。Dev エージェント機能を使用するには Git のインストールが必要です。" />
)}
// 通常のアプリ UI をそのまま表示
```

### 2. CLI 検出コマンド

`cli_detection.rs` の設計:

```rust
#[derive(Serialize)]
pub struct CliDetectionResult {
    pub name: String,        // "claude" | "gemini" | "codex"
    pub display_name: String, // "Claude Code" | "Gemini CLI" | "Codex CLI"
    pub installed: bool,
    pub version: Option<String>,
}

#[tauri::command]
pub async fn detect_installed_clis() -> Vec<CliDetectionResult> {
    // "claude --version", "gemini --version", "codex --version" を並列実行
    // 各 CLI の NotFound → installed: false, 成功 → installed: true + version パース
}
```

既存の `git.rs` の `check_git_installed` と同パターン（`Command::new(name).arg("--version")`）を踏襲する。

### 3. フロントエンドフック

```typescript
export function useCliDetection() {
    const [results, setResults] = useState<CliDetectionResult[]>([]);
    const [loading, setLoading] = useState(true);

    const refresh = async () => {
        setLoading(true);
        const data = await invoke<CliDetectionResult[]>('detect_installed_clis');
        setResults(data);
        setLoading(false);
    };

    useEffect(() => { refresh(); }, []);

    return { results, loading, refresh };
}
```

## テスト方針

- Git 未インストール環境でアプリ起動 → ワーニングバナーが表示され、他機能は通常通り使えること
- Claude CLI インストール済み環境で `detect_installed_clis` → `{ name: "claude", installed: true, version: "1.x.x" }`
- 存在しない CLI → `{ installed: false, version: null }`
- 全 CLI 未インストールでもコマンド自体はエラーにならないこと
