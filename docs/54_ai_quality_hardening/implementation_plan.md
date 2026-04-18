# EPIC54: AI品質向上と保守性改善 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: EPIC53 完了
- 作成日: 2026-04-17

## Epic の目的

AI 関連機能は EPIC53 までで一通りつながったが、保守性の観点では次のボトルネックが見えている。

1. `src-tauri/src/ai.rs` に責務が集中し、局所変更でも広範囲を読む必要がある
2. マルチ CLI 対応後も、共通レイヤーに Claude 固有名が残り、設計意図が読み取りにくい
3. AI 指示文が transport ごとに分散し、品質改善を横断的に行いづらい
4. 最近の Epic 追加により、周辺コメント・イベント名・フロントエンド型名に命名のねじれが蓄積している

本 Epic では、機能改変ではなく「構造の再整理」と「曖昧さの除去」を主目的とする。

## 調査結果サマリ

### 1. `ai.rs` の責務集中

`src-tauri/src/ai.rs` は 2847 行あり、少なくとも以下の責務を同居させている。

- 共通型定義（`GeneratedTask`, `Message`, `RefinedIdeaResponse` など）
- transport 解決と CLI 実行 helper
- PO アシスタント関連ロジック
- Task 分解 / Idea refine / Inception
- Retro review / KPT synthesis
- `#[cfg(test)]` のユニットテスト群

この状態では、ある 1 つのフローだけを修正しても import / helper / test の影響を広く追う必要がある。

### 2. 共通層に残る旧命名

以下は「Claude 固有実装」ではなく、すでに共通責務になっているにもかかわらず旧命名のまま残っている箇所である。

- `src-tauri/src/claude_runner.rs`
- `src-tauri/src/llm_observability.rs` の `ClaudeCliUsageRecordInput`
- Tauri command: `get_active_claude_sessions`, `execute_claude_task`, `kill_claude_process`
- Tauri event: `claude_cli_started`, `claude_cli_output`, `claude_cli_exit`
- フロントエンド型 / 関数名: `ActiveClaudeSession`, `ClaudeOutputPayload`, `ClaudeExitPayload`

一方で `src-tauri/src/cli_runner/claude.rs` のような「Claude 固有 Runner」はそのまま残すべきであり、今回の rename 対象は明確に切り分ける必要がある。

### 3. AI 指示文の曖昧さ・不統一

具体的には以下が気になった。

- `refine_idea` の API 側 system prompt が `"PO Assist"` のみで、役割・出力形式・会話姿勢が CLI より曖昧
- `build_task_prompt()` は最低限の指示しかなく、完了条件・既存変更尊重・失敗時の振る舞い・自己検証の期待値が粗い
- `generate_tasks_from_story` は API と CLI で文脈の与え方と制約の細かさが異なる
- PO アシスタントは API / CLI で同種の方針を別々に保持しており、片側だけ改善されるリスクが高い

### 4. 今回ついでに直す価値が高い点

- `ai.rs` と `claude_runner.rs` に共通 helper が散っている
- `scaffolding.rs` に「共通 CLI イベントなのに claude_runner と同じ」といった stale な説明が残っている
- フロントエンドのトースト文言・型名・イベント名が旧実装名に引きずられている
- テストが巨大ファイル末尾に集約され、関連ロジックとの距離が遠い

## スコープ

### 対象ファイル（主要）

- `src-tauri/src/ai.rs` → `src-tauri/src/ai/` への再編
- `src-tauri/src/claude_runner.rs` または後継の generic 名モジュール
- `src-tauri/src/llm_observability.rs`
- `src-tauri/src/scaffolding.rs`
- `src-tauri/src/lib.rs`
- `src/components/kanban/TaskCard.tsx`
- `src/components/project/ScaffoldingPanel.tsx`
- `src/components/terminal/TerminalDock.tsx`
- `src/types/index.ts`

### 対象外

- 新 provider / 新 CLI 追加
- プロンプトの仕様変更を伴う大きな UX 改修
- `frontend-core` 配下の型・Context・Hooks の構造変更

## 実装方針

### 1. `ai.rs` をディレクトリモジュールへ移行する

`mod ai;` は維持しつつ、実体を単一ファイルからディレクトリ構成へ移す。

想定構成:

```text
src-tauri/src/ai/
  mod.rs
  common.rs
  task_generation.rs
  idea_refine.rs
  inception.rs
  team_leader.rs
  retro.rs
```

分割方針:

