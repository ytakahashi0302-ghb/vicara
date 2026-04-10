# Epic 42: PO アシスタント CLI/API 選択対応 タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: Epic 41 完了（Phase 2 完了後）
- 作成日: 2026-04-09

## 概要

PO アシスタントの全4機能（refine_idea, generate_tasks, chat_inception, team_leader）を CLI 経由でも実行できるようにする。ユーザーは PO アシスタントの実行方式として API または CLI を選択できるようになる。

## 実行順序

### 1. PO アシスタント用 CLI 実行基盤の実装
- [x] `src-tauri/src/ai.rs` に CLI 経由でプロンプトを実行し JSON レスポンスを取得する共通関数 `execute_po_cli_prompt()` を追加する。
- [x] この関数は以下を行う:
  - CLI プロセスを起動（Dev エージェントと異なり 1ショット実行）
  - stdout を全量キャプチャ
  - 出力から JSON を `parse_json_response()` で抽出
- [x] Dev エージェントの `cli_runner` モジュールの `CliRunner` trait を再利用する。
- [x] Windows の Gemini CLI では短い `--prompt` と stdin を併用し、長文 prompt を引数展開せずに headless モードを維持する。
- [x] Gemini CLI はアプリ設定の API キー注入に依存せず、既存ログイン状態で実行する。
- [x] Gemini CLI では未 trust の project local path を直接 `cwd` に使わず、trust 済みフォルダへフォールバックして起動する。

### 2. refine_idea の CLI 対応
- [x] `refine_idea()` 関数に transport 分岐を追加する。
- [x] CLI モード: システムプロンプト + ユーザー入力を1つのプロンプトにまとめて CLI に渡す。
- [x] CLI 出力から `RefinedIdeaResponse` JSON をパースする。
- [x] 既存の API モードは変更しない。

### 3. generate_tasks_from_story の CLI 対応
- [x] `generate_tasks_from_story()` 関数に transport 分岐を追加する。
- [x] CLI モード: ストーリー情報をプロンプトに含めて CLI に渡す。
- [x] CLI 出力から `Vec<GeneratedTask>` JSON をパースする。

### 4. chat_inception の CLI 対応
- [x] `chat_inception()` 関数に transport 分岐を追加する。
- [x] CLI モード: 会話履歴全体をプロンプトにシリアライズして CLI に渡す。
- [x] CLI 出力から `ChatInceptionResponse` JSON をパースする。
- [x] 会話履歴のシリアライズ形式を定義する（Markdown 形式を推奨）。

### 5. chat_with_team_leader の CLI 対応
- [x] `chat_with_team_leader()` 関数に transport 分岐を追加する。
- [x] CLI モード: 既存の `execute_fallback_team_leader_plan()` パターンを流用する。
  - CLI に JSON 形式の実行計画を返させる
  - アプリ側で JSON をパースして DB 操作を実行
- [x] Tool calling は使用しない（CLI では不可のため）。
- [x] team_leader の汎用バックログ作成要求で PRODUCT_CONTEXT を踏まえた具体案を生成する。
- [x] API provider の最終応答失敗後も、DB 更新済みなら部分成功として返答する。
- [x] API provider の一時的な 503 では再試行し、未反映なら通常チャット返信として失敗を返す。

### 6. PO アシスタント transport 設定の追加
- [x] settings.json に `po-assistant-transport` キーを追加する（`"api"` | `"cli"`）。
- [x] CLI 選択時に使用する CLI 種別とモデルの設定キーを追加する:
  - `po-assistant-cli-type` (デフォルト: `"claude"`)
  - `po-assistant-cli-model` (デフォルト: CLI 種別に応じたデフォルトモデル)

### 7. 設定画面の更新
- [x] `GlobalSettingsModal.tsx` の PO アシスタント設定タブに「実行方式」セクションを追加する。
- [x] ラジオボタン: `API` / `CLI`
- [x] CLI 選択時: CLI 種別ドロップダウン + モデル入力を表示する。
- [x] API 選択時: 既存の Provider 選択を表示する（変更なし）。

### 8. 動作確認
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` が成功すること。
- [x] `npm run build` が成功すること。
- [x] Claude CLI で `chat_with_team_leader` による backlog 作成が成功すること。
- [x] Claude API で `chat_with_team_leader` による backlog 作成が成功すること。
- [ ] Claude API で既存実装済み機能と重複しない backlog を安定して提案できること。
- [ ] API モードで全4機能が従来通り動作すること（Anthropic / Gemini / OpenAI の実運用回帰テスト）。
- [ ] Gemini CLI で `refine_idea` / `generate_tasks` / `chat_inception` / `team_leader` が安定して動作すること。
- [ ] Gemini API で `chat_with_team_leader` が 503 に左右されず安定動作すること。
- [ ] Codex CLI モードで `refine_idea` と `chat_with_team_leader` の基本動作を確認すること。
- [ ] OpenAI API モードで `refine_idea` と `chat_with_team_leader` の基本動作を確認すること。

### 9. クローズアウト
- [x] `walkthrough.md` を作成し、Epic 42 の実装内容・検証結果・残課題を記録すること。
- [x] `handoff.md` を作成し、Epic 43 へ引き継ぐべき観測結果と未解決事項を整理すること。
