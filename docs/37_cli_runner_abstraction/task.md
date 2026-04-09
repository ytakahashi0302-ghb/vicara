# Epic 37: CLI Runner 抽象化レイヤー + DB 拡張 タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: Epic 36 完了
- 作成日: 2026-04-09

## 概要

現在 Claude Code CLI にハードコードされている `claude_runner.rs` を、複数 CLI に対応可能な抽象化レイヤーに再設計する。`CliRunner` trait を定義し、既存の Claude Code 実装を最初の具体実装として移行する。合わせて DB スキーマに `cli_type` カラムを追加する。

## 実行順序

### 1. CliRunner trait の定義
- [ ] `src-tauri/src/cli_runner/mod.rs` を新規作成する。
- [ ] `CliRunner` trait を定義する（`command_name`, `build_command`, `parse_version` メソッド）。
- [ ] CLI 種別を表す列挙型 `CliType` を定義する（`Claude`, `Gemini`, `Codex`）。
- [ ] `CliType` から対応する `CliRunner` 実装を返すファクトリ関数 `create_runner()` を実装する。

### 2. Claude Code Runner の実装
- [ ] `src-tauri/src/cli_runner/claude.rs` を新規作成する。
- [ ] `claude_runner.rs` の CLI コマンド構築部分（Windows: L548-575, Unix: L696-742）を `ClaudeRunner` 構造体に抽出する。
- [ ] `CliRunner` trait を `ClaudeRunner` に実装する。
- [ ] `claude_runner.rs` 内の `spawn_claude_process` 関数を、`CliRunner` trait 経由でコマンドを構築するように書き換える。

### 3. 共通実行ロジックの汎用化
- [ ] `claude_runner.rs` 内のセッション管理（`ClaudeState`, `reserve_session_slot`, `promote_session_to_running`）を CLI 非依存な命名に変更する（例: `AgentState`, `AgentSession`）。
- [ ] `execute_prompt_request` 関数を `CliRunner` を受け取る形に変更する。
- [ ] 出力ストリーミング（stdout/stderr → Tauri イベント emit）は共通ロジックとして維持する。
- [ ] タイムアウト・後処理ロジックも共通化する。

### 4. DB スキーマ拡張
- [ ] マイグレーション `XX_cli_type_support.sql` を作成する。
- [ ] `team_roles` テーブルに `cli_type TEXT NOT NULL DEFAULT 'claude'` カラムを追加する。
- [ ] `db.rs` の `TeamRole` / `TeamRoleInput` 構造体に `cli_type` フィールドを追加する。
- [ ] `get_team_configuration` / `save_team_configuration` のクエリに `cli_type` を含める。

### 5. フロントエンド型定義の更新
- [ ] `src/types/index.ts` の `TeamRoleSetting` に `cli_type: string` を追加する。

### 6. タスク実行フローの結合
- [ ] `execute_claude_task` コマンドを、ロールの `cli_type` に基づいて対応する `CliRunner` を選択するように変更する。
- [ ] 既存の Claude CLI タスク実行が変更後も正常に動作することを確認する。

### 7. 動作確認
- [ ] Claude Code CLI でのタスク実行が従来通り動作することを回帰テストする。
- [ ] DB マイグレーション適用後、既存ロールの `cli_type` が `'claude'` に設定されていることを確認する。
