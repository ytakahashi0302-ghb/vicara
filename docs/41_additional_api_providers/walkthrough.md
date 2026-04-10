# Epic 41: 他 API プロバイダー対応 (OpenAI + Ollama) Walkthrough

## 概要

Epic 41 では、PO アシスタントの API レイヤーを Anthropic / Gemini の 2 基盤から、OpenAI / Ollama を加えた 4 基盤へ拡張した。これにより、クラウド API とローカル LLM の両方を PO アシスタントの思考エンジンとして選択できる状態が整った。

## 1. バックエンドの拡張

### 1-1. OpenAI / Ollama を同じ系統で扱うアーキテクチャ

実装の中心は `src-tauri/src/rig_provider.rs` である。

- `AiProvider` enum に `OpenAI` と `Ollama` を追加した。
- `resolve_provider_and_key()` を拡張し、`openai-api-key` / `openai-model`、`ollama-endpoint` / `ollama-model` を解決できるようにした。
- OpenAI は `rig::providers::openai` の Completions API クライアントを使って実装した。
- Ollama は別実装を増やさず、同じ OpenAI 互換クライアントに対して `base_url` を差し替える形で接続した。

この構成により、OpenAI と Ollama で以下の利点が得られた。

- `chat_with_history()` と `chat_team_leader_with_tools()` の分岐を最小限に抑えられる。
- Ollama 側でも OpenAI 互換の tool calling 経路を流用できる。
- OpenAI 系のプロバイダー追加時に、共通ヘルパーを再利用しやすい。

具体的には、OpenAI 用の completion client 構築と、Ollama 用の completion client 構築を分けたうえで、実際の会話処理は共通の OpenAI 互換チャット関数へ集約した。

### 1-2. Ollama の稼働確認とモデル取得

`check_ollama_status` コマンドを追加し、`/api/tags` を使って以下を返せるようにした。

- 稼働中かどうか
- 検出したモデル一覧
- 実際に接続を試みた endpoint
- 失敗時のメッセージ

また `get_available_models()` にも Ollama 分岐を追加し、接続確認と同じ情報源からモデル一覧を引けるようにした。フロントエンドからは未保存の endpoint 値を override できるため、保存前の接続テストにも対応している。

## 2. Observability の統合

### 2-1. OpenAI 料金テーブルの追加

`src-tauri/src/llm_observability.rs` の `resolve_pricing()` を拡張し、OpenAI 系モデルの料金テーブルを追加した。

- `gpt-5.x`
- `gpt-4.1`
- `gpt-4o`
- `o3`
- `o4-mini`

これにより、PO アシスタント経由で OpenAI を使った場合も、既存の LLM 使用量記録基盤へ自然に乗るようになった。

### 2-2. Ollama は完全無料として扱う設計

Ollama はローカル実行であり API 利用料金が発生しないため、`resolve_pricing()` で `provider == ollama` を検出した場合は常に `PricingSnapshot::zero()` を返すようにした。

この設計により、Ollama 利用時は以下が保証される。

- token 使用量は記録される
- provider / model 名も記録される
- estimated cost は常に 0

あわせて `AnalyticsTab.tsx` の表示条件を調整し、コストが 0 でも token が記録されていれば OpenAI / Ollama の内訳が見えるようにした。

## 3. UI の拡張

### 3-1. 4 プロバイダー選択

`src/components/ui/GlobalSettingsModal.tsx` の PO アシスタント設定タブを拡張し、既定プロバイダーの選択肢を以下の 4 つに広げた。

- Anthropic
- Gemini
- OpenAI
- Ollama

既存のカード UI を踏襲しつつ、OpenAI は API Key ベース、Ollama はローカル稼働ベースという違いが視覚的に分かるよう、バッジ文言も分けている。

### 3-2. OpenAI / Ollama の設定項目

OpenAI 選択時は以下を追加した。

- API Key 入力
- モデル入力またはモデル一覧選択

Ollama 選択時は以下を追加した。

- endpoint URL 入力
- 接続テストボタン
- モデル入力またはモデル一覧選択

特に Ollama は「保存してから試す」では UX が悪いため、未保存の endpoint をそのまま `check_ollama_status` と `get_available_models()` に渡せるようにし、その場で疎通確認できる流れにした。

### 3-3. Setup Status への反映

`src/components/ui/SetupStatusTab.tsx` では以下を反映した。

- API キー一覧に OpenAI を追加
- Ollama は API キーではなく「ローカル LLM」セクションとして独立表示
- 稼働中 / 未稼働、検出モデル数、失敗メッセージを表示

これにより、クラウド API とローカルランタイムの状態を意味に応じて分けて表示できるようになった。

## 4. 検証結果

今回の実装では以下を確認した。

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm run build`

OpenAI API Key および稼働中 Ollama を用いた実機確認は、現在のテスト環境では行えないため、本 Epic ではスキップし、後日の結合テストフェーズへ持ち越した。

## 5. 運用メモ

- `task.md` は、バックエンド完了、Observability 完了、UI 完了の節目ごとに都度更新した。
- 命名ルールは `walkthrough.md` に統一し、表記ゆれを避けた。
- Epic 41 完了時点で、PO アシスタントの API レイヤーは 4 基盤対応になっている。