- `mod.rs`: 公開型、`#[tauri::command]` の再 export、最小限の束ね役
- `common.rs`: transport 解決、JSON parse、CLI 実行 helper、共通 utility
- `task_generation.rs`: `generate_tasks_from_story`
- `idea_refine.rs`: `refine_idea`
- `inception.rs`: `build_inception_system_prompt`, `chat_inception`
- `team_leader.rs`: PO アシスタント関連、fallback、execution plan 適用
- `retro.rs`: retro review / synthesis、retro prompt builder、関連 test

重要なのは「まず移動と import 整理だけで挙動を変えない」ことであり、ロジック変更は最小限に抑える。

### 2. 命名の generic 化は「内部責務」と「外部契約」を分けて進める

今回の rename は 2 層に分ける。

#### 2-1. 内部責務の rename

- `ClaudeCliUsageRecordInput` → `CliUsageRecordInput`
- `ClaudeOutputPayload` / `ClaudeExitPayload` → generic 名へ変更
- `ActiveClaudeSession` → `ActiveAgentSession`
- コメント・ログメッセージ内の「Claude 固定」表現を、共通責務であれば generic 化する

#### 2-2. 外部契約の rename

対象:

- command: `execute_claude_task`, `kill_claude_process`, `get_active_claude_sessions`
- event: `claude_cli_started`, `claude_cli_output`, `claude_cli_exit`

方針:

- バックエンドとフロントエンドを同一 Epic 内で同時更新する
- 破壊的影響が大きい場合は、短期的に alias / 互換 emit を残して安全に移行する
- `cli_runner/claude.rs` のような「Claude 固有 Runner」は rename 対象外とする

候補名:

- `execute_agent_task`
- `kill_agent_process`
- `get_active_agent_sessions`
- `agent_cli_started`
- `agent_cli_output`
- `agent_cli_exit`

### 3. AI 指示文は shared builder を増やしてズレを抑える

単に文言を直すだけでなく、「同じ意図の指示が API / CLI で二重管理されない」状態を目指す。

実施内容:

- prompt inventory を先に作る
- API / CLI 共通の核となる instruction を builder 関数へ寄せる
- transport 差分は「出力フォーマット」「ツール呼び出し方式」など最小限に限定する

優先改善対象:

1. `refine_idea`
   - API 側に役割、会話姿勢、JSON schema、出力粒度を明示する
2. Dev Agent 実行プロンプト
   - 完了条件、自己検証、既存変更尊重、ブロッカー時の振る舞いを明確にする
3. `generate_tasks_from_story`
   - project context の使い方、語彙の具体性、priority / dependency ルールを transport 間で揃える
4. PO アシスタント
   - create_story / add_note / suggest_retro の使用境界を共通方針として一元化する

### 4. 追加是正は「今回の分割と相性が良いもの」に限定する

優先度が高い候補:

- stale comment / stale naming の整理
- テストの責務ごとの近接配置
- 共有 helper の抽出
- prompt builder の unit test 強化
- Dev Agent 完了後に `package.json` / lockfile 変更を検知し、shared `node_modules` へ依存を再同期する
- Review / Preview 導線で再開発コメントを安全に追加コンテキストとして渡せるようにする
- Preview サーバ停止を Windows のプロセスツリー単位で扱い、merge/remove 後の残留を減らす
- Preview 起動前にも依存ヘルスチェックを行い、manifest 未変更でも local binary 不足を self-heal する

逆に、仕様変更を伴う改善や新機能提案は本 Epic では広げない。

## 実装順序

1. 変更前の呼び出し点・イベント購読点・テスト観点を固定する
2. `ai.rs` をディレクトリモジュールへ分割し、コンパイルを通す
3. 共通 DTO / payload / session 型の rename を行う
4. command / event 名の generic 化をバックエンド・フロントエンドで揃える
5. AI 指示文の共通 builder 化と曖昧文言の改善を行う
6. stale comment / helper / test 配置を整える
7. 自動テストと手動 smoke test を実施する
8. Preview 確認後の差分コメント付き rerun 導線を追加し、既存の `execute_agent_task(additional_context)` に載せる
9. Preview 起動前・merge/remove 時に stale preview pid を cleanup できるよう停止経路を補強する
10. Dev Agent 完了時に `package.json` / lockfile 変更を検知したら、worktree 上で root / `--prefix` package の install を自動再同期する
11. Preview 起動前に script で使う local binary の不足を検知し、必要な場合だけ install を走らせて self-heal する
12. `agent_runner/spawn` と `team_leader/plan` をさらに小さいサブモジュールへ分け、終了処理 / timeout / platform 差分 / fallback 適用の責務を分離する
13. `agent_retro` から provider 固有の stream-json パースを分離し、capture 本体には generic な mutation 適用だけを残す

