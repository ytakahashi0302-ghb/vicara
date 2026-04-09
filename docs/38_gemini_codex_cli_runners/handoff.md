# Epic 38 Handoff

## この Epic で完了したこと

- バックエンド側の Gemini / Codex Runner 実装は完了した。
- `src-tauri/src/cli_runner/gemini.rs` と `src-tauri/src/cli_runner/codex.rs` を追加し、`create_runner()` から返せる状態にした。
- `CliRunner` trait にデフォルトモデル解決とインストールヒントを持たせ、`claude_runner.rs` 側で共通利用できるようにした。
- Epic 36 の `detect_installed_clis` を使った事前チェックを追加し、未インストール CLI の場合は起動前にエラーメッセージを返すようにした。

## Epic 39 でやるべきこと

- 設定画面を作成し、ロールごとに `Claude / Gemini / Codex` を選択できるようにすること。
- UI から実際に LLM / CLI を切り替え、Gemini / Codex / 未インストール時の挙動を結合テストできる状態にすること。
- `BACKLOG.md` に追記済みの「Gemini/Codex CLIの最新引数仕様への追従」を参照し、実機検証の結果に応じて実行コマンドを見直すこと。

## 注意点

- Epic 38 時点では、PO 承認済み計画に基づき Gemini は `--sandbox permissive`、Codex は `--full-auto` で実装している。
- 公式ドキュメント上は Gemini の `--yolo` 系、Codex の `codex exec` 系が主系統に見えるため、UI 実装後の結合テストで必ず再確認すること。
- Tauri コマンド名 `execute_claude_task` とイベント名 `claude_cli_*` は互換性維持のため据え置きである。

## 運用ルール

- 今後の Epic でも、タスクを1つ消化するたびに `task.md` のチェックボックスを小まめに更新し、常に最新の進捗を可視化すること。
