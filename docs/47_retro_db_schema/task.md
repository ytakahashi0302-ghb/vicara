# EPIC47: レトロスペクティブ DBスキーマ + バックエンドCRUD

## 背景

レトロスペクティブ機能を実装するにあたり、データ永続化の基盤が必要である。KPTアイテム、レトロセッション、POノート、ルールの4つのドメインモデルを定義し、バックエンドのCRUD操作を先に整備することで、後続のUI・AI連携EPICが安全に進められる。

## ゴール

- レトロスペクティブに必要な全テーブルのマイグレーションを作成する
- Rust側のstruct定義とTauriコマンド（CRUD）を実装する
- スプリント完了時にdraftレトロセッションを自動作成する仕組みを組み込む
- タスクテーブルに実行ログサマリ列を追加する

## スコープ

### 含む

- `src-tauri/migrations/18_retrospective_notes.sql` の作成（4テーブル + ALTER TABLE）
- `src-tauri/src/db.rs` にstruct定義 + CRUDコマンド追加
- `src-tauri/src/lib.rs` にマイグレーション登録 + invoke_handler登録
- `complete_sprint` 処理内でのdraftレトロセッション自動生成

### 含まない

- フロントエンドUI（EPIC48で実装）
- TypeScript型定義（EPIC48で実装）
- SM AI合成ロジック（EPIC51で実装）
- POアシスタントのAI Tool連携（EPIC52で実装）

## タスクリスト

### Story 1: マイグレーション作成

- [x] `retro_sessions` テーブル作成（id, project_id, sprint_id, status, summary, timestamps）
- [x] `retro_items` テーブル作成（id, retro_session_id, category, content, source, is_approved, sort_order）
- [x] `retro_rules` テーブル作成（id, project_id, retro_item_id, content, is_active）
- [x] `project_notes` テーブル作成（id, project_id, sprint_id, title, content, source）
- [x] `tasks` テーブルに `execution_log_summary TEXT` カラムを追加
- [x] インデックス作成（retro_sessions_project_sprint, retro_items_session, retro_rules_project_active, project_notes_project）

### Story 2: Rust struct定義 + CRUDコマンド

- [x] `RetroSession`, `RetroItem`, `RetroRule`, `ProjectNote` の `FromRow` struct定義
- [x] retro_sessions: `get_retro_sessions`, `get_retro_session`, `create_retro_session`, `update_retro_session`
- [x] retro_items: `get_retro_items`, `add_retro_item`, `update_retro_item`, `delete_retro_item`, `approve_retro_item`
- [x] retro_rules: `get_retro_rules`, `add_retro_rule`, `update_retro_rule`, `delete_retro_rule`
- [x] project_notes: `get_project_notes`, `add_project_note`, `update_project_note`, `delete_project_note`

### Story 3: スプリント完了時の自動セッション作成

- [x] `complete_sprint` 処理内でdraft状態の `retro_session` を自動生成する
- [x] 重複作成防止（同一sprint_idで既存セッションがあればスキップ）

### Story 4: lib.rs登録

- [x] マイグレーション18の登録（L20-123のパターンに従う）
- [x] 全CRUDコマンドのinvoke_handler登録（L161-229のパターンに従う）

## 完了条件

- [x] マイグレーションが正常に適用され、全テーブルが作成される
- [x] `cargo test` が通る
- [x] `cargo build` がエラーなく完了する
- [x] 各CRUDコマンドが正しく動作する（insert/select/update/delete）
- [x] スプリント完了時にdraftレトロセッションが自動作成される
