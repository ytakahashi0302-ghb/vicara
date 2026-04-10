# Epic 41 Handoff

## Epic 41 の到達点

Epic 41 は完了済み。PO アシスタントの API レイヤー拡張（Phase 2）はここで完了し、以下の 4 基盤が揃った。

- Anthropic
- Gemini
- OpenAI
- Ollama

これにより、PO アシスタントはクラウド API とローカル LLM の両方を選択肢として持つ状態になった。

## 重要な実装ポイント

### 1. OpenAI と Ollama は別系統ではなく「OpenAI 互換系」として整理した

`src-tauri/src/rig_provider.rs` では、OpenAI は `rig` の OpenAI provider をそのまま使い、Ollama は OpenAI 互換 API として `base_url` を差し替える方式で実装している。

この方針により、以下を共通化できている。

- 会話処理
- tool calling 対応
- モデル取得フローの考え方

次 Epic で CLI 実行系を足す場合も、「API provider」と「CLI transport」を同じ抽象に無理に混ぜるより、実行経路を分けつつ観測や設定を寄せる方が安全である。

### 2. Observability はすでに OpenAI / Ollama を理解している

`src-tauri/src/llm_observability.rs` は以下まで対応済み。

- OpenAI の料金テーブル
- Ollama は常に cost = 0
- provider / model 単位の内訳表示

そのため次 Epic では、CLI 実行経路を PO アシスタントへ追加する場合でも、observability の保存先を新設する必要は薄い。既存の `record_llm_usage` / `record_claude_cli_usage` 系にどう接続するかを主に考えればよい。

### 3. UI は 4 プロバイダー対応済み

`src/components/ui/GlobalSettingsModal.tsx` と `src/components/ui/SetupStatusTab.tsx` はすでに以下へ対応済み。

- 4 プロバイダー選択
- OpenAI の API Key / model 設定
- Ollama の endpoint / model / 接続テスト
- Setup Status での OpenAI / Ollama 状態表示

したがって次 Epic では、PO アシスタントの実行経路を CLI へ広げる際に、設定 UI を全面改修する必要はない。既存 UI に CLI 実行モードをどう統合するか、また API provider 選択とどう共存させるかが主論点になる。

## Epic 42 に向けたコンテキスト

### 1. 次は Phase 3: 全機能の CLI / API 統合

次 Epic 42 では、PO アシスタント機能を API だけでなく CLI 経由でも実行可能にするアーキテクチャ改修を行う。

言い換えると、Epic 41 で API レイヤーの選択肢は広がったが、次は実行 transport そのものを API / CLI の両対応へ広げるフェーズである。

### 2. 既存の CLI 基盤は Epic 39 / 40 側にある

次 Epic の入口として、以下の流れを再確認するとよい。

- Epic 39 / 40 で Team Settings 側の Multi-CLI 基盤が整っている
- Epic 41 で PO アシスタントの API provider 側が 4 基盤になった
- Epic 42 では、この 2 系統を PO アシスタント実行アーキテクチャとしてどう統合するかが主題になる

## 運用ルール

次 Epic でも、以下の運用ルールを厳格に守ること。

- タスクを 1 つ消化するたびに `task.md` のチェックボックスを小まめに更新すること
- まとめて更新しないこと
- ファイル名は `walkthrough.md` に統一すること

## 主要ファイル

- `src-tauri/src/rig_provider.rs`
- `src-tauri/src/llm_observability.rs`
- `src-tauri/src/ai.rs`
- `src/components/ui/GlobalSettingsModal.tsx`
- `src/components/ui/SetupStatusTab.tsx`
- `docs/41_additional_api_providers/task.md`
- `docs/41_additional_api_providers/walkthrough.md`

## 検証メモ

今回確認済みなのは以下である。

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm run build`

未実施なのは、OpenAI API Key と実稼働 Ollama を使った結合確認であり、これは後日の結合テストフェーズで回収する前提で Epic 41 をクローズしている。
