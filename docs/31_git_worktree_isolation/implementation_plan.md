# Epic 31: Git Worktreeを用いたエージェント環境の完全隔離と1-ClickプレビューUXの実装

## 実装計画書 (Implementation Plan)

---

## 実装結果サマリ（2026-04-07）

- [x] Git Worktree を用いた task 単位の物理隔離
- [x] Claude 実行完了時の `Review` 遷移と 1-Click マージ導線
- [x] プレビューサーバー管理と静的 `index.html` 直開き対応
- [x] `git.rs` / `preview.rs` / `worktree.rs` の3層責務分離
- [x] `ensure_git_repo` によるゼロ構成初期化
- [x] Git未インストール時の起動時ガードと UI 案内
- [x] `cargo test` によるバックエンド回帰確認
- [ ] Review フローの E2E / クロスプラットフォーム手動検証

---

## 1. 概要

本Epicでは、複数AIエージェントが同時にコードを編集する際のファイル競合を完全に排除するため、**Git Worktree**を活用したタスクごとの物理的ファイルシステム隔離を実装する。さらに、ユーザーがGit操作を一切意識せずにコードレビュー・動作確認・マージを完結できる**1-Clickレビュー体験**をカンバンボードに統合する。

### 開発方針: トランクベース開発
- AIエージェントは常に `main` ブランチから作業を開始
- タスクごとに `feature/task-<ID>` ブランチを自動生成
- 作業完了後は `main` へ自動マージ（Fast-forward or 3-way merge）

---

## 2. アーキテクチャ設計

### 2.1 全体フロー

```
[タスク実行開始]
    │
    ▼
┌─────────────────────────────────┐
│ 1. Worktree生成                  │
│    git worktree add              │
│    .scrum-ai-worktrees/task-<ID> │
│    -b feature/task-<ID> main     │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 2. Claude CLI実行               │
│    cwd = worktree path           │
│    --add-dir = worktree path     │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 3. 完了 → ステータスをReviewへ    │
│    カンバンのReview列に表示       │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 4. ユーザーレビュー              │
│    ▶️ プレビュー起動              │
│    ✅ 承認してマージ              │
└────────────┬────────────────────┘
             │
             ▼
┌─────────────────────────────────┐
│ 5. 自動マージ & クリーンアップ     │
│    git merge → worktree remove   │
│    → branch delete               │
└─────────────────────────────────┘
```

### 2.2 ディレクトリ構成

```
<project-root>/
├── .scrum-ai-worktrees/           # 隠しフォルダ（.gitignore対象）
│   ├── task-abc123/               # タスクAのワークツリー
│   │   ├── src/
│   │   ├── package.json
│   │   └── ...
│   └── task-def456/               # タスクBのワークツリー（並行稼働）
│       ├── src/
│       └── ...
├── src/                           # mainブランチ（本体）
├── .gitignore                     # .scrum-ai-worktrees/ を追加
└── ...
```

---

## 3. バックエンド設計 (Tauri / Rust)

### 3.1 新規モジュール: `src-tauri/src/worktree.rs`

Git Worktreeのライフサイクル管理を担う専用モジュールを新設する。

#### 主要関数・コマンド

| Tauriコマンド | 説明 |
|---|---|
| `create_worktree(project_path, task_id)` | ワークツリー生成 + ブランチ作成 |
| `remove_worktree(project_path, task_id)` | ワークツリー削除 + ブランチ削除 |
| `merge_worktree(project_path, task_id)` | feature → main マージ + クリーンアップ |
| `get_worktree_status(project_path, task_id)` | ワークツリーの存在確認・状態取得 |
| `start_preview_server(project_path, task_id, command, port)` | ワークツリー内でdevサーバー起動 |
| `stop_preview_server(task_id)` | プレビューサーバー停止 |
| `open_preview_in_browser(port)` | ブラウザでプレビューURL表示 |
| `get_worktree_diff(project_path, task_id)` | mainとの差分取得（レビュー用） |

