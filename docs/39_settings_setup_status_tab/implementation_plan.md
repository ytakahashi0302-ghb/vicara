# Epic 39: 設定画面 - セットアップ状況タブ 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 36 完了
- 作成日: 2026-04-09

## Epic の目的

ユーザーが「何がセットアップ済みで、何が足りないか」を一目で把握できるダッシュボードタブを設定画面に追加する。特に初回起動時やツールインストール後の確認をスムーズにする。

## スコープ

### 対象ファイル（新規）
- `src/components/ui/SetupStatusTab.tsx` — セットアップ状況表示コンポーネント

### 対象ファイル（変更）
- `src/components/ui/GlobalSettingsModal.tsx` — タブ構成変更、デフォルトタブロジック

### 対象外
- チーム設定タブの CLI 種別対応（Epic 40）
- `frontend-core` 配下（CLAUDE.md ルールに従い変更しない）

## 実装方針

### 1. SetupStatusTab の設計

```
┌─────────────────────────────────────────────────┐
│  セットアップ状況                      [再検出]   │
├─────────────────────────────────────────────────┤
│                                                 │
│  🔧 開発ツール                                   │
│  ┌───────────────────┬──────────┬─────────────┐ │
│  │ Git               │ ✅ v2.43 │             │ │
│  │ Claude Code CLI   │ ✅ v1.x  │             │ │
│  │ Gemini CLI        │ ❌ 未検出 │ [導入方法]   │ │
│  │ Codex CLI         │ ❌ 未検出 │ [導入方法]   │ │
│  └───────────────────┴──────────┴─────────────┘ │
│                                                 │
│  🔑 API キー                                     │
│  ┌───────────────────┬──────────┐               │
│  │ Anthropic         │ ✅ 設定済 │               │
│  │ Gemini            │ ⚠️ 未設定 │               │
│  └───────────────────┴──────────┘               │
│                                                 │
│  ℹ️ Dev エージェントには Git + CLI が1つ以上必要   │
│  ℹ️ PO アシスタントには API キーが必要             │
└─────────────────────────────────────────────────┘
```

### 2. データソース

| データ | 取得方法 |
|--------|---------|
| Git ステータス | `WorkspaceContext` の `gitStatus`（既存） |
| CLI 検出結果 | `useCliDetection()` フック（Epic 36 で作成） |
| API キー有無 | `invoke('check_api_key_status')` 新コマンド、または store 直接参照 |

API キー有無の検出方法:
- 方法 A: 新しい Tauri コマンド `check_api_key_status` を追加（バックエンドで store を確認）
- 方法 B: フロントエンドから直接 store を参照
- **推奨: 方法 A** — API キーの値がフロントエンドに露出しない

```rust
#[tauri::command]
pub async fn check_api_key_status(app: AppHandle) -> Result<Vec<ApiKeyStatus>, String> {
    // settings.json から各キーの有無のみ返す
    // { name: "anthropic", configured: true }
    // { name: "gemini", configured: false }
}
```

### 3. GlobalSettingsModal のタブ構成変更

現在のタブ順:
```
1. POアシスタント設定  2. プロジェクト設定  3. チーム設定
```

変更後:
```
1. セットアップ状況  2. POアシスタント設定  3. チーム設定  4. プロジェクト設定
```

デフォルトタブ選択ロジック:
```typescript
const defaultTab = useMemo(() => {
    const hasAnyCli = cliResults.some(r => r.installed);
    const hasAnyApiKey = apiKeyStatus.some(k => k.configured);
    // CLI も API キーもない場合はセットアップタブを表示
    if (!hasAnyCli && !hasAnyApiKey) return 'setup';
    return 'po-assistant'; // 通常時は POアシスタントタブ
}, [cliResults, apiKeyStatus]);
```

### 4. スタイリング

既存の `GlobalSettingsModal.tsx` のスタイル（Tailwind CSS）に合わせる。ステータス表示には:
- ✅ 緑色テキスト + チェックアイコン
- ❌ 赤色テキスト + バツアイコン
- ⚠️ 黄色テキスト + 警告アイコン

## テスト方針

- 全ツールインストール済み → 全項目に ✅ が表示されること
- CLI 未インストール → 該当行に ❌ と導入方法リンクが表示されること
- API キー未設定 → 該当行に ⚠️ が表示されること
- 全未セットアップ → セットアップ状況タブがデフォルトで開くこと
- 再検出ボタン → CLI 検出結果が更新されること
