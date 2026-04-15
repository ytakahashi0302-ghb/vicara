# EPIC50 実装メモ

## 2026-04-15 初期調査

- `src-tauri/src/cli_runner/claude.rs` には Gemini / Codex と違って `prepare_invocation` が未実装で、Windows 向け npm shim 解決が入っていない。
- 実機の `Get-Command claude` は `C:\Users\green\.local\bin\claude.exe` を返し、この開発環境では `.cmd` shim ではなくスタンドアロン実行ファイルが優先されている。
- ただし `npm root -g` は `C:\Users\green\AppData\Roaming\npm\node_modules` を返し、npm グローバル配置を前提にした shim 解決の互換性は引き続き必要と判断した。
- `claude --help` で `--output-format text|json|stream-json` と `--include-partial-messages` の存在を確認した。`stream-json` は `--print` と併用時のみ有効。
- npm 版 Claude Code のエントリポイントは `@anthropic-ai/claude-code/cli.js` 直下構成とみられる。これは公開 issue のパッケージ内容説明を根拠にした推定で、実装では存在確認付きのフォールバックにする。

## 2026-04-15 実装

- `src-tauri/src/cli_runner/claude.rs` に `prepare_invocation` を追加し、Windows で `claude.cmd` を見つけた場合は `node.exe + @anthropic-ai/claude-code/cli.js` へ自動で書き換えるようにした。
- 同ファイルの `build_args()` に `--output-format stream-json` と `--include-partial-messages` を追加した。`claude --help` が `stream-json` を realtime streaming 用フォーマットとして案内していたため、テキスト既定値よりストリーミング優先と判断した。
- `src-tauri/src/claude_runner.rs` の stdout/stderr リーダースレッドに `[STREAM][stdout|stderr]` デバッグログを追加し、受信・重複抑制・emit の各段階が追えるようにした。
- ログの preview は 160 文字で打ち切る実装にして、巨大な chunk 全体を debug 出力しないようにしている。

## 2026-04-15 検証

- `cargo test --manifest-path src-tauri/Cargo.toml` は 78 件すべて成功した。今回追加した Claude 用 npm shim 解決テストも通過。
- `cargo build --manifest-path src-tauri/Cargo.toml` は成功した。
- この開発環境の `claude` 実体は `claude.exe` であり、npm `.cmd` shim の実経路は手元では再現していない。そのため Windows npm グローバルインストール環境での Dev ターミナル実機確認は別途必要。
- Dev ターミナル上でのリアルタイム表示、Gemini/Codex 回帰、長時間タスクでの途切れ有無はこのターンでは未実施。

## 2026-04-15 追加フィードバック

- ユーザー確認により、Claude の出力自体は Dev ターミナルへリアルタイム表示される状態になった。
- 一方で `stream-json` の生 JSON 行がそのまま並ぶため可読性が低く、thinking だけ見たいという追加要望が出た。
- 次の修正では、Claude の `stream-json` を TerminalDock 側で吸収し、thinking 中心の表示へ整形する。

## 2026-04-15 thinking 表示整形

- `src/components/terminal/TerminalDock.tsx` に Claude stream-json 専用の簡易パーサを追加した。
- 1 セッションごとに未完了行バッファを保持し、chunk 境界で JSON 行が分割されても次回イベントで継続パースできるようにした。
- 表示対象は `thinking_delta` を優先し、thinking が一度も出ていない場合だけ `text_delta` / assistant text を最小限フォールバック表示する。
- JSON として解釈できない行はそのまま表示するため、stderr のプレーンテキストエラーは引き続き見える。
- セッション終了時には残バッファを flush してから終了行を足すようにした。

## 2026-04-15 フロント検証

- `npm run build` は成功した。

## 2026-04-15 レトロ用ログ保存設計

- レトロ目的では raw `stream-json` 全保存は容量効率が悪いため、`run` 集約テーブルと `tool event` 明細テーブルの 2 段構成にした。
- 保存対象は `reasoning_log` / `final_answer` / `changed_files` / `tool events` / 実行メタ情報に限定し、生 stdout 全文やツール出力全文は保存しない方針にした。
- 文字数上限は backend 側で適用し、`reasoning_log` は 32KB、`final_answer` は 16KB、tool summary は 512 文字で打ち切る。

## 2026-04-15 レトロ用ログ保存実装

- migration `20_agent_retro_logs.sql` を追加し、`agent_retro_runs` / `agent_retro_tool_events` を作成した。
- `src-tauri/src/db.rs` に retro run / tool event の insert helper を追加した。
- `src-tauri/src/agent_retro.rs` を新設し、CLI ごとの retro capture と DB 永続化を分離した。
- `claude_runner.rs` のセッションに retro capture state と response capture path を持たせ、完了・タイムアウト・手動 kill の各経路で保存するようにした。
- Claude は `stream-json` から `thinking_delta` / assistant text / tool_use / tool_result を構造化抽出する。
- Gemini は現行出力を best-effort で `reasoning_log` と `final_answer` 候補に保存する。
- Codex は今回から `prepare_response_capture` を実行経路に組み込み、`--output-last-message` を `final_answer` 保存へ流用するようにした。
- task に紐づく実行では `changed_files` も保存する。

## 2026-04-15 レトロ保存の検証

- `cargo test --manifest-path src-tauri/Cargo.toml` は 80 件すべて成功した。
- Claude retro parser のユニットテストと Gemini plain-text fallback のユニットテストを追加し、通過した。
- `cargo build --manifest-path src-tauri/Cargo.toml` は成功した。
- DB へ実際に書き込まれた run レコード内容の手動確認はこのターンでは未実施。保存コードと migration の導線までは接続済み。

## 2026-04-15 PO 実機検証

- PO による Dev ターミナル上の実機確認で、Claude CLI の thinking が可読な形でリアルタイム表示されることを確認した。
- Gemini CLI についても既存ストリーミング挙動に悪影響がなく、回帰していないことを確認した。
- Epic 50 の残タスクとして残していた長時間タスク時の出力途切れ確認、重複抑制の悪影響確認、Gemini/Codex 回帰確認は PO 検証完了としてクローズした。
- これにより、Epic 50 の目的である「Claude のストリーミング修正」と「レトロ用ログ蓄積基盤の確立」は完了と判断する。