#### 3.1.1 ワークツリー生成の詳細

```rust
// 擬似コード
async fn create_worktree(project_path: &str, task_id: &str) -> Result<String> {
    let worktree_dir = format!("{}/.scrum-ai-worktrees/task-{}", project_path, task_id);
    let branch_name = format!("feature/task-{}", task_id);

    // 1. .scrum-ai-worktrees ディレクトリ確認・作成
    fs::create_dir_all(&worktree_dir)?;

    // 2. mainブランチが最新であることを確認
    // (ローカルのみ。リモート同期はスコープ外)

    // 3. git worktree add
    Command::new("git")
        .args(["worktree", "add", &worktree_dir, "-b", &branch_name, "main"])
        .current_dir(project_path)
        .output()?;

    // 4. .gitignore に .scrum-ai-worktrees/ を追記（初回のみ）
    ensure_gitignore_entry(project_path, ".scrum-ai-worktrees/")?;

    Ok(worktree_dir)
}
```

#### 3.1.2 自動マージの詳細

```rust
async fn merge_worktree(project_path: &str, task_id: &str) -> Result<MergeResult> {
    let branch_name = format!("feature/task-{}", task_id);

    // 1. ワークツリー内の変更をコミット（未コミットがあれば）
    auto_commit_if_needed(&worktree_path)?;

    // 2. mainブランチでマージ実行
    let output = Command::new("git")
        .args(["merge", &branch_name, "--no-ff", "-m", &format!("Merge task-{}", task_id)])
        .current_dir(project_path)
        .output()?;

    if !output.status.success() {
        // コンフリクト発生 → マージを中止し、ユーザーに通知
        Command::new("git")
            .args(["merge", "--abort"])
            .current_dir(project_path)
            .output()?;

        return Ok(MergeResult::Conflict {
            conflicting_files: parse_conflict_files(&output.stderr),
        });
    }

    // 3. クリーンアップ
    remove_worktree(project_path, task_id)?;

    Ok(MergeResult::Success)
}
```

### 3.2 既存モジュールへの変更

#### `claude_runner.rs` の変更

`execute_claude_task` コマンドを拡張し、ワークツリーパスを `cwd` として使用する。

```rust
// 変更点: タスク実行時にworktreeを自動生成し、そのパスでClaude CLIを実行
pub async fn execute_claude_task(...) {
    // 既存: cwd = project.local_path
    // 新規: cwd = create_worktree(project.local_path, task_id)

    let worktree_path = worktree::create_worktree(&project.local_path, &task_id).await?;

    // Claude CLIのcwdとadd-dirをworktree_pathに変更
    let mut cmd = Command::new("claude");
    cmd.current_dir(&worktree_path)
       .args(["--add-dir", &worktree_path, ...]);
}
```

#### `db.rs` のスキーマ変更

タスクに `Review` ステータスとワークツリー関連メタデータを追加する。

```sql
-- Migration v14
-- タスクステータスに 'Review' を追加（既存のCHECK制約を更新）
-- ※ SQLiteではCHECK制約の変更にテーブル再作成が必要な場合がある

-- ワークツリー管理テーブル
CREATE TABLE IF NOT EXISTS worktrees (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    worktree_path TEXT NOT NULL,
    branch_name TEXT NOT NULL,
    preview_port INTEGER,
    preview_pid INTEGER,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK(status IN ('active', 'merging', 'merged', 'conflict', 'removed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### 3.3 プレビューサーバー管理

```rust
struct PreviewServer {
    task_id: String,
    port: u16,
    child_process: Child,
    worktree_path: String,
}

