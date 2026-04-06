# Epic 28 実装計画

## 1. 背景整理

Epic 27 により、Team 設定は以下の形で SQLite に保存されるようになった。

- `team_settings.id = 1` に singleton で `max_concurrent_agents` を保持
- `team_roles` に `id`, `name`, `system_prompt`, `model`, `sort_order` を保持
- 保存時制約として `max_concurrent_agents <= roles.len()` を保証

一方で現状の実行系には以下の制約が残っている。

- `claude_runner.rs` は単一セッション前提で、同時に 1 プロセスしか持てない
- `execute_claude_task` はフロントから渡された文字列 prompt をそのまま `-p` で実行している
- role の `model` / `system_prompt` は Claude CLI 実行に使われていない
- TerminalDock は単一ストリーム表示で、複数タスクの識別ができない
- 緊急停止は「現在 1 個だけある実行中プロセス」を止める設計になっている

本 Epic では、保存済み Team 設定を「実行基盤」に接続する。

## 2. 設計方針

### 2-1. MVP の基本判断

- 並行数上限を超えた要求は queue せず即時 reject する
- role 解決と prompt 合成はバックエンドに集約する
- フロントは `task_id` を起点に実行要求するだけにし、role prompt の組み立てを持たない
- 実行中プロセスの識別子は PID ではなく `task_id` とする
- 停止対象の特定は `kill_claude_process(task_id)` で行う
- TerminalDock は「タブ式マルチプレックス」を採用する

### 2-2. 理由

- `task_id` 基準にすると Windows / Unix の実装差や PID 再利用の影響を UI に漏らさずに済む
- prompt 合成をバックエンドに寄せることで、role 未設定や model 未指定を UI 迂回で実行される事故を防げる
- queue を入れないことで、まずは安全な並行実行と状態同期の責務に集中できる

## 3. データモデル計画

### 3-1. タスクへの role 割り当て

MVP では `tasks` テーブルに新しく `assigned_role_id TEXT NULL` を追加する案を採用する。

理由:

- 既存の `assignee_type` は現在ほぼ未使用で、役割 ID を入れるには名前が曖昧
- `assigned_role_id` の方が Team Role との関連を明示できる
- 将来 `assignee_type = human | ai` のような概念を導入しても衝突しない

想定 migration:

- `ALTER TABLE tasks ADD COLUMN assigned_role_id TEXT REFERENCES team_roles(id) ON DELETE SET NULL`

更新対象:

- Rust `Task` struct
- `add_task`, `update_task`, `get_tasks`, `get_tasks_by_story_id`
- TypeScript `Task` interface
- `useTasks`
- `TaskFormModal`

### 3-2. 役割未設定時の扱い

- タスク作成時は `NULL` を許可する
- ただし Claude 実行時は `assigned_role_id` 必須とする
- role が削除済みで参照切れになった場合も実行前 validation で止める

### 3-3. UI 案

担当ロールの選択 UI は、MVP では `TaskFormModal` に追加する。

- 項目名: `担当ロール`
- 入力形式: Team 設定の role 一覧を使った `select`
- 表示ラベル: `role.name`
- 補助情報: option の右側または注記で `model` も見せる
- 初期値: 未設定可

この方式なら、専用画面を増やさず既存のタスク編集フローに自然に乗せられる。

## 4. Rust 側の非同期プロセス管理計画

### 4-1. `ClaudeState` の再設計

現状:

- `current_session: Option<ClaudeSession>`

変更案:

- `sessions: HashMap<String, ClaudeSession>`
- key は `task_id`

`ClaudeSession` に持たせるメタデータ案:

- `task_id`
- `role_id`
- `role_name`
- `model`
- `started_at`
- `temp_file_path`
- `killer`

必要なら UI 初期化用に軽量 DTO `ActiveClaudeSession` を別に切る。

### 4-2. 実行フロー

`execute_claude_task` は、フロントから生 prompt を受け取らず、以下の順序で動くよう変更する。

