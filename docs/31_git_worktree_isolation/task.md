# Epic 31: Git Worktree隔離 & 1-Clickレビュー — タスク分解

---

## Phase 1: バックエンド基盤 — Worktreeライフサイクル管理

### Task 1.1: `worktree.rs` モジュール新設 — Worktree生成・削除
- **対象ファイル**: `src-tauri/src/worktree.rs` (新規), `src-tauri/src/lib.rs`
- **内容**:
  - `create_worktree(project_path, task_id)` の実装
    - `.scrum-ai-worktrees/task-<ID>` ディレクトリへの `git worktree add`
    - `.gitignore` への `.scrum-ai-worktrees/` 自動追記（初回のみ）
    - ブランチ名: `feature/task-<ID>`
  - `remove_worktree(project_path, task_id)` の実装
    - `git worktree remove` + `git branch -d` による安全な削除
  - `get_worktree_status(project_path, task_id)` の実装
    - ワークツリーの存在確認、ブランチ状態の取得
  - Tauriコマンドとして登録（`lib.rs` に追加）
  - Gitコマンドの存在チェックユーティリティ
- **完了条件**: 一時ディレクトリでworktree生成→確認→削除が成功すること

### Task 1.2: 自動マージ機能の実装
- **対象ファイル**: `src-tauri/src/worktree.rs`
- **内容**:
  - `merge_worktree(project_path, task_id)` の実装
    - ワークツリー内の未コミット変更の自動コミット
    - `main` ブランチへの `git merge --no-ff`
    - 成功時: ワークツリー削除 + ブランチ削除
    - 失敗時: `git merge --abort` + コンフリクトファイル一覧の返却
  - `MergeResult` 列挙型の定義（`Success` / `Conflict { files }` / `Error`）
  - `get_worktree_diff(project_path, task_id)` の実装（main との差分取得）
- **完了条件**: 正常マージとコンフリクト時の安全なロールバックが動作すること

### Task 1.3: node_modules シンボリックリンク共有
- **対象ファイル**: `src-tauri/src/worktree.rs`
- **内容**:
  - ワークツリー生成時に `node_modules` への symlink を自動作成
  - Unix: `std::os::unix::fs::symlink`
  - Windows: `std::os::windows::fs::symlink_dir` (権限不足時は `junction` にフォールバック)
  - `package.json` 差分検出時の警告ログ出力
- **完了条件**: ワークツリー内で `node_modules` が正しく参照できること

### Task 1.4: 起動時のorphanedワークツリー検出・クリーンアップ
- **対象ファイル**: `src-tauri/src/worktree.rs`, `src-tauri/src/lib.rs`
- **内容**:
  - アプリ起動時に `git worktree list` を実行し、DBのworktreeレコードと照合
  - 不整合（DBに存在しない or ステータスが `removed`）のワークツリーを自動削除
  - 削除対象のログ出力
- **完了条件**: 異常終了後の再起動でゴミワークツリーが検出・削除されること

---

## Phase 2: DBスキーマ拡張 & ステータス遷移

### Task 2.1: DBマイグレーション v14 — Reviewステータス & worktreesテーブル
- **対象ファイル**: `src-tauri/src/lib.rs` (マイグレーション追加), `src-tauri/src/db.rs`
- **内容**:
  - タスクステータスの許容値に `'Review'` を追加
    - SQLiteのCHECK制約更新（テーブル再作成が必要な場合の対応）
  - `worktrees` テーブルの新設:
    ```sql
    CREATE TABLE IF NOT EXISTS worktrees (
        id TEXT PRIMARY KEY,
        task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
        project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        worktree_path TEXT NOT NULL,
        branch_name TEXT NOT NULL,
        preview_port INTEGER,
        preview_pid INTEGER,
        status TEXT NOT NULL DEFAULT 'active',
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now'))
    );
    ```
  - `db.rs` に worktrees テーブルの CRUD 関数を追加
- **完了条件**: マイグレーション適用後、Review ステータスのタスクが永続化できること

