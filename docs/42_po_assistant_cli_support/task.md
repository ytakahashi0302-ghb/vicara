# Epic 42: PO アシスタント CLI/API 選択対応 タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: Epic 41 完了（Phase 2 完了後）
- 作成日: 2026-04-09

## 概要

PO アシスタントの全4機能（refine_idea, generate_tasks, chat_inception, team_leader）を CLI 経由でも実行できるようにする。ユーザーは PO アシスタントの実行方式として API または CLI を選択できるようになる。

## 実行順序

### 1. PO アシスタント用 CLI 実行基盤の実装
- [ ] `src-tauri/src/ai.rs` に CLI 経由でプロンプトを実行し JSON レスポンスを取得する共通関数 `execute_po_cli_prompt()` を追加する。
- [ ] この関数は以下を行う:
  - CLI プロセスを起動（Dev エージェントと異なり 1ショット実行）
  - stdout を全量キャプチャ
  - 出力から JSON を `parse_json_response()` で抽出
- [ ] Dev エージェントの `cli_runner` モジュールの `CliRunner` trait を再利用する。

### 2. refine_idea の CLI 対応
- [ ] `refine_idea()` 関数に transport 分岐を追加する。
- [ ] CLI モード: システムプロンプト + ユーザー入力を1つのプロンプトにまとめて CLI に渡す。
- [ ] CLI 出力から `RefinedIdeaResponse` JSON をパースする。
- [ ] 既存の API モードは変更しない。

### 3. generate_tasks_from_story の CLI 対応
- [ ] `generate_tasks_from_story()` 関数に transport 分岐を追加する。
- [ ] CLI モード: ストーリー情報をプロンプトに含めて CLI に渡す。
- [ ] CLI 出力から `Vec<GeneratedTask>` JSON をパースする。

### 4. chat_inception の CLI 対応
- [ ] `chat_inception()` 関数に transport 分岐を追加する。
- [ ] CLI モード: 会話履歴全体をプロンプトにシリアライズして CLI に渡す。
- [ ] CLI 出力から `ChatInceptionResponse` JSON をパースする。
- [ ] 会話履歴のシリアライズ形式を定義する（Markdown 形式を推奨）。

### 5. chat_with_team_leader の CLI 対応
- [ ] `chat_with_team_leader()` 関数に transport 分岐を追加する。
- [ ] CLI モード: 既存の `execute_fallback_team_leader_plan()` パターンを流用する。
  - CLI に JSON 形式の実行計画を返させる
  - アプリ側で JSON をパースして DB 操作を実行
- [ ] Tool calling は使用しない（CLI では不可のため）。

### 6. PO アシスタント transport 設定の追加
- [ ] settings.json に `po-assistant-transport` キーを追加する（`"api"` | `"cli"`）。
- [ ] CLI 選択時に使用する CLI 種別とモデルの設定キーを追加する:
  - `po-assistant-cli-type` (デフォルト: `"claude"`)
  - `po-assistant-cli-model` (デフォルト: CLI 種別に応じたデフォルトモデル)

### 7. 設定画面の更新
- [ ] `GlobalSettingsModal.tsx` の PO アシスタント設定タブに「実行方式」セクションを追加する。
- [ ] ラジオボタン: `API` / `CLI`
- [ ] CLI 選択時: CLI 種別ドロップダウン + モデル入力を表示する。
- [ ] API 選択時: 既存の Provider 選択を表示する（変更なし）。

### 8. 動作確認
- [ ] API モードで全4機能が従来通り動作すること（回帰テスト）。
- [ ] CLI モードで `refine_idea` が JSON レスポンスを返すこと。
- [ ] CLI モードで `generate_tasks` がタスクリストを返すこと。
- [ ] CLI モードで `chat_inception` がフェーズ進行できること。
- [ ] CLI モードで `team_leader` がストーリー・タスクを DB に登録できること。
