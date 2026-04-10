# Epic 42 Handoff

## Epic 42 の到達点

Epic 42 では、PO アシスタントに API / CLI の transport 選択を導入し、以下の 4 機能を transport 分岐対応にした。

- `refine_idea`
- `generate_tasks_from_story`
- `chat_inception`
- `chat_with_team_leader`

また、設定画面では PO アシスタント用に `API / CLI` の実行方式切り替え、CLI 種別、モデル設定を追加した。これにより、PO アシスタントは Anthropic / Gemini / OpenAI / Ollama の API 基盤と、Claude / Gemini / Codex の CLI 基盤を選べる状態になっている。

## この Epic で重要だった実装ポイント

### 1. PO アシスタントの CLI は Dev エージェントと異なる

PO アシスタントの CLI 実行は PTY やセッション管理ではなく、`execute_po_cli_prompt()` による 1 ショット実行である。

- CLI を起動
- stdout を全量キャプチャ
- JSON を抽出してアプリ側でパース

このため、Dev エージェントの長時間ストリーミング基盤とは別系統で見た方が分かりやすい。

### 2. Team Leader の CLI は tool calling ではなく JSON 計画実行

CLI モードの `chat_with_team_leader` は、モデルに JSON の実行計画を返させ、アプリ側で `create_story_and_tasks` 相当の DB 操作を実行する設計にしている。

このパターンは、将来 CLI provider を増やしても共通化しやすい一方で、モデルが文脈を読み違えた場合に重複 backlog をそのまま登録しやすい。Epic 43 ではこの点のガードが必要になる。

### 3. API 障害時の扱いを少し強くした

Epic 42 では以下を追加している。

- provider 最終応答失敗後でも DB 更新済みなら部分成功として返す
- Gemini API の 503 / `UNAVAILABLE` は再試行し、未反映なら通常返信として扱う

ただし、Gemini API の安定性自体はまだ不十分であり、ここは Epic 43 へ持ち越しである。

## 2026-04-10 時点の動作確認状況

- Claude CLI: `○`
- Claude API: `○`
- Gemini CLI: `×`
- Gemini API: `×`
- Codex CLI: `?`
- OpenAI API: `?`

補足:

- Claude API は実行自体は成功するが、既存実装済みの DB 設計や一覧・詳細表示機能を踏まえず、重複 backlog を作るケースがある
- Gemini CLI は timeout、Gemini API は 503 `UNAVAILABLE` が継続している

## 次 Epic 43 で優先すべきこと

### 1. Gemini CLI / Gemini API の reliability 改善

- Gemini CLI の headless 実行で、timeout 時に何が起きているかを UI / ログから分かるようにする
- Gemini API の 503 を、tool 実行前失敗・後失敗・部分成功に分けて扱う

### 2. Claude API の context 精度改善

今回の調査で、重複 backlog 提案の主因は transport ではなく context にあることが分かっている。

- `build_project_context()` が `archived = 0` の story / task しか渡していない
- Done task が archive 済みになるため、完了した実装事実が PO に見えない
- `ARCHITECTURE.md` が PostgreSQL 前提のままで、SQLite 移行の現状とズレている
- `create_story_and_tasks` に重複 story 防止ガードがない

Epic 43 では、context の改善と tool 側ガードをセットで行うのがよい。

### 3. 未検証 provider の回収

- Codex CLI
- OpenAI API

この 2 つは未検証のまま Epic 42 を閉じるため、Epic 43 の冒頭で matrix を埋めること。

## 主要ファイル

- `src-tauri/src/ai.rs`
- `src-tauri/src/rig_provider.rs`
- `src-tauri/src/cli_runner/mod.rs`
- `src-tauri/src/cli_runner/gemini.rs`
- `src/components/ui/GlobalSettingsModal.tsx`
- `docs/42_po_assistant_cli_support/task.md`
- `docs/42_po_assistant_cli_support/walkthrough.md`
- `docs/43_transport_layer_unification/implementation_plan.md`
- `docs/43_transport_layer_unification/task.md`

## 運用ルール

次 Epic でも以下を厳守すること。

- タスクを 1 つ消化するたびに `task.md` をその場で更新する
- まとめて更新しない
- 修正内容確認ファイル名は `walkthrough.md` に統一する
