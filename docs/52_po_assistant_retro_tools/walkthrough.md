# EPIC52 実装ウォークスルー

## 変更ファイル一覧

| ファイル | 変更内容 |
|---|---|
| `src-tauri/src/ai_tools.rs` | `AddProjectNoteTool` / `SuggestRetroItemTool` の2ツール追加 |
| `src-tauri/src/rig_provider.rs` | Anthropic / Gemini 両パスで新ツール2つをエージェントに登録 |
| `src-tauri/src/ai.rs` | `PoAction` struct追加 / `PoAssistantExecutionPlan` 拡張 / CLIプロンプト全面再設計 / APIプロンプト更新 |
| `src-tauri/src/db.rs` | `insert_story_with_tasks` の sequence_number 採番バグ修正 |
| `src/components/ai/NotesPanel.tsx` | `kanban-updated` リスナー追加 / ℹ ツールチップ追加 |
| `src/components/kanban/RetrospectiveView.tsx` | `kanban-updated` リスナー追加 |
| `src/components/ai/PoAssistantSidebar.tsx` | ヘッダー説明文更新 / ℹ ツールチップ追加 |
| `src/App.tsx` | サイドバーボタンラベル変更 |

---

## Story 1-2: AI Tool 実装（`ai_tools.rs`）

### AddProjectNoteTool

- **Args**: `title`, `content`, `sprint_id`（Optional）
- **description**: 「会話中の気づきを『ふせん』として残す」ニュアンスを含む。PBI作成依頼では使わない旨を明記
- DB関数 `db::add_project_note` を `source="po_assistant"` で呼び出し
- 成功時に `kanban-updated` イベントを emit

### SuggestRetroItemTool

- **Args**: `category`（keep/problem/try）, `content`
- `db::get_retro_sessions` でアクティブセッション（`draft` or `in_progress`）を検索
- 存在しない場合は `CustomToolError` でAIに差し戻し → ユーザーへレトロ開始を案内
- DB関数 `db::add_retro_item` を `source="po"` で呼び出し
- 成功時に `kanban-updated` イベントを emit

---

## Story 3: APIツール登録（`rig_provider.rs`）

- `chat_team_leader_with_tools` 関数内の Anthropic / Gemini 両パスで `.tool(note_tool).tool(retro_tool)` を追加
- 既存の `CreateStoryAndTasksTool` と同様のパターン

---

## Story 4: CLIマルチツールルーティング（`ai.rs`）

### 背景

CLIはAPIと異なり tool_use プロトコルがないため、JSON フィールド `action` でルーティングする**アクション配列方式**を採用。

### 新JSONスキーマ

```json
{
  "reply": "ユーザーへのメッセージ",
  "actions": [
    { "action": "create_story", "payload": { "story_title": "...", "tasks": [...] } },
    { "action": "add_note",     "payload": { "title": "...", "content": "..." } },
    { "action": "suggest_retro","payload": { "category": "try", "content": "..." } }
  ]
}
```

複数アクションを1レスポンスで同時実行可能。

### 後方互換戦略

旧フォーマット（`operations` 配列）は `#[serde(default)]` で維持。
`apply_team_leader_execution_plan` が両フォーマットを処理：

- `actions` が非空 → アクション種別ルーティングを実行して早期リターン
- `actions` が空 / `operations` が非空 → 旧ロジックにフォールスルー

フォールバックAPI経路（`execute_fallback_team_leader_plan`）や `execute_contextual_cli_backlog_plan` は旧フォーマットのまま動作継続。

### suggest_retro のレトロセッション未存在時の挙動

CLIルーティング側では、レトロセッションが存在しない場合でも**エラーで落とさず**、`action_results` に「スキップしました」メッセージを追加してフローを継続する。
（API Toolパスでは `CustomToolError` でAIに差し戻す設計を維持）

---

## Story 5: バグ修正

### バグ1: sequence_number 採番漏れ（`db.rs`）

**原因**: `insert_story_with_tasks`（POアシスタントが使うバルク登録関数）の stories / tasks INSERT 文に `sequence_number` が欠落していた。通常の `add_story` / `add_task` は `next_project_sequence_number` を呼んで採番しているが、この関数だけ省略されていた。