1. `task_id` から task を取得する
2. task の `assigned_role_id` を確認する
3. `team_roles` から担当 role を取得する
4. `team_settings.max_concurrent_agents` を取得する
5. 実行中数と比較し、上限超過ならエラー
6. 同一 `task_id` がすでに実行中ならエラー
7. task の状態が `In Progress` または `Done` ならエラー
8. 実行用一時ファイルを生成する
9. Claude CLI を `--file` と `--model` 付きで起動する
10. 実行中セッションに登録し、started event を emit する

### 4-3. 一時ファイルの内容

一時ファイルの先頭には、role context を固定フォーマットで合成する。

例:

```text
あなたは [役割名] です。
[system_prompt]

# タスク名
...

# 詳細
...

# 作業指示
- タスクのゴールを達成するための実装を行ってください。
- 必要な変更を加え、完了前に自己検証してください。
```

この方式により、role ごとの人格・責務・モデルをバックエンドで強制できる。

### 4-4. Claude CLI 引数

現状:

```text
claude -p <prompt> --permission-mode bypassPermissions --add-dir <cwd> --verbose
```

変更後の想定:

```text
claude --file <temp_file> --model <role.model> --permission-mode bypassPermissions --add-dir <cwd> --verbose
```

### 4-5. イベント設計

既存:

- `claude_cli_output`
- `claude_cli_exit`

追加:

- `claude_cli_started`

推奨 payload:

- `claude_cli_started`
  - `task_id`
  - `role_id`
  - `role_name`
  - `model`
  - `task_title`
  - `started_at`
- `claude_cli_output`
  - `task_id`
  - `output`
- `claude_cli_exit`
  - `task_id`
  - `success`
  - `reason`

加えて、画面再読み込みや TerminalDock 再 mount に備えて `get_active_claude_sessions` コマンドを追加する。

### 4-6. kill と cleanup

- `kill_claude_process(task_id)` は対象 session のみ kill する
- exit / kill / timeout いずれでも session を確実に map から除去する
- 一時ファイルは成功・失敗・kill を問わず必ず削除する
- timeout はタスク単位で持ち、他タスクの実行に影響させない

## 5. フロントエンド計画

### 5-1. タスク起動ボタンの状態制御

`TaskCard.tsx` の「開発を実行」は以下で無効化する。

- `task.status === 'In Progress'`
- `task.status === 'Done'`
- `runningTaskIds` に task が含まれる

表示方針は MVP では「disabled 推奨」とする。

理由:

- 非表示よりも、押せない理由を tooltip 等で説明しやすい
- 状態変化がユーザーに見えやすい

補助表示案:

- `In Progress`: 「このタスクはすでに実行中です」
- `Done`: 「完了済みタスクは再実行できません」
- role 未設定: 「担当ロールを設定してください」

### 5-2. 実行要求の流れ

現在はフロントが prompt を組み立てて `execute_claude_task` に渡しているが、これをやめる。

変更後:

- フロントは `taskId` と `cwd` のみ渡す
- role prompt と model 注入は Rust が担当する

これにより、UI 側が role 情報の真実源にならない。

### 5-3. 状態同期

フロントでは、実行中タスクを `Set<task_id>` とセッションメタデータ map で保持する。

更新契機:

- `claude_cli_started`
- `claude_cli_exit`
- 初回 mount 時の `get_active_claude_sessions`

これを `TaskCard` と `TerminalDock` の双方から使えるよう、専用 hook か Context に切り出すのが望ましい。

## 6. TerminalDock 改修案

### 6-1. UI 方式

MVP は「タブ UI」を採用する。

各タブ表示項目:

- タスク名
- 担当ロール名
- 状態 (`Running`, `Exited`, `Killed`, `Failed`)
- 状態インジケーター（例: Running は回転スピナーまたは緑ドット、Failed は赤アイコン、Done はチェック）

タブ本文:

- その task 専用のログバッファを xterm に描画する

