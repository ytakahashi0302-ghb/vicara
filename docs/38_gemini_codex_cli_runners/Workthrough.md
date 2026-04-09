# Epic 38: Gemini CLI / Codex CLI Runner 実装 修正内容の確認

## 実施内容

- `src-tauri/src/cli_runner/gemini.rs` を追加し、Gemini CLI 向けの `GeminiRunner` を実装した。
- `src-tauri/src/cli_runner/codex.rs` を追加し、Codex CLI 向けの `CodexRunner` を実装した。
- `src-tauri/src/cli_runner/mod.rs` を拡張し、`CliRunner` trait に以下を追加した。
  - `default_model()`
  - `install_hint()`
  - `resolve_model()`
- `create_runner()` を更新し、`CliType::Gemini` と `CliType::Codex` の分岐を有効化した。
- Claude / Gemini / Codex の既定モデルをそれぞれ定義し、ロール設定の `model` が空文字のときは Runner ごとの既定値へフォールバックするようにした。
- `src-tauri/src/claude_runner.rs` に CLI 事前チェックを追加し、Epic 36 の `detect_installed_clis` を使って、未インストール CLI を選択した場合は起動前に明確なエラーメッセージを返すようにした。

## ファクトリ拡張の意図

Epic 37 で抽象化された `CliRunner` の責務は維持しつつ、CLI ごとの差分を Runner 実装へ閉じ込めた。これにより、今後 UI で `cli_type` を切り替えたときも、バックエンド側では `create_runner()` から適切な実行ロジックを取得するだけで済む構成になった。

特に今回の拡張では、以下を共通処理として整理した。

- CLI ごとのデフォルトモデル解決
- CLI ごとのインストールヒント表示
- 未インストール時の事前チェック

この整理により、今後 CLI 種別が増えた場合でも、Runner 実装追加と `create_runner()` への登録で同じパターンを再利用できる。

## 公式ドキュメントとの差異を発見した経緯

PO 承認済みの `implementation_plan.md` に基づいて、まず以下の引数マッピングで実装を進めた。

- Gemini: `--sandbox permissive`
- Codex: `--full-auto`

その後、実装の妥当性確認として最新の公式ドキュメントを参照したところ、案内されている実行方法に差異があることを確認した。

- Gemini CLI の公式ドキュメントでは、非対話実行に `--prompt` を使いつつ、自動承認系は `--yolo` または `--approval-mode=yolo` が中心に案内されていた。
- Codex CLI の公式ドキュメントでは、非対話実行は `codex exec` が主系統として案内されていた。

ただし、本Epicでは PO 承認済みの計画書に従うことを優先し、実装自体は計画書どおりの引数で完了させた。差異はリスクとして整理し、`BACKLOG.md` に「最新引数仕様への追従」タスクとして追記した。

## テスト

- `cargo test --manifest-path src-tauri/Cargo.toml`
  - 結果: 成功

## 補足

- 実機での CLI 切替確認は、設定画面（UI）が未実装のため Epic 39 以降へ持ち越した。
- `task.md` の動作確認項目は、PO 判断により「Epic 39以降で実施」としてスキップ完了扱いに更新した。
