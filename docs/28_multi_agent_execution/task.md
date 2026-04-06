# Epic 28: マルチエージェント並行実行とタスクディスパッチ基盤

## 目的

Epic 27 で保存可能になった Team 設定（`max_concurrent_agents`, `team_roles`）を実行系に接続し、複数タスクを複数の Claude CLI プロセスで安全に並行実行できる土台を作る。

## スコープ

- タスクに担当ロールを割り当てる仕組みを追加する
- 担当ロールの `model` / `system_prompt` を Claude 実行時に注入する
- `max_concurrent_agents` を超えない並行実行制御を Rust 側で実装する
- 複数実行を識別できる TerminalDock UI を整備する
- 実行ボタン / 緊急停止ボタンの状態制御を厳密化する

## MVP 方針

- タスク起動時にキューイングはしない
- 上限超過時は即時エラーを返し、フロントで通知する
- 実行対象の特定は PID ではなく `task_id` を主キーにする
- プロンプト合成はフロントではなく Rust 側で行う
- タスクの担当ロールは `task` に保持し、実行時に DB から role 定義を再解決する

## 実装タスクリスト

- [x] Phase 1: タスクへの担当ロール割り当て用データモデルを追加する
- [x] Phase 1-1: `tasks` テーブルに `assigned_role_id` を追加する migration を作成する
- [x] Phase 1-2: Rust `Task` モデル、Tauri コマンド、TypeScript 型、hooks を更新する
- [x] Phase 1-3: タスク編集 UI に担当ロールドロップダウンを追加する
- [x] Phase 1-4: タスクカードに担当ロール表示を追加する

- [x] Phase 2: Claude 実行リクエストをバックエンド主導に再設計する
- [x] Phase 2-1: `execute_claude_task` が `task_id` から task / role / team_settings を解決するよう変更する
- [x] Phase 2-2: 一時ファイルに role context + task 詳細を書き出し、`-p` でファイルパスを渡して起動する
- [x] Phase 2-3: role ごとの `--model` 指定を追加する
- [x] Phase 2-4: 実行終了・kill・timeout 時に一時ファイルを必ず掃除する

- [x] Phase 3: マルチエージェント並行実行制御を Rust 側で実装する
- [x] Phase 3-1: `ClaudeState` を単一セッション保持から `task_id -> session` の管理に変更する
- [x] Phase 3-2: `max_concurrent_agents` を DB から取得して起動数を制限する
- [x] Phase 3-3: 同一タスクの二重起動を禁止する
- [x] Phase 3-4: `kill_claude_process(task_id)` が対象タスクだけを停止するよう変更する
- [x] Phase 3-5: `claude_cli_started` / `claude_cli_output` / `claude_cli_exit` と実行中一覧取得コマンドを追加する

- [x] Phase 4: フロントエンドの状態制御を厳密化する
- [x] Phase 4-1: Running / Done のタスクでは「開発を実行」を disabled または非表示にする
- [x] Phase 4-2: 実行中タスク集合をフロントで保持し、起動ボタンと停止ボタンに反映する
- [x] Phase 4-3: 再読み込み時にも状態復元できるよう、起動中一覧取得を初期化時に呼ぶ

- [x] Phase 5: TerminalDock をマルチプレックス対応に改修する
- [x] Phase 5-1: タスク単位のタブ UI を追加する
- [x] Phase 5-2: 各タブに `role name` / `task title` / 実行状態を表示する
- [x] Phase 5-2a: 非アクティブタブでも状態が分かるよう、タブタイトルに Running / Error / Done の視覚インジケーターを付ける
- [x] Phase 5-3: 停止ボタンはアクティブタブに紐づく task のみを停止する
- [x] Phase 5-4: 出力ログを task ごとに分離保持する

- [x] Phase 6: 検証
- [x] Phase 6-1: `cargo check`
- [x] Phase 6-2: `npm run build`
- [x] Phase 6-3: 手動確認（単体実行 / 2 並行実行 / 上限制御 / タブ切替 / 個別停止 / 完了時の状態更新）

## 非スコープ

- タスクの自動ロール推論
- 実行待ちキュー
- 再起動後のログ永続化
- 実行履歴の DB 保存
