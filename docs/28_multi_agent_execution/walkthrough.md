# Epic 28: 修正内容の確認

## 概要

Epic 27 で保存可能になっていた Team 設定を、実際の Claude Code CLI 実行基盤へ接続した。
今回の実装により、1 つのロール定義をテンプレートとして使いながら、複数タスクを複数の Claude CLI プロセスで安全に並行実行できるようになった。

主な成果:

- タスクごとの担当ロール割り当て
- role の `system_prompt` / `model` を使った Claude 実行
- `max_concurrent_agents` に基づく並行数ガード
- `task_id` 単位のセッション管理
- TerminalDock のタブ化と状態インジケーター
- 個別 task 単位の Kill 制御

## バックエンド

- `src-tauri/migrations/13_task_role_assignment.sql` を追加
- `tasks.assigned_role_id` を追加し、タスクと team role を紐付け可能にした
- `src-tauri/src/db.rs` を更新し、Task モデルと Task CRUD に `assigned_role_id` を追加
- `get_task_by_id`, `get_team_role_by_id`, `get_max_concurrent_agents_value` を追加
- `src-tauri/src/claude_runner.rs` を大幅に再設計
  - `ClaudeState` を `task_id -> session` の HashMap 管理に変更
  - `Starting` / `Running` 状態を持つセッション管理を導入
  - `max_concurrent_agents` 超過時の起動拒否
  - 同一 task の二重起動拒否
  - `claude_cli_started` イベントを追加
  - `get_active_claude_sessions` を追加
  - `kill_claude_process(task_id)` を task 単位 kill に変更
- role context と task 詳細は一時ファイルへ書き出す方式を採用した
- 一時ファイルは正常終了 / エラー終了 / timeout / 手動 kill のすべてで削除する

## フロントエンド

- `src/components/board/TaskFormModal.tsx`
  - Team 設定から role 一覧を取得し、担当ロールドロップダウンを追加
- `src/components/kanban/StorySwimlane.tsx`
  - 新規タスク作成時に `assigned_role_id` を保存するよう更新
- `src/components/kanban/TaskCard.tsx`
  - タスク更新時に `assigned_role_id` を保存するよう更新
  - `In Progress` / `Done` タスクの「開発を実行」を disabled 化
  - role 未設定時は起動前にガードするよう変更
- `src/components/terminal/TerminalDock.tsx`
  - 単一ログ表示からタブ式 UI へ変更
  - タスク単位でログバッファを分離
  - アクティブタブだけを xterm に描画する構成に変更
  - タブ切替時に `fitAddon.fit()` を呼び、xterm のサイズ崩れを防止
  - 非アクティブタブにも状態インジケーターを表示
  - 起動中セッションを `get_active_claude_sessions` で復元
  - Kill ボタンをアクティブタブの task にのみ紐付け

## Claude CLI 実行方式の変遷

### 当初案

- 一時ファイルを生成し、Claude CLI に `--file <temp_file_path>` を渡して実行する案を採用していた

### 実地テストで発覚した問題

- Claude Code CLI の `--file` はプロンプトファイル読み込みではなく、外部ファイルアタッチ機能として解釈された
- その結果、以下のエラーで即座に Failed した

```text
Error: Session token required for file downloads. CLAUDE_CODE_SESSION_ACCESS_TOKEN must be set.
```

### 最終対応

- `--file` オプションの使用を完全に廃止
- 一時ファイル生成ロジック自体は維持
- `-p` で一時ファイルのパスを Claude に伝え、AI 自身にファイルを読ませる方式へ変更

最終的な実行イメージ:

```text
claude -p "以下のファイルに記載された役割とタスク指示を読み込み、それに従って開発を実行してください。ファイルパス: <temp_file_path>" --model <role.model> --permission-mode bypassPermissions --add-dir <cwd> --verbose
```

この変更により、role context の外部化とデバッグ容易性を維持しつつ、Claude Code CLI の仕様に適合させた。

## アーキテクチャ上の重要な学び

実地テストにより、1 つの role 定義から複数のエージェントを同時に起動できることが確認できた。
この結果から、本システムにおける `role` は「実体の人数」ではなく「エージェント生成のテンプレート」であることが明確になった。

つまり:

- role 数 = 同時起動可能数 ではない
- `max_concurrent_agents` は role 数制約ではなく、CPU / API レート / UI 安全性のための全体上限である

この学びにより、Epic 27 で導入した `max_concurrent_agents <= roles.len()` 制約は、現在のアーキテクチャとは論理的に整合しないことが判明した。

## 確認結果

- `cargo check` 通過
- `npm run build` 通過
- `npx tsc --noEmit` 通過
- Rust 側の警告は 0 件
- 実地テストで以下を確認
  - `-p` 方式で Claude CLI が正常起動すること
  - 同一 role から複数タブを生成し、独立に並行稼働すること
  - TerminalDock のタブ UI と状態インジケーターが正しく動作すること
  - Failed / Running / Completed の切り替えがタブ上で視認できること

## 補足

- 現在の Team 設定 UI / DB バリデーションには、まだ `max_concurrent_agents <= roles.len()` の制約が残っている
- 実行基盤側は role 数に依存せず並行稼働できるため、次の Epic ではこの制約撤廃が最優先課題になる
