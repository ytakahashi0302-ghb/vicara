# Vicara — Epic 52 完了・引き継ぎ書

> 作成日: 2026-04-17
> 対象: Epic 53 担当の次の開発者・次セッションの Claude

---

## 1. Epic 52 で完成したこと

### 概要

「POアシスタント レトロ連携ツール」として、POアシスタントが会話中に自律的に以下の3つのアクションを実行できる基盤を構築した。

| アクション | 説明 | 対応モード |
|---|---|---|
| `add_project_note` | 気づきを「ふせん」としてボードに記録 | API ツール呼び出し + CLI JSON ルーティング |
| `suggest_retro_item` | KPT アイテム（Keep/Problem/Try）をレトロボードに追加 | API ツール呼び出し + CLI JSON ルーティング |
| `create_story_and_tasks` | PBI・タスクをバックログに登録（既存） | API ツール呼び出し + CLI JSON ルーティング |

### 実装ファイル一覧

| ファイル | 変更内容 |
|---|---|
| `src-tauri/src/ai_tools.rs` | `AddProjectNoteTool` / `SuggestRetroItemTool` 追加 |
| `src-tauri/src/rig_provider.rs` | Anthropic / Gemini 両パスでツール登録 |
| `src-tauri/src/ai.rs` | CLIマルチアクションJSON設計 / ルーティング / プロンプト更新 |
| `src-tauri/src/db.rs` | `insert_story_with_tasks` の sequence_number 採番バグ修正 |
| `src/components/ai/NotesPanel.tsx` | `kanban-updated` リスナー追加・ツールチップ |
| `src/components/kanban/RetrospectiveView.tsx` | `kanban-updated` リスナー追加 |
| `src/components/ai/PoAssistantSidebar.tsx` | 説明文更新・ツールチップ追加 |
| `src/App.tsx` | サイドバーボタンラベル変更 |

---

## 2. 重要な設計・注意点（Epic 53 で参照すべき情報）

### CLIマルチアクション JSON スキーマ

CLIモードは API の tool_use プロトコルが使えないため、独自の JSON ルーティング方式を採用している。

```json
{
  "reply": "ユーザーへのメッセージ",
  "actions": [
    { "action": "create_story",  "payload": { ...CreateStoryAndTasksArgs... } },
    { "action": "add_note",      "payload": { "title": "...", "content": "...", "sprint_id": null } },
    { "action": "suggest_retro", "payload": { "category": "try", "content": "..." } }
  ]
}
```

`apply_team_leader_execution_plan`（`ai.rs`）が `actions` 配列を走査してルーティングする。旧フォーマット（`operations` 配列）は `#[serde(default)]` で後方互換を維持。

**Epic 53 で新アクションを追加する場合**、同関数の `match action.action.as_str()` に新ブランチを追加し、CLIプロンプトにアクション定義を追記するだけで拡張できる。

### kanban-updated イベントの購読状況

POアシスタントの全アクション完了後に `app.emit("kanban-updated", ())` が必ず発火する。フロントエンドでは以下が購読済み：

| コンポーネント / コンテキスト | 購読 | 再取得内容 |
|---|---|---|
| `ScrumContext` | ✅ | stories / tasks / sprints / dependencies |
| `NotesPanel` | ✅（Epic 52 で追加） | notes / retro sessions |
| `RetrospectiveView` | ✅（Epic 52 で追加） | retro items |

**新しいデータ種別を追加した場合は、対応するコンポーネントで `kanban-updated` の購読を追加すること。**

### retro_items のソース種別

`retro_items.source` カラムの有効値（`VALID_RETRO_ITEM_SOURCES`）:

```
"agent" | "po" | "sm" | "user"
```

POアシスタントが追加したアイテムは `source = "po"` で登録される。
SMエージェントが追加したアイテムは `source = "sm"`（Epic 51 で実装済み）。

### project_notes のソース種別

`project_notes.source` カラムの有効値（`VALID_PROJECT_NOTE_SOURCES`）:

```
"user" | "po_assistant"
```

POアシスタントが追加したノートは `source = "po_assistant"` で登録される。

---

## 3. Epic 53 実装ガイド（Try → ルール パイプライン）

### 概要

レトロで承認された `Try` アイテムを**永続的な「ルール」として DB に保存**し、DEV エージェントのタスク実行プロンプトへ自動注入する Vicara の核心パイプライン。

### DB

`retro_rules` テーブルはマイグレーション 7 (`7_scrum_foundation.sql`) で既に定義済み。
Tauri コマンドも `db::get_retro_rules` / `add_retro_rule` / `update_retro_rule` / `delete_retro_rule` が実装済み。

### 実装対象ファイル

| ファイル | 実装内容 |
|---|---|
| `src/components/kanban/RetrospectiveView.tsx` | Try カラムに「ルール化」ボタンを追加。承認済み Try アイテムをクリック1つで `add_retro_rule` に連携 |
| `src-tauri/src/claude_runner.rs` | `build_task_prompt` 関数に「承認済みルール」セクションを追加。`get_retro_rules` で取得したルールを注入 |
| `src/components/ui/settings/` | ルール管理 UI（一覧・編集・削除）の追加 |

### 既存の承認済み Try 取得コマンド

`get_approved_try_items` コマンドが既に実装済み（Epic 51 で追加）。
`RetrospectiveView.tsx` の `useEffect` 内で呼ばれており、`approvedTryItems` ステートに格納されている。

### 実装の流れ

1. `RetrospectiveView` の Try カラムに「📌 ルール化」ボタンを追加
2. ボタン押下 → `add_retro_rule` を呼び出し → `kanban-updated` emit → UI 更新
3. `claude_runner.rs` の `build_task_prompt` でルールを取得・注入:
   ```rust
   let rules = crate::db::get_retro_rules(&app, &project_id).await?;
   // プロンプト末尾に「## チームルール」セクションとして追記
   ```
4. 設定画面にルール管理 UI を追加（表示・削除のみでも可）

---

## 4. 既知の TODO

| 場所 | 内容 |
|---|---|
| `db.rs` `get_agent_retro_runs_by_sprint_and_role` | `role_name` 文字列フィルタ → 将来 `role_id` ベースに移行 |
| `insert_story_with_tasks` | sequence_number 採番のユニットテストが未実装（spawn_task として提起済み） |
| チャンクサイズ警告 | `npm run build` で 987KB チャンク警告。動作影響なし。将来的に dynamic import で分割を検討 |

---

## 5. 開発コマンド

```bash
# フロントエンドビルド（型チェック + Vite）
npm run build

# Rust ユニットテスト
cd src-tauri && cargo test

# 開発起動（Tauri dev モード）
npm run tauri dev
```

### PowerShell 文字化け対策

```powershell
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8;
```
