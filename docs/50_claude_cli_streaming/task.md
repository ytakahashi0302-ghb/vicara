# EPIC50: Claude CLIストリーミング修正

## 背景

Gemini CLIやCodex CLIではエージェントの逐次動作がDevターミナルにリアルタイムで表示されるが、Claude CLIだけ出力がストリーミングされない問題がある。3つのCLIはすべて同一の `spawn_agent_process` 関数を通り、`std::process::Command` + piped stdout/stderr で実行されるため、ストリーミングの仕組み自体は共通である。問題はClaude CLI固有の出力挙動にあると推定される。

レトロスペクティブ機能ではエージェントの実行ログを振り返りに活用するため、ストリーミングの修正は重要な前提となる。

## ゴール

- Claude CLIの出力がDevターミナルにリアルタイムでストリーミングされるようにする
- 既存のGemini/Codexのストリーミングに影響を与えない
- 根本原因を特定し、恒久的な修正を行う

## スコープ

### 含む

- Claude CLIのストリーミング問題の根本原因調査
- `src-tauri/src/cli_runner/claude.rs` の修正（npm shim解決 / 出力フォーマットフラグ等）
- `src-tauri/src/claude_runner.rs` の修正（重複抑制ロジックの調整等）
- デバッグログの追加による原因の切り分け

### 含まない

- Gemini/Codexのストリーミング変更
- TerminalDock.tsx のフロントエンド変更（ストリーミングが正しくemitされれば表示は動く前提）
- 実行ログ蓄積機能の追加（EPIC51で実装）

## タスクリスト

### Story 0: ドキュメント更新

- [x] implementation_plan.md をレトロ用ログ保存スコープ込みで更新
- [x] task.md をレトロ用ログ保存タスク込みで更新

### Story 1: 原因調査

- [x] Claude CLIが `.cmd` shim経由で起動されているか確認（Geminiのようなnpm shim解決がClaudeに未実装）
- [x] Claude CLI (`claude -p`) のstdoutバッファリング挙動をパイプ環境で調査
- [x] `should_suppress_duplicate_output` の750ms閾値がClaude出力パターンに悪影響を与えていないかログ追加で確認
- [x] Claude CLIの `--output-format` オプションの有無を確認

### Story 2: 修正実装

- [x] Claude CLIの `prepare_invocation` を実装し、Windowsでのnpm shim解決を追加（Gemini Runnerのパターンに従う）
- [x] 必要に応じて `--output-format stream-json` または類似フラグを `build_args` に追加
- [x] Devターミナル表示を stream-json 生データではなく thinking 中心の可読表示に整形
- [x] 重複抑制ロジックの調整（必要な場合のみ）
- [x] デバッグ用ログの追加（stdout/stderrの読み取りイベント発生確認）

### Story 3: 検証

- [x] Claude CLIの出力がDevターミナルにリアルタイム表示されることを確認
- [x] Gemini/Codexのストリーミングが引き続き動作することを確認
- [x] 長時間タスクでの出力途切れがないことを確認

### Story 4: レトロ用ログ保存

- [x] レトロ用 DB スキーマ（run / tool events）を追加
- [x] `claude_runner.rs` にセッション単位の retro capture state を追加
- [x] Claude CLIの thinking / 回答 / tool use を構造化抽出して保存
- [x] Gemini CLIの実行ログと最終回答候補を保存
- [x] Codex CLIの実行ログと `--output-last-message` を保存
- [x] changed_files を run レコードへ保存
- [x] 保存サイズ上限を導入
- [x] run 完了時に DB へ永続化
- [x] parser / DB helper のテストを追加
- [x] `cargo test` が通る
- [x] `cargo build` がエラーなく完了する

## 完了条件

- [x] Claude CLIの出力がDevターミナルにリアルタイムでストリーミングされる
- [x] Gemini/CodexのDevターミナル出力に影響がない
- [x] レトロ用 run ログが Claude/Gemini/Codex で保存される
- [x] tool event が保存可能な CLI では明細が残る
- [x] `cargo test` が通る
- [x] `cargo build` がエラーなく完了する