// Tauri State で管理
struct PreviewState {
    servers: Mutex<HashMap<String, PreviewServer>>,
}
```

- **ポート割り当て**: 3100番台から動的に空きポートを検出（`TcpListener::bind("127.0.0.1:0")`）
- **起動コマンド**: プロジェクトの `package.json` の `dev` スクリプトを参照し、`PORT=<割り当てポート>` 環境変数付きで実行
- **停止**: タスクのマージ時 or ユーザーの明示的操作で `kill` シグナル送信

---

## 4. フロントエンド設計 (React)

### 4.1 タスクステータスの拡張

```typescript
// src/types/index.ts
// 変更前: 'To Do' | 'In Progress' | 'Done'
// 変更後:
type TaskStatus = 'To Do' | 'In Progress' | 'Review' | 'Done';
```

### 4.2 カンバンボードの4列化

#### `StatusColumn.tsx` の変更

```typescript
// 新しいカラム定義
const columnConfig = {
  'To Do':       { label: '未着手',    bg: 'slate-50'   },
  'In Progress': { label: '進行中',    bg: 'blue-50'    },
  'Review':      { label: 'レビュー',  bg: 'amber-50'   },
  'Done':        { label: '完了',      bg: 'emerald-50' },
};
```

#### `StorySwimlane.tsx` の変更

3列から4列レイアウトへ拡張（`grid-cols-3` → `grid-cols-4`）。

### 4.3 Review列専用カードUI

`TaskCard.tsx` にReview状態専用のアクションボタンを追加する。

```tsx
{task.status === 'Review' && (
  <div className="flex gap-2 mt-2">
    <button onClick={handleStartPreview}>
      ▶️ プレビュー起動
    </button>
    <button onClick={handleApproveAndMerge}>
      ✅ 承認してマージ
    </button>
  </div>
)}
```

#### プレビュー起動フロー
1. ボタン押下 → `invoke('start_preview_server', { projectPath, taskId, command, port })`
2. サーバー起動完了 → ポート番号を受け取る
3. `invoke('open_preview_in_browser', { port })` でブラウザオープン
4. カード上にプレビュー中インジケーター表示 + 停止ボタン

#### 承認・マージフロー
1. ボタン押下 → 確認ダイアログ表示
2. `invoke('merge_worktree', { projectPath, taskId })`
3. 成功 → タスクステータスを `Done` に更新、カードが Done 列に移動
4. コンフリクト → エラーダイアログで競合ファイル一覧を表示

### 4.4 ステータス遷移の自動化

`claude_runner.rs` の `claude_cli_exit` イベント処理を拡張：
- エージェント正常完了時、タスクステータスを `In Progress` → `Review` に自動遷移
- フロントエンド側でイベントを受信し、カンバンを即時更新

---

## 5. コンフリクト発生時のエラーハンドリング

### 5.1 方針

コンフリクトは「例外」ではなく「想定されるケース」として扱う。

### 5.2 フロー

```
マージ試行
    │
    ├── 成功 → Done へ移動、クリーンアップ
    │
    └── コンフリクト発生
            │
            ▼
        ┌───────────────────────────┐
        │ 1. git merge --abort       │
        │ 2. ステータスを Review に維持│
        │ 3. UIにエラー表示           │
        │    - 競合ファイル一覧       │
        │    - 推奨アクション         │
        └───────────┬───────────────┘
                    │
                    ▼
        ユーザーの選択肢:
        ├── A. 手動解決: ターミナルDockでgit操作
        ├── B. AI再実行: タスクをIn Progressに戻し、
        │      コンフリクト情報を含むプロンプトで再実行
        └── C. ワークツリー破棄: 変更を捨ててクリーンアップ