### Task 2.2: タスクステータス遷移ロジックの更新
- **対象ファイル**: `src-tauri/src/db.rs`, `src-tauri/src/claude_runner.rs`
- **内容**:
  - `update_task_status` でReviewステータスを受け付けるように修正
  - `claude_runner.rs`: エージェント正常完了時にステータスを `In Progress` → `Review` へ自動遷移
  - `claude_cli_exit` イベントのペイロードに `new_status: "Review"` を追加
- **完了条件**: エージェント完了後にタスクが自動的にReview状態になること

---

## Phase 3: プレビューサーバー管理

### Task 3.1: プレビューサーバーの起動・停止制御
- **対象ファイル**: `src-tauri/src/worktree.rs`
- **内容**:
  - `PreviewState`（Tauri Managed State）の実装
  - `start_preview_server(project_path, task_id, command)` の実装
    - 空きポートの動的検出（`TcpListener::bind("127.0.0.1:0")`）
    - ワークツリー内でdevサーバーをバックグラウンド起動
    - `PORT` 環境変数の設定
    - `worktrees` テーブルの `preview_port`, `preview_pid` を更新
  - `stop_preview_server(task_id)` の実装
    - プロセス終了 + DB更新
  - `open_preview_in_browser(port)` の実装
    - `tauri_plugin_opener` を使用してブラウザ起動
  - アプリ終了時の全プレビューサーバー自動停止
- **完了条件**: ワークツリー内のdevサーバーが別ポートで起動し、ブラウザで確認できること

---

## Phase 4: フロントエンド — Review列 & 1-ClickレビューUI

### Task 4.1: タスクステータス型の拡張
- **対象ファイル**: `src/types/index.ts`
- **内容**:
  - `TaskStatus` 型に `'Review'` を追加
  - ※ `frontend-core` モジュールの変更のため、最小限の修正にとどめる
- **完了条件**: TypeScript型チェックがパスすること

### Task 4.2: カンバンボードの4列化
- **対象ファイル**: `src/components/kanban/StatusColumn.tsx`, `src/components/kanban/StorySwimlane.tsx`, `src/components/kanban/Board.tsx`
- **内容**:
  - `StatusColumn.tsx`: Review列の定義追加（ラベル: "レビュー", 背景: amber-50）
  - `StorySwimlane.tsx`: 3列 → 4列グリッドレイアウトに変更
  - `Board.tsx`: ドラッグ&ドロップの対象列にReviewを追加
  - カラム順序: To Do → In Progress → Review → Done
- **完了条件**: カンバンボードにReview列が表示され、D&Dが正常動作すること

### Task 4.3: Review列専用タスクカードUIの実装
- **対象ファイル**: `src/components/kanban/TaskCard.tsx`
- **内容**:
  - Review状態のタスクカードに2つのアクションボタンを追加:
    - 「▶️ プレビュー起動」ボタン
      - `invoke('start_preview_server')` を呼び出し
      - 起動中はスピナー表示 → 完了後「プレビュー中」バッジ + 停止ボタン表示
    - 「✅ 承認してマージ」ボタン
      - 確認ダイアログ表示後、`invoke('merge_worktree')` を呼び出し
      - 成功: タスクを Done に移動
      - コンフリクト: エラーダイアログ（競合ファイル一覧 + アクション選択肢）
  - Review状態のカード視覚デザイン（amber系の強調ボーダー等）
- **完了条件**: ボタン押下でプレビュー起動・マージが実行され、UIが適切に更新されること

### Task 4.4: コンフリクト時のエラーUI
- **対象ファイル**: `src/components/kanban/TaskCard.tsx` (または新規モーダルコンポーネント)
- **内容**:
  - コンフリクト発生時のエラーダイアログ実装:
    - 競合ファイル一覧の表示
    - アクション選択肢:
      - A. 手動解決（ターミナルDockへ誘導）
      - B. AI再実行（コンフリクト情報をプロンプトに含めて再実行）
      - C. ワークツリー破棄（変更を捨ててクリーンアップ）
  - コンフリクト状態のタスクカードに赤色バッジ表示
