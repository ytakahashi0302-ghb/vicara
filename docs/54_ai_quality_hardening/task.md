# EPIC54: AI品質向上と保守性改善 タスクリスト

## ステータス

- 状態: `Done`
- 着手条件: EPIC53 完了
- 作成日: 2026-04-17

## 背景

EPIC53 までで、レトロスペクティブを通じて AI が学習し続けるループは一通り成立した。一方で、AI 関連のバックエンドは責務集中と命名負債が目立ち始めている。

- `src-tauri/src/ai.rs` は 2800 行超となり、PO アシスタント、Inception、レトロ、CLI 実行補助、テストが同居している
- 複数 CLI 対応済みの共通層に `claude_runner` / `ClaudeCliUsageRecordInput` / `claude_cli_*` など旧来の Claude 固有名が残っている
- AI への指示文が API / CLI でずれていたり、役割や完了条件が曖昧な箇所がある
- フロントエンド側も `execute_claude_task` / `get_active_claude_sessions` / `claude_cli_output` など旧命名に追従しており、設計意図と実体がずれている

本 Epic では機能追加ではなく、既存挙動を崩さずに保守性・可読性・品質を引き上げる。

## ゴール

- `ai.rs` を責務ごとに分割し、読みやすく安全に変更できる構造へ移行する
- 共通コンポーネントから特定 LLM 名に依存した命名を除去する
- AI 指示文の棚卸しを行い、曖昧さや transport 間のズレを減らす
- 周辺の技術的負債を同時に洗い出し、今回のリファクタと相性の良い範囲で是正する

## スコープ

### 含む

- `src-tauri/src/ai.rs` の分割再編
- `src-tauri/src/claude_runner.rs` を含む CLI 実行基盤の共通命名整理
- `src-tauri/src/llm_observability.rs` の CLI 使用量記録 DTO / コメント /補助関数の命名整理
- AI プロンプトの棚卸しと、曖昧な指示文の改善
- 旧命名に追従しているフロントエンド呼び出し側の整理
- リファクタ後のテスト整備と回帰確認

### 含まない

- 新しい LLM / CLI / Provider の追加
- AI 機能そのものの仕様変更
- UI デザイン刷新

## タスクリスト

### Story 0: 現状把握と安全網の整理

- [x] `ai.rs` の責務を機能単位で棚卸しし、分割後のモジュール境界を決める
- [x] `claude_*` 命名を「Claude 固有」と「本来は共通」の 2 種類に分類する
- [x] AI 指示文の一覧を作り、曖昧さ・重複・transport 間差分を整理する
- [x] 変更前に必要な unit test / smoke test の観点を固定する

### Story 1: `ai.rs` 分割

- [x] `src-tauri/src/ai/` ディレクトリ構成へ移行する
- [x] 共通型・共通 helper・transport 解決処理を基盤モジュールへ抽出する
- [x] PO アシスタント、Task 生成、Idea refine、Inception、Retro を責務別モジュールへ分割する
- [x] `#[tauri::command]` の公開面は `ai::` モジュールから維持する
- [x] テストコードも責務に応じて近いモジュールへ再配置する

### Story 2: 共通命名のジェネリック化

- [x] `claude_runner.rs` のうち共通実行責務を表す命名を generic に改める
- [x] `ClaudeCliUsageRecordInput` など、共通 DTO の旧命名を整理する
- [x] `claude_cli_started` / `claude_cli_output` / `claude_cli_exit` などのイベント名を見直す
- [x] `execute_claude_task` / `kill_claude_process` / `get_active_claude_sessions` の見直し方針を決め、必要なら互換性レイヤを用意する
- [x] フロントエンドの購読側・呼び出し側を追従させる
- [x] CLI厳密Usage計測の準備: `CliUsageRecordInput`（旧 `ClaudeCliUsageRecordInput`）構造体に、将来の詳細なトークン計測を見据えて `prompt_tokens`, `completion_tokens` などの Optional フィールドをあらかじめ定義しておく
- [x] stream-json対応のインターフェース準備: `agent_runner`（旧 `claude_runner`）の標準出力（stdout）をパースする処理を独立した関数・インターフェースとして切り出し、将来的に stream-json 専用のパーサーを簡単に差し込める構造にしておく

### Story 3: AI 指示文の品質改善

- [x] Dev Agent 実行プロンプトの必須制約・完了条件・自己検証要件を明文化する
- [x] `refine_idea` の API 指示文を具体化し、CLI 版との期待値差を縮める
- [x] `generate_tasks_from_story` の API / CLI 指示文を共通方針で揃える
- [x] PO アシスタントの API / CLI 指示文の共通核を整理し、transport ごとの差分を最小化する
- [x] レトロ系プロンプトは出力制約と役割定義を保ちつつ、不要な曖昧さがないか確認する
- [x] PO アシスタントの task 分解 prompt を「何をどうするか」が伝わる日本語の詳細粒度へ強化する

### Story 4: 追加の是正ポイント

- [x] `agent_runner.rs` を state / prompt / spawn / command などの責務別モジュールへ再分割する
- [x] `team_leader.rs` を prompt / fallback / plan_apply / command などの責務別モジュールへ再分割する
- [x] 競合後の AI 再実行で worktree cleanup と agent session が詰まらないよう安定化する
- [x] Scaffolding 完了時に Node 依存を bootstrap し、worktree でも共有 `node_modules` を再利用できるようにする
- [x] Dev Agent 完了後に `package.json` / lockfile 変更を検知し、worktree 上の依存を自動再同期できるようにする
- [x] Preview 確認時に期待との差分コメントを添えて Dev エージェントを再実行できるようにする
- [x] Preview サーバの残留プロセスを Windows でも確実に停止し、初回 preview 失敗を起こしにくくする
- [x] Preview 起動前に依存ヘルスチェックを行い、`concurrently` / `vite` などの local binary 不足を self-heal できるようにする
- [x] 重複 helper / コメント / stale naming を整理する
- [x] `scaffolding.rs` など周辺ファイルの旧名称参照を解消する
- [x] テストしやすさを下げている密結合箇所を緩める
- [x] リファクタに合わせてドキュメント上の前提差分を更新する

### Story 5: 検証

- [x] `cargo test --manifest-path src-tauri/Cargo.toml`
- [x] `npm run build`
- [x] Dev Agent 実行 / 停止 / ターミナル表示の回帰確認
- [x] PO アシスタントの主要フロー（idea refine / task generation / team leader）の回帰確認
- [x] レトロレビュー / KPT 合成の回帰確認
- [x] Scaffolding の CLI 実行イベント連携の回帰確認

## 完了条件

- [x] `ai.rs` が責務分割され、単一ファイル肥大化が解消されている
- [x] 共通層から不適切な Claude 固有命名が除去されている
- [x] AI 指示文の品質改善方針がコードに反映され、主要 transport 間のズレが縮小している
- [x] 主要導線の挙動が変更前と同等以上であることを確認できている
