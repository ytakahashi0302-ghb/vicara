# Epic 42: PO アシスタント CLI/API 選択対応 Walkthrough

## 概要

Epic 42 では、PO アシスタントの実行方式を API / CLI から選択できるようにし、`refine_idea`、`generate_tasks_from_story`、`chat_inception`、`chat_with_team_leader` の全 4 機能を transport 切り替え可能な構成へ拡張した。

今回の Epic では、単純な transport 分岐の追加だけでなく、PO アシスタント特有の 1 ショット CLI 実行、会話履歴のシリアライズ、CLI での JSON 計画実行、provider 障害時のフォールバックも含めて整備している。

## 実施内容

### 1. バックエンド: PO アシスタント transport 分岐の実装

- `src-tauri/src/ai.rs` に `PoTransport`、`resolve_po_transport()`、`execute_po_cli_prompt()` を追加し、PO アシスタント専用の transport 解決と CLI 実行基盤を実装した。
- `execute_po_cli_prompt()` は CLI を 1 ショットで起動し、stdout を全量キャプチャして `parse_json_response()` に渡す構成にしている。
- `refine_idea`、`generate_tasks_from_story`、`chat_inception`、`chat_with_team_leader` の全 4 関数で API / CLI 分岐を追加した。
- `chat_inception` と `refine_idea` では会話履歴を Markdown 形式でシリアライズし、CLI へまとめて渡すようにした。

### 2. Team Leader: CLI 計画実行と API 障害時の救済

- `chat_with_team_leader` の CLI モードでは、tool calling を使わず JSON の実行計画を返させ、アプリ側で DB 登録を行う構成にした。
- API モードでは、provider の最終応答が失敗しても、`create_story_and_tasks` による DB 更新済みなら部分成功として返すようにした。
- 一時的な 503 / `UNAVAILABLE` に対しては再試行し、未反映時は例外ではなく通常返信として扱う経路を追加した。
- 抽象的な「バックログを 1 つ作って」要求では、`PRODUCT_CONTEXT.md` / `ARCHITECTURE.md` / `Rule.md` と既存 backlog から具体案を生成する fallback を追加した。

### 3. Gemini CLI 向けの追加調整

- Windows の Gemini CLI では、長文 prompt をコマンド引数へ直接渡さず、短い `--prompt` と stdin の併用で headless モードを維持するようにした。
- Gemini CLI はアプリ設定の API キー注入に依存しない前提へ修正し、既存ログイン状態で動作させる構成へ戻した。
- `~/.gemini/trustedFolders.json` を参照し、project local path が未 trust の場合は trust 済みフォルダへ `cwd` をフォールバックする処理を追加した。

### 4. フロントエンド: PO アシスタント設定 UI の拡張

- `src/components/ui/GlobalSettingsModal.tsx` に PO アシスタントの「実行方式」セクションを追加した。
- `API / CLI` のラジオボタンを追加し、CLI 選択時は `CLI 種別` と `モデル`、API 選択時は既存の provider 設定を表示する構成にした。
- 保存キーとして `po-assistant-transport`、`po-assistant-cli-type`、`po-assistant-cli-model` を追加した。

## 検証結果

### 自動検証

以下はいずれも成功した。

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm run build`

### 手動確認の到達点

- Claude CLI: backlog 作成まで確認済み
- Claude API: backlog 作成まで確認済み
- Gemini CLI: timeout が継続し、安定動作は未確認
- Gemini API: 503 `UNAVAILABLE` が断続的に発生し、安定動作は未確認
- Codex CLI: 未検証
- OpenAI API: 未検証

## 残課題

### 1. Gemini 系の安定性

- Gemini CLI は headless / trust / 実行ディレクトリの切り分けが残っている
- Gemini API は provider 側の高負荷時に 503 が断続的に発生する

### 2. PO コンテキスト精度

- Claude API は実行自体は成功するが、既存実装済みの DB 設計や一覧・詳細表示を踏まえず、重複 backlog を提案するケースが確認された
- 主因は `build_project_context()` が archived 済みの story / task を含めないことと、`ARCHITECTURE.md` が現状とズレていることにある

### 3. 未完了の動作確認

- Codex CLI と OpenAI API は PO アシスタントの手動検証が未実施
- API モード全体の回帰確認も、Anthropic / Gemini / OpenAI をまたいだ完全な matrix には到達していない

## 主要変更ファイル

- `src-tauri/src/ai.rs`
- `src-tauri/src/rig_provider.rs`
- `src-tauri/src/cli_runner/mod.rs`
- `src-tauri/src/cli_runner/gemini.rs`
- `src/components/ui/GlobalSettingsModal.tsx`
- `docs/42_po_assistant_cli_support/task.md`

## 補足

- Epic 42 の本体実装は完了しているが、provider / transport ごとの信頼性と context 精度の改善は Epic 43 へ持ち越す。
- 特に Gemini 系のデバッグと、Claude API での重複 backlog 防止は次 Epic の主要テーマである。
