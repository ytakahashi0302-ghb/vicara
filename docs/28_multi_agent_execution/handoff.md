# Epic 28: 引き継ぎ書

## 現在の状態

- Epic 28 の実装は完了
- Team 設定を使った Claude CLI 実行基盤は接続済み
- タスク単位で担当ロールを割り当て、role ごとの model / system prompt を使って実行できる
- `task_id` ベースの HashMap 管理により、複数プロセスの並行実行が可能
- TerminalDock はタブ UI に改修済み
- 自動検証は `cargo check`、`npx tsc --noEmit`、`npm run build` まで完了
- 実地テストで複数タブの並行稼働を確認済み

## 重要な仕様

- `tasks.assigned_role_id` に担当ロールを保存する
- role は「物理人数」ではなく「エージェント生成テンプレート」として扱う
- `max_concurrent_agents` はシステム全体の同時実行上限であり、role 数と 1:1 対応しない
- 実行対象の識別子は PID ではなく `task_id`
- セッション状態は `Starting` / `Running` / `Completed` / `Failed` / `Killed` の考え方で UI に反映される

## Claude CLI 実行方式

- role context と task 詳細は一時ファイルへ書き出す
- ただし `--file` は使わない
- Claude CLI には `-p` で一時ファイルのパスを渡し、AI 自身にファイルを読ませる

実行イメージ:

```text
claude -p "以下のファイルに記載された役割とタスク指示を読み込み、それに従って開発を実行してください。ファイルパス: <temp_file_path>" --model <role.model> --permission-mode bypassPermissions --add-dir <cwd> --verbose
```

## 実地テストで確認できたこと

- 同一 role（例: Lead Engineer）から複数のエージェントを同時起動できる
- つまり role 数は同時起動可能数の上限ではない
- TerminalDock のタブ UI、状態インジケーター、task 単位 Kill は正常動作する

## 次の Epic / バックログの最優先課題

最優先で、`max_concurrent_agents <= roles.len()` の制約を撤廃すること。

理由:

- 現在のアーキテクチャでは role はテンプレートであり、同一 role から複数エージェントを並行起動できる
- そのため、role 数で並行数を制限するのは論理的に矛盾している
- `max_concurrent_agents` は純粋に「同時に何プロセスまで安全に動かすか」を表すべき

修正対象の候補:

- `src-tauri/src/db.rs`
  - `validate_team_configuration` 内の `config.max_concurrent_agents as usize > config.roles.len()` チェック削除
- `src/components/ui/GlobalSettingsModal.tsx`
  - `validateTeamConfiguration` 内の `config.max_concurrent_agents > config.roles.length` チェック削除
- `src/components/ui/TeamSettingsTab.tsx`
  - `登録ロール数 / 利用可能枠` の表示見直し
  - role 数と並行数が結びついているように見える文言の修正

## 次にやるとよいこと

- Team 設定保存時の role 数制約を撤廃する
- UI 上の「利用可能枠」や警告文言を、全体実行上限として再定義する
- 実地テストで `roles = 1, max_concurrent_agents = 3` のような構成が保存・実行できることを確認する

## 主な関連ファイル

- `src-tauri/migrations/13_task_role_assignment.sql`
- `src-tauri/src/db.rs`
- `src-tauri/src/claude_runner.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/scaffolding.rs`
- `src/components/board/TaskFormModal.tsx`
- `src/components/kanban/StorySwimlane.tsx`
- `src/components/kanban/TaskCard.tsx`
- `src/components/terminal/TerminalDock.tsx`
- `docs/28_multi_agent_execution/task.md`
- `docs/28_multi_agent_execution/walkthrough.md`
- `docs/28_multi_agent_execution/handoff.md`
