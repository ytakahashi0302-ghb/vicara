# 16_context_aware_ai: タスクリスト

## 1. フロントエンドの改修
- [x] `src/components/ai/IdeaRefinementDrawer.tsx` で `WorkspaceContext` から `currentProjectId` を取得し、`invoke('refine_idea', { ... })` の引数に追加する。
- [x] `src/components/kanban/StorySwimlane.tsx` で `WorkspaceContext` から `currentProjectId` を取得し、`invoke('generate_tasks_from_story', { ... })` の引数に追加する。

## 2. バックエンド（Rust/SQLite）の改修
- [x] `src-tauri/src/ai.rs` の `refine_idea` コマンドの引数に `projectId: String` を追加する。
- [x] `src-tauri/src/ai.rs` の `generate_tasks_from_story` コマンドの引数に `projectId: String` を追加する。
- [x] `src-tauri/src/db.rs` または `ai.rs` 内に、指定された `project_id` に属する `archived = 0` の Story一覧 と Task一覧 を取得するDBアクセス用関数を実装する。

## 3. コンテキスト結合（簡易RAG）の実装
- [x] 取得したプロジェクトの Story/Task データを Markdown（またはJSON）テキスト形式に整形する処理を実装する。
- [x] 整形されたテキストを「現在のプロジェクトコンテキスト」として、`refine_idea` と `generate_tasks_from_story` の実行時にLLMのシステムプロンプト（System Instruction）の末尾に結合して送信する。

## 4. 動作確認・ドキュメント作成
- [x] 実際にLLMが既存のストーリーやタスクを加味した提案を行えるかテストする。
- [x] `walkthrough.md` を作成してPOにレビュー依頼を出す。
