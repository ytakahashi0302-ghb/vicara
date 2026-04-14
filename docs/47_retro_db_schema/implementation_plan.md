# EPIC47 実装計画

## 概要

レトロスペクティブ機能の基盤となるDBスキーマとバックエンドCRUDを実装する。既存の `TeamChatMessage` や `Sprint` のパターン（`FromRow` derive + `#[tauri::command]` 関数 + `select_query` / `execute_query` ヘルパー）に厳密に従う。

## 現状整理

### 既存マイグレーション

- 最新: `17_cli_type_support.sql`
- 次の番号: `18`
- 登録先: `src-tauri/src/lib.rs` L20-123 の `migrations` 配列

### 既存CRUDパターン（db.rs）

```rust
// struct定義の例（TeamChatMessage相当）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RetroSession {
    pub id: String,
    pub project_id: String,
    pub sprint_id: String,
    pub status: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// コマンド定義の例
#[tauri::command]
pub async fn get_retro_sessions(app: AppHandle, project_id: String) -> Result<Vec<RetroSession>, String> {
    select_query::<RetroSession>(
        &app,
        "SELECT * FROM retro_sessions WHERE project_id = ? ORDER BY created_at DESC",
        vec![json!(project_id)],
    ).await
}
```

### スプリント完了フロー

`complete_sprint` コマンド（db.rs）内でスプリント状態を `Completed` に更新した後、draftレトロセッションを自動生成する。

## マイグレーションSQL

### `src-tauri/migrations/18_retrospective_notes.sql`

```sql
CREATE TABLE retro_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sprint_id TEXT NOT NULL REFERENCES sprints(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','in_progress','completed')),
    summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_retro_sessions_project_sprint ON retro_sessions(project_id, sprint_id);

CREATE TABLE retro_items (
    id TEXT PRIMARY KEY,
    retro_session_id TEXT NOT NULL REFERENCES retro_sessions(id) ON DELETE CASCADE,
    category TEXT NOT NULL CHECK (category IN ('keep','problem','try')),
    content TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('agent','po','sm','user')),
    source_role_id TEXT,
    is_approved INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_retro_items_session ON retro_items(retro_session_id);

CREATE TABLE retro_rules (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    retro_item_id TEXT REFERENCES retro_items(id) ON DELETE SET NULL,
    sprint_id TEXT REFERENCES sprints(id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_retro_rules_project_active ON retro_rules(project_id, is_active);

CREATE TABLE project_notes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sprint_id TEXT REFERENCES sprints(id) ON DELETE SET NULL,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT 'user' CHECK (source IN ('user','po_assistant')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_project_notes_project ON project_notes(project_id, created_at DESC);

ALTER TABLE tasks ADD COLUMN execution_log_summary TEXT;
```

## 実施ステップ

### Step 1: マイグレーションファイル作成

- `src-tauri/migrations/18_retrospective_notes.sql` を上記SQLで作成する

### Step 2: db.rs にstruct + CRUD追加

- `RetroSession`, `RetroItem`, `RetroRule`, `ProjectNote` を `FromRow` derive付きで定義する
- 各テーブルに対し get / add / update / delete の `#[tauri::command]` 関数を実装する
- `approve_retro_item` は `is_approved = 1` へのUPDATEを行う専用コマンドとする
- IDの生成にはuuid crateを使用（既存パターンに従う）

### Step 3: complete_sprint でdraftセッション自動生成

- `complete_sprint` コマンド内、status更新後に `retro_sessions` へINSERT
- 同一 `sprint_id` で既存セッションがないことを確認してから作成する
- 失敗してもスプリント完了自体はブロックしない（warn log + 続行）

### Step 4: lib.rs 登録

- マイグレーション18をmigrations配列に追加
- 全CRUDコマンドをinvoke_handler配列に追加

## リスクと対策

### リスク 1: ALTER TABLE の互換性

- SQLiteのALTER TABLE ADD COLUMNは既存データに影響しない（NULLデフォルト）
- `execution_log_summary` はNULLable TEXTなので安全

### リスク 2: 外部キー制約

- `retro_sessions.sprint_id` → `sprints.id` のCASCADE DELETEにより、スプリント削除時にレトロも消える
- これは期待動作（スプリント自体が消えればレトロも不要）

## テスト方針

### 自動テスト

- 各CRUDコマンドの基本動作テスト（insert → select → update → delete）
- `complete_sprint` 後にdraftレトロセッションが存在することを確認
- 重複呼び出し時にセッションが2つ作られないことを確認

### 手動確認

- アプリ起動後にマイグレーションが自動適用されることを確認
- devtools等からinvokeを呼び出し、CRUD操作が正しく動作することを確認

## 成果物

- `src-tauri/migrations/18_retrospective_notes.sql`（新規）
- `src-tauri/src/db.rs`（struct + CRUD追加）
- `src-tauri/src/lib.rs`（マイグレーション + コマンド登録）