```

### 5.3 UI表現

コンフリクト状態のタスクカードには以下を表示:
- 赤色のコンフリクトバッジ
- 競合ファイル一覧（折りたたみ可能）
- 3つのアクションボタン（手動解決 / AI再実行 / 破棄）

---

## 6. node_modules（依存関係）の効率的な扱い

### 6.1 課題

各ワークツリーに独立した `node_modules` があると、ディスク容量とインストール時間が大幅に増加する。

### 6.2 解決策: 段階的アプローチ

#### Phase 1（本Epic）: シンプルな共有戦略

- **symlink方式**: ワークツリー生成時に、メインプロジェクトの `node_modules` へのシンボリックリンクを作成
  ```bash
  ln -s <project-root>/node_modules <worktree-path>/node_modules
  ```
- **利点**: ゼロコストでディスク使用量を抑制
- **制限**: ワークツリー内で `package.json` を変更するタスクには非対応
- **フォールバック**: `package.json` の差分を検出した場合のみ、`npm install` を実行

#### Phase 2（将来）: pnpm / npm workspaces

- `pnpm` のハードリンク最適化を活用し、共有ストアから依存関係を配布
- 各ワークツリーが独立した `node_modules` を持ちつつ、ディスク使用量を最小化

### 6.3 実装の詳細

```rust
async fn setup_worktree_dependencies(
    project_path: &str,
    worktree_path: &str,
) -> Result<()> {
    let main_node_modules = Path::new(project_path).join("node_modules");
    let wt_node_modules = Path::new(worktree_path).join("node_modules");

    if main_node_modules.exists() && !wt_node_modules.exists() {
        // シンボリックリンク作成
        #[cfg(unix)]
        std::os::unix::fs::symlink(&main_node_modules, &wt_node_modules)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&main_node_modules, &wt_node_modules)?;
    }

    Ok(())
}
```

---

## 7. テスト方針

### 7.1 バックエンド (Rust)

| テスト種別 | 対象 | 内容 |
|---|---|---|
| ユニットテスト | `worktree.rs` | worktree生成・削除・マージの各関数を一時ディレクトリで検証 |
| ユニットテスト | `worktree.rs` | コンフリクト発生時の`merge --abort`と状態復帰を検証 |
| ユニットテスト | `worktree.rs` | プレビューサーバーの起動・停止・ポート割り当てを検証 |
| 統合テスト | `claude_runner.rs` | worktree生成→Claude実行→ステータス遷移の一連フローを検証 |
| 統合テスト | DB migration | v14マイグレーションの適用と`Review`ステータスの永続化を検証 |

### 7.2 フロントエンド (React)

| テスト種別 | 対象 | 内容 |
|---|---|---|
| コンポーネントテスト | `StatusColumn` | 4列表示、Review列の正しいスタイリング |
| コンポーネントテスト | `TaskCard` | Review状態でのアクションボタン表示/非表示 |
| インタラクションテスト | `Board` | ドラッグ&ドロップで4列間の移動が正常に動作 |
| E2Eテスト | 全体フロー | タスク実行→Review遷移→プレビュー→マージ→Done遷移 |

### 7.3 手動テスト項目

- [ ] 2つのタスクを同時実行し、ワークツリーが独立して生成されること
- [ ] マージ成功時にワークツリーとブランチが完全に削除されること
- [ ] 意図的にコンフリクトを発生させ、エラーUIが正しく表示されること
- [ ] プレビューサーバーが正しいポートで起動し、ブラウザで確認できること
- [ ] アプリ終了時に起動中のプレビューサーバーが適切にクリーンアップされること

---

## 8. リスクと対策

| リスク | 影響度 | 対策 |
|---|---|---|
| Git未インストール環境 | 高 | 起動時にgitコマンドの存在チェック。未インストール時はエラーメッセージ表示 |
| mainブランチにpushなしでworktreeが古くなる | 中 | worktree生成前にmainの最新性を警告表示 |
| ワークツリーのゴミが残る（異常終了時） | 中 | アプリ起動時にorphanedワークツリーを検出・クリーンアップ |
| Windows環境でのsymlink権限 | 中 | Windowsでは`junction`を使用、または`npm install`にフォールバック |
| 大量のワークツリーによるディスク圧迫 | 低 | 最大同時ワークツリー数の上限設定（デフォルト5） |

---

## 9. スコープ外（将来対応）

- リモートリポジトリ（GitHub等）への自動push/PR作成
- pnpmベースの依存関係最適化
- ワークツリー間のファイル差分のインラインプレビュー（Diff Viewer）
- ブランチ戦略の設定UI（トランクベース以外の対応）
