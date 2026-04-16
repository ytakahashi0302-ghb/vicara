# EPIC52: POアシスタント レトロ連携ツール

## 背景

POアシスタントとの会話の中で、レトロスペクティブに関連するアイデアや気づきが生まれることがある。POアシスタントがこれらを検知し、「レトロに追加しますか？」と提案したり、直接ノートを作成したりできる機能を追加する。既存の `CreateStoryAndTasksTool` パターンに倣い、AI Toolとして実装する。

## ゴール

- POアシスタントがプロジェクトノートを自動作成できるAI Toolを追加する
- POアシスタントがレトロアイテムの追加を提案できるAI Toolを追加する
- POアシスタントのシステムプロンプトにレトロ関連の指示を追加する
- CLIモードでもふせん・KPT追加が動作するようにする

## スコープ

### 含む

- `src-tauri/src/ai_tools.rs` に `AddProjectNoteTool` 追加
- `src-tauri/src/ai_tools.rs` に `SuggestRetroItemTool` 追加
- `src-tauri/src/ai.rs` のPOアシスタントツールレジストリへの登録
- POアシスタントシステムプロンプトへのレトロ関連指示追加
- CLIモードのJSONスキーマをマルチアクション対応に再設計
- UIの画面更新漏れ修正（NotesPanel / RetrospectiveView）
- バグ修正・プロンプト品質改善

### 含まない

- SMエージェント機能（EPIC51で実装済み前提）
- NotesPanel UI（EPIC49で実装済み前提）
- RetrospectiveView UI（EPIC48で実装済み前提）

---

## タスクリスト

### Story 1: AddProjectNoteTool

- [x] `AddProjectNoteArgs` struct定義（title, content, sprint_id）
- [x] `AddProjectNoteTool` struct定義（app: AppHandle, project_id: String）
- [x] `Tool` trait実装（definition, call）
- [x] ノート追加成功時のフロントエンドイベント通知（`kanban-updated`）

### Story 2: SuggestRetroItemTool

- [x] `SuggestRetroItemArgs` struct定義（category, content）
- [x] `SuggestRetroItemTool` struct定義（app: AppHandle, project_id: String）
- [x] `Tool` trait実装（definition, call）
- [x] アクティブなレトロセッションが存在しない場合のエラーハンドリング

### Story 3: APIツール登録 + プロンプト更新

- [x] `rig_provider.rs` のAnthropicパスに `AddProjectNoteTool` / `SuggestRetroItemTool` を登録
- [x] `rig_provider.rs` のGeminiパスに同2ツールを登録
- [x] POアシスタントのAPIシステムプロンプトにレトロ連携指示を追加:
  - 会話中にプロセス改善や問題点に気づいたらふせん作成を提案する
  - レトロセッションがアクティブな場合はKPTアイテム追加を提案する
  - 改善提案はTry、良かった点はKeep、問題点はProblemとして分類する

### Story 4: CLIマルチツールルーティング（追加要件）

- [x] `PoAction` struct を `ai.rs` に追加（action: String, payload: serde_json::Value）
- [x] `PoAssistantExecutionPlan` に `actions: Vec<PoAction>` フィールドを追加（後方互換: `#[serde(default)]`）
- [x] CLI用システムプロンプトのJSON出力スキーマを、複数アクション（`create_story` / `add_note` / `suggest_retro`）に対応できる構造に再設計
- [x] `apply_team_leader_execution_plan` にアクション種別ルーティングを実装
- [x] `create_story` / `add_note` / `suggest_retro` 各ブランチで `kanban-updated` を emit

### Story 5: バグ修正

- [x] `insert_story_with_tasks` の stories/tasks INSERT に `sequence_number` が欠落していたバグを修正（サブクエリ採番に変更）
- [x] `create_story` アクションルーティングで `kanban-updated` の emit が漏れていたバグを修正
- [x] `NotesPanel` が `kanban-updated` を購読しておらずPOアシスタント追加後に画面更新されない問題を修正
- [x] `RetrospectiveView` が `kanban-updated` を購読しておらずPOアシスタント追加後に画面更新されない問題を修正

### Story 6: プロンプト品質改善

- [x] `AddProjectNoteTool` の description に「PBI作成依頼では使わない」制約を明記
- [x] APIプロンプト・CLIプロンプトに `create_story` 使用条件を追加（明示的なバックログ追加依頼のみ使用）
- [x] 「ストーリー」→「PBI」表記統一（ユーザー向け文言）

### Story 7: UIブラッシュアップ

- [x] ヘッダー説明文を「意思決定サポート・バックログ整理・レトロスペクティブ連携を担当」に更新
- [x] POアシスタント名横に ℹ アイコン + ホバーツールチップ（できること4項目）を追加
- [x] ふせんタブに ℹ アイコン + ホバーツールチップ（説明3項目）を追加
- [x] サイドバー開閉ボタンのラベルを「PO アシスタント / ふせん」に変更

---

## 完了条件

- [x] POアシスタントが会話中にふせんを自動作成できる（API・CLIどちらのモードでも）
- [x] POアシスタントがレトロアイテムの追加を提案・実行できる（API・CLIどちらのモードでも）
- [x] ふせん/レトロアイテム/PBI作成時にUIが自動更新される
- [x] PBIやタスクに sequence_number が正しく採番される
- [x] `cargo test` が通る（92テスト全通過）
- [x] `npm run build` がエラーなく完了する
