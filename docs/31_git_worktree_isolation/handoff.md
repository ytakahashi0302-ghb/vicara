# Epic 31 Handoff

## 現在の前提

- Epic 31 の中核機能は実装済み。AI タスクは `main` 直下ではなく Git Worktree 上で実行される。
- Review フローは `In Progress` → `Review` → `Done` を基本経路とする。
- プレビューは技術スタックに応じて 2 系統ある。
  - 開発サーバー型: `npm run dev` または `npm run serve`
  - 静的サイト型: Worktree 内の `index.html` を直接開く
- Git が未インストールの環境では、フロントエンドが起動時に検知してブロッキング UI を表示する。

## バックエンドのモジュール分割

### `src-tauri/src/git.rs`

- Git CLI 呼び出しの低レイヤー責務を担当。
- 主な役割:
  - `run_git`, `run_git_raw`
  - `auto_commit_if_needed`
  - `parse_conflict_files`
  - `get_worktree_diff`
  - `ensure_git_repo`
  - `check_git_installed`（Tauri コマンド）
- `ensure_git_repo` はゼロ構成の中核。
  - `.git` が無ければ `git init -b main`
  - 既存ファイルを `Initial commit` としてコミット
  - 既に Git 管理下でもコミットが無ければ空コミットを作成
  - `main` ブランチの存在を保証

### `src-tauri/src/preview.rs`

- PreviewState とプレビュー用プロセス管理を担当。
- 主な役割:
  - `PreviewState`
  - `PreviewServerInfo`
  - `start_preview_for_task`
  - `open_preview_in_browser`
  - `open_local_path`
- Tauri コマンド自体は持たず、`worktree.rs` から利用される内部モジュール。

### `src-tauri/src/worktree.rs`

- Tauri コマンドの受付口と、Worktree ライフサイクルの高レイヤー制御を担当。
- 主な役割:
  - `create_worktree`
  - `remove_worktree`
  - `merge_worktree`
  - `get_worktree_status`
  - `get_worktree_diff`
  - `start_preview_server`
  - `stop_preview_server`
  - `open_preview_in_browser`
  - `open_static_preview`
- `create_worktree` の冒頭で `git::ensure_git_repo` を呼ぶため、ユーザーはフォルダ指定だけで AI 開発を始められる。

## フロントエンド仕様

### Review カード

- `Review` 列のタスクは専用カード UI を持つ。
- アクション:
  - プレビュー起動
  - 承認してマージ
  - 競合時の再実行 / 手動解決 / 破棄
- マージ確認は `await confirm(...)` 済みで、OK 時のみ実行される。

### プレビュー判定

- `TaskCard.tsx` が `ARCHITECTURE.md`、`package.json`、`index.html` を見てプレビュー方式を選ぶ。
- ルール:
  - `package.json` に `dev` / `serve` があればコマンド型
  - 純粋な静的サイトは `index.html` 直開き
  - 上記に当てはまらないスタックは未対応

### Git 未インストール時

- `WorkspaceContext` が起動時に `check_git_installed` を呼ぶ。
- Git 未導入なら `App.tsx` が全画面の警告 UI を表示し、通常操作を止める。
- UI から Git 公式サイトを開ける。

## 既知の残課題

- フロントエンドコンポーネントテストは未整備。
- Review フローの E2E とクロスプラットフォーム手動検証は未完。
- Git CLI 依存は残っているため、将来的には `git2-rs` かポータブルGit同梱での置換が望ましい。