**修正**: トランザクション内サブクエリで採番するよう変更。SQLite では同一トランザクション内のINSERTが即時可視なため、複数タスクを連続挿入しても正しくインクリメントされる。

```sql
-- stories
INSERT INTO stories (id, project_id, sequence_number, ...)
VALUES (?, ?, (SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM stories WHERE project_id = ?), ...)

-- tasks
INSERT INTO tasks (id, project_id, story_id, sequence_number, ...)
VALUES (?, ?, ?, (SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM tasks WHERE project_id = ?), ...)
```

### バグ2: create_story アクションで kanban-updated が emit されない（`ai.rs`）

**原因**: `apply_team_leader_execution_plan` の `create_story` ブランチで `app.emit("kanban-updated", ())` が漏れていた。`suggest_retro` / `add_note` が先に emit → UIリフレッシュ → その後 `create_story` が DB 挿入するが emit なし → PBI が表示されない。

**修正**: `insert_story_with_tasks` 直後に `app.emit("kanban-updated", ())` を追加。

### バグ3: NotesPanel / RetrospectiveView が kanban-updated を購読していない（フロントエンド）

**原因**: `kanban-updated` は `ScrumContext` のみが受信しており、stories/tasks/sprints/dependencies を再取得するが、`useProjectNotes`（ふせん）と `useRetrospective`（KPT）は購読していなかった。POアシスタントがふせんやレトロアイテムを追加しても、パネルを開き直すまで反映されない。

**修正**:

```tsx
// NotesPanel.tsx
useEffect(() => {
    const unlisten = listen('kanban-updated', () => {
        void fetchNotes();
        void fetchSessions();
    });
    return () => { void unlisten.then((fn) => fn()); };
}, [fetchNotes, fetchSessions]);

// RetrospectiveView.tsx
useEffect(() => {
    const sessionId = currentSession?.id ?? null;
    const unlisten = listen('kanban-updated', () => {
        void fetchItems(sessionId);
    });
    return () => { void unlisten.then((fn) => fn()); };
}, [currentSession?.id, fetchItems]);
```

---

## Story 6: プロンプト品質改善

### 問題: レトロ依頼で不要なPBI作成が発生

「次のTRYとして〜」のようなレトロ専用の依頼に対し、AIが自己判断で `create_story` アクションも発行し、意図しないPBIが作成・成功メッセージが表示されていた。

**修正方針**: `create_story` / `create_story_and_tasks` の使用条件を「ユーザーが明示的にバックログ追加を求めた場合のみ」と明示。以下の例を禁止ケースとして列挙：

- 「次のTRYとして〜」
- 「レトロに追加して」
- 「改善提案として〜」

### 問題: ふせん追加依頼でふせんが過剰に生成される

「PBIに追加して」のようなバックログ作成依頼で `add_project_note` も同時に呼ばれていた。

**修正**: `AddProjectNoteTool` の description と API / CLI 両プロンプトに「明示的なPBI/タスク作成依頼では絶対に使わない」制約を追加。

### 用語統一: 「ストーリー」→「PBI」

ユーザー向け文言（プロンプト・固定メッセージ）を「ストーリー」から「PBI（プロダクトバックログアイテム）」に統一。内部コード変数名（`story_title` 等）は変更なし。

---

## Story 7: UIブラッシュアップ

| 変更箇所 | 変更前 | 変更後 |
|---|---|---|
| サイドバー開閉ボタン（`App.tsx`） | `PO アシスタント` | `PO アシスタント / ふせん` |
| ヘッダー説明文（`PoAssistantSidebar.tsx`） | 意思決定サポートとバックログ整理を担当 | 意思決定サポート・バックログ整理・レトロスペクティブ連携を担当 |
| POアシスタント名横 ℹ アイコン | なし | ホバーでできること4項目をツールチップ表示 |
| ふせんタブ ℹ アイコン | なし | ホバーでふせん説明3項目をツールチップ表示（タブクリックと干渉しないよう stopPropagation 済み） |

ツールチップは Tailwind の `group-hover:opacity-100` を使ったCSS純正実装（外部ライブラリ追加なし）。

---

## 最終検証結果

- `cargo test`: **全92テスト通過**（回帰なし）
- `npm run build`: TypeScript型チェック + Viteビルド成功（チャンクサイズ警告は既知・動作影響なし）