- **完了条件**: コンフリクト時にユーザーが3つの選択肢から操作を選べること

---

## Phase 5: claude_runner.rs 統合 — Worktree内でのエージェント実行

### Task 5.1: `execute_claude_task` のWorktree統合
- **対象ファイル**: `src-tauri/src/claude_runner.rs`
- **内容**:
  - タスク実行前に `create_worktree` を呼び出し
  - Claude CLIの `cwd` と `--add-dir` をワークツリーパスに変更
  - 実行完了後のステータス遷移を `Review` に変更
  - ワークツリー情報をDBに記録
  - エラー時のワークツリークリーンアップ処理
- **完了条件**: エージェントがワークツリー内で実行され、完了後にReview状態になること

### Task 5.2: 並行実行時の競合防止
- **対象ファイル**: `src-tauri/src/claude_runner.rs`, `src-tauri/src/worktree.rs`
- **内容**:
  - 同一タスクIDでのworktree重複生成防止
  - 最大同時ワークツリー数の制限（設定可能、デフォルト5）
  - ワークツリー生成失敗時のグレースフルなエラーハンドリング
- **完了条件**: 2つのタスクが同時実行され、独立したワークツリーで並行稼働すること

---

## Phase 6: テスト & 品質保証

### Task 6.1: バックエンドユニットテスト
- **対象ファイル**: `src-tauri/src/worktree.rs` (テストモジュール)
- **内容**:
  - worktree生成・削除の正常系テスト
  - マージ成功・コンフリクトの各パターンテスト
  - symlink生成テスト
  - orphanedワークツリー検出テスト
- **完了条件**: `cargo test` で全テストがパスすること

### Task 6.2: フロントエンドコンポーネントテスト
- **内容**:
  - Review列の表示テスト
  - アクションボタンの表示/非表示テスト
  - ドラッグ&ドロップの4列対応テスト

### Task 6.3: 統合テスト & 手動検証
- **内容**:
  - エージェント実行→Review遷移→プレビュー→マージ→Done遷移の一連フロー
  - 2タスク同時実行での独立性確認
  - コンフリクト発生時のUI動作確認
  - Windows/macOS/Linux各環境での動作確認

---

## 依存関係グラフ

```
Task 1.1 (Worktree生成・削除)
    │
    ├──→ Task 1.2 (自動マージ)
    ├──→ Task 1.3 (node_modules symlink)
    └──→ Task 1.4 (orphanedクリーンアップ)

Task 2.1 (DBマイグレーション)
    │
    └──→ Task 2.2 (ステータス遷移)

Task 1.1 + Task 2.1
    │
    └──→ Task 5.1 (claude_runner統合)
             │
             └──→ Task 5.2 (並行実行)

Task 1.1
    │
    └──→ Task 3.1 (プレビューサーバー)

Task 2.1
    │
    └──→ Task 4.1 (型拡張)
             │
             └──→ Task 4.2 (4列化)
                      │
                      └──→ Task 4.3 (ReviewカードUI)
                               │
                               └──→ Task 4.4 (コンフリクトUI)

Task 5.1 + Task 4.3
    │
    └──→ Task 6.x (テスト)
```

## 推奨実装順序

1. **Task 1.1** → **Task 2.1** (並行可能: バックエンド基盤 + DB)
2. **Task 1.2** → **Task 1.3** (マージ + 依存関係)
3. **Task 4.1** → **Task 4.2** (フロントエンド基盤)
4. **Task 2.2** → **Task 5.1** (ステータス遷移 + claude_runner統合)
5. **Task 3.1** (プレビューサーバー)
6. **Task 4.3** → **Task 4.4** (ReviewカードUI + コンフリクトUI)
7. **Task 5.2** (並行実行制御)
8. **Task 1.4** (クリーンアップ)
9. **Task 6.1** → **Task 6.2** → **Task 6.3** (テスト)