## リスクと対策

### リスク 1: command / event 名変更による UI 断線

対策:

- 先に購読点と invoke 点を全列挙する
- バックエンドとフロントを同一コミット系列で更新する
- 必要なら一時的に alias を残す

### リスク 2: prompt 文言の修正で期待挙動が変わる

対策:

- 仕様変更ではなく「明文化」と「整合」に留める
- JSON schema と禁止事項は既存期待に合わせる
- 主要フローは手動 smoke test を行う

### リスク 3: 大規模移動で import 崩れや循環参照が起きる

対策:

- 1 機能ずつ段階的に分割する
- `common.rs` に寄せすぎず、依存方向を明確にする
- 各ステップで `cargo test` を回して崩れを早期検知する

### リスク 4: Preview コメント rerun が新しい実行 API を増やして複雑化する

対策:

- backend の新規 command は増やさず、既存の `execute_agent_task` が受け取る `additional_context` を利用する
- UI 側で「期待した動作 / 実際の動作」をまとめた文面を作り、prompt 側では追加コンテキストを優先課題として扱う
- 既存の conflict rerun と同じ状態遷移 (`Review` → `In Progress`) に揃えて運用差分を最小化する

### リスク 5: Windows で preview 停止が親プロセスしか落とせず、子の `npm/node` が残留する

対策:

- preview 停止は `Child::kill()` だけに頼らず、Windows では `taskkill /PID /T /F` でプロセスツリーごと停止する
- `PreviewState` にセッションが無い場合でも、DB に残った `preview_pid` を fallback に使って stale process を止める
- `start_preview_server` の前にも stale cleanup を走らせ、前回の残留が初回 preview を邪魔しにくいようにする

### リスク 6: Dev Agent が `package.json` だけ更新しても依存が再同期されず、preview で `concurrently` などが見つからない

対策:

- Scaffolding 用に分散していた Node 依存導入 helper を共通モジュールへ寄せる
- Dev Agent 完了時に worktree 差分を確認し、`package.json` / lockfile 変更があった場合のみ root / `--prefix` package の install を自動実行する
- install の stdout / stderr は `agent_cli_output` に流し、失敗時は `Review` へ進めずに明示エラーとして返す

### リスク 7: manifest が変わっていない既存 worktree では依存不足を見逃し、Preview 初回だけ失敗する

対策:

- Preview 起動前に `npm run dev` などの script を解析し、`concurrently` / `vite` / `next` など local binary が `node_modules/.bin` に存在するか確認する
- worktree の shared `node_modules` link を張り直したうえで、不足がある場合だけ install を実行する
- 毎回 install は行わず、不足が見つかったときだけ self-heal する

## テスト方針

### 自動テスト

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm run build`
- prompt builder / JSON parse / fallback 判定の unit test
- rename 後の session / payload / observability 周辺の unit test

### 手動テスト

- Dev Agent 実行開始、ログ表示、完了、手動停止が従来通り動くこと
- `TaskCard` からの起動・再実行・競合後再実行が動くこと
- `TaskCard` の Preview からコメント付き再開発を実行し、差分コメントが Dev エージェント prompt に反映されること
- merge/remove 後に preview プロセスが残らず、次の task の初回 preview 起動が blocked されないこと
- Dev Agent が `package.json` / lockfile を変更した task で、自動 install が走って preview 前に依存不足を解消できること
- manifest 変更が無い既存 task でも、Preview 起動前の self-heal により `concurrently` / `vite` 不足で初回失敗しにくくなること
- `agent_runner/spawn` の再分割後も Dev Agent / Scaffold AI の CLI 実行が従来どおり流れること
- `team_leader/plan` の再分割後も backlog 操作と fallback 生成が従来どおり適用されること
- `agent_retro` の parser 分離後も Claude / Gemini の retro capture が従来どおり保存されること
- `TerminalDock` が active session を復元し、イベントを継続受信できること
- PO アシスタントの `refine_idea` / `generate_tasks_from_story` / `chat_with_team_leader` が主要 provider / transport で破綻しないこと
- Retro review / KPT synthesis が JSON 解析失敗なく完了すること
- Scaffolding の CLI 実行イベントが引き続き UI に流れること

## 完了イメージ

実装完了時には、AI 関連コードが「機能単位で読める」「命名から責務が分かる」「prompt 品質改善を局所的に進めやすい」状態になっていることを目標とする。今回の Epic は見た目の新機能追加ではなく、今後の Epic を安全に積み上げるための土台づくりである。