### 6-2. なぜプレフィックス方式ではなくタブ方式か

- 長時間ログが混ざると可読性が急激に落ちる
- kill 対象を UI 上で一意に選びやすい
- 将来「完了済みログを少し残す」拡張に向いている

### 6-3. 停止ボタンの制御

停止ボタンは「アクティブタブが Running のときのみ」表示または活性化する。

挙動:

- 押下時に `kill_claude_process(activeTaskId)` を呼ぶ
- 実行中でないタブでは停止ボタンを出さない
- タブを閉じても backend 側 process は勝手に kill しない

### 6-4. 実装メモ

- xterm を 1 個で使い回すより、task ごとのログ文字列を state に保持してアクティブタブ切替時に再描画する方が実装しやすい
- アクティブタスクの started event を受けたら、そのタブへ自動フォーカスする
- 直近で終了したタブは一定数残して、結果確認後に閉じられるようにする
- 非アクティブタブでも稼働状況を把握できるよう、タブラベル右側に状態インジケーターを常時表示する

## 7. バックログ課題への具体対応

### 7-1. BACKLOG課題1: 「開発を実行」ボタン

対応方針:

- `In Progress` / `Done` 時は disabled
- 実行中集合にもとづいて厳密に disabled
- tooltip または title で理由を表示
- backend 側でも status / 二重起動 / 上限超過を再検証する

### 7-2. BACKLOG課題2: 「緊急停止」ボタン

対応方針:

- 実行中セッションがないときは非表示または disabled
- TerminalDock の active tab に紐づく task だけ停止する
- kill API は `task_id` ベースに統一する
- session 一覧取得コマンドにより、再描画後も停止対象を見失わないようにする

## 8. 想定ファイル影響範囲

### Backend

- `src-tauri/migrations/13_task_role_assignment.sql`（新規想定）
- `src-tauri/src/db.rs`
- `src-tauri/src/claude_runner.rs`
- `src-tauri/src/lib.rs`

### Frontend

- `src/types/index.ts`
- `src/hooks/useTasks.ts`
- `src/components/board/TaskFormModal.tsx`
- `src/components/kanban/TaskCard.tsx`
- `src/components/terminal/TerminalDock.tsx`
- 必要に応じて `src/context/**` または `src/hooks/**` に実行状態管理を追加

## 9. テスト方針

### 9-1. バックエンド

- `cargo check` で型整合性を確認する
- ClaudeState の map 化に伴う borrow / lock の破綻がないことを確認する
- role 未設定、role 不存在、上限超過、同一 task 二重起動で適切にエラーになることを確認する

### 9-2. フロントエンド

- `npm run build` で型と bundling を確認する
- TaskFormModal で role 選択が保存されることを確認する
- `TaskCard` で Running / Done のボタン状態が正しいことを確認する
- TerminalDock でタブ切替時にログ混線が起きないことを確認する

### 9-3. 手動確認シナリオ

1. role を 2 件以上持つ Team 設定で、各タスクに別 role を割り当てて保存できる
2. `max_concurrent_agents = 1` のとき、2 件目の起動が拒否される
3. `max_concurrent_agents = 2` のとき、2 件が同時に走り、3 件目は拒否される
4. 起動したタスクが role ごとの model で実行される
5. TerminalDock の各タブでログが分離される
6. アクティブタブの停止ボタンで対象 task だけ止まる
7. 正常終了時に該当 task だけ `Done` へ更新される
8. kill / timeout / エラー終了時に task が誤って `Done` にならない

## 10. PO 確認ポイント

以下を前提に実装へ進む想定。

- 担当ロール選択 UI は `TaskFormModal` に置く
- `assigned_role_id` を新設し、既存 `assignee_type` は温存する
- 並行数超過時は queue せずエラー通知とする
- TerminalDock はタブ方式で進める
- 「開発を実行」は非表示ではなく disabled を基本方針とする

承認後は、この方針に沿って migration → backend → frontend の順で実装に入る。
