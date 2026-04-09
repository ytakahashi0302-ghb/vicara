# Epic 36: Git ブロッキング修正 + CLI 検出基盤 タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: PO 承認済み
- 作成日: 2026-04-09

## 概要

Git 未インストール時にアプリ全体がブロックされる問題を修正し、複数 CLI ツールのインストール状況を検出する基盤コマンドを追加する。

## 実行順序

### 1. Git ブロッキングの解除
- [ ] `src/App.tsx` の Git 未インストール時フルスクリーンブロック（L366-408）を削除する。
- [ ] 代わりに、Git 未インストール時はアプリ上部にワーニングバナーを表示する。
- [ ] バナーには Git ダウンロードリンクと「Devエージェント機能には Git が必要です」の説明を含める。
- [ ] `src/context/WorkspaceContext.tsx` の `refreshGitStatus()` は引き続き保持する（状態は参照用として維持）。

### 2. CLI 検出コマンドの実装（バックエンド）
- [ ] `src-tauri/src/cli_detection.rs` を新規作成する。
- [ ] 以下の CLI のインストール状態を検出する関数を実装する:
  - `claude --version` (Claude Code CLI)
  - `gemini --version` (Gemini CLI)
  - `codex --version` (Codex CLI)
- [ ] Tauri コマンド `detect_installed_clis` を追加し、各 CLI の `{ name, installed, version }` を返却する。
- [ ] `src-tauri/src/lib.rs` にコマンドを登録する。

### 3. フロントエンド検出フックの追加
- [ ] `src/hooks/useCliDetection.ts` を新規作成する。
- [ ] `detect_installed_clis` を呼び出し、結果をキャッシュするカスタムフックを実装する。
- [ ] 手動リフレッシュ機能を提供する（CLI をインストール後に再検出）。

### 4. 動作確認
- [ ] Git 未インストール環境でアプリが正常に起動し、カンバン操作・PO アシスタントが利用できることを確認する。
- [ ] 各 CLI がインストール済み/未インストールの場合に検出コマンドが正しい結果を返すことを確認する。
