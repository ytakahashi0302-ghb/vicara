# Epic 41: 他 API プロバイダー対応 (OpenAI + Ollama) タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: Epic 40 完了（Phase 1 完了後）
- 作成日: 2026-04-09

## 概要

PO アシスタントの API プロバイダーに OpenAI と Ollama（ローカル LLM）を追加する。これにより、Anthropic / Gemini に加え、ChatGPT API やローカル LLM をPO アシスタントのバックエンドとして選択可能になる。

## 実行順序

### 1. OpenAI プロバイダーの追加（バックエンド）
- [ ] `src-tauri/src/rig_provider.rs` の `AiProvider` enum に `OpenAI` を追加する。
- [ ] `chat_openai()` 関数を実装する（Rig の OpenAI provider を使用）。
- [ ] `resolve_provider_and_key()` に OpenAI 分岐を追加する。
  - store キー: `openai-api-key`, `openai-model`
  - デフォルトモデル: `gpt-4o`
- [ ] `chat_with_history()` に OpenAI 分岐を追加する。
- [ ] `chat_team_leader_with_tools()` に OpenAI 分岐を追加する（tool calling 対応）。
- [ ] `get_available_models()` に OpenAI 分岐を追加する（OpenAI models API 呼び出し）。

### 2. Ollama プロバイダーの追加（バックエンド）
- [ ] `AiProvider` enum に `Ollama` を追加する。
- [ ] Ollama は OpenAI 互換 API を公開しているため、OpenAI provider のエンドポイント URL を差し替える方式で実装する。
- [ ] `resolve_provider_and_key()` に Ollama 分岐を追加する。
  - store キー: `ollama-endpoint`（デフォルト: `http://localhost:11434`）, `ollama-model`
  - API キーは不要（空文字で可）
- [ ] Ollama 接続確認コマンド `check_ollama_status` を追加する（`/api/tags` エンドポイントに GET）。
- [ ] Ollama からモデル一覧を取得する処理を `get_available_models()` に追加する。

### 3. 設定画面の更新（PO アシスタント設定タブ）
- [ ] `src/components/ui/GlobalSettingsModal.tsx` の PO アシスタント設定タブに OpenAI / Ollama セクションを追加する。
- [ ] Provider 選択ラジオボタンを拡張: Anthropic / Gemini / OpenAI / Ollama
- [ ] OpenAI セクション: API Key 入力 + モデル選択
- [ ] Ollama セクション: エンドポイント URL 入力 + モデル選択 + 接続テストボタン

### 4. セットアップ状況タブへの反映
- [ ] `SetupStatusTab.tsx`（Epic 39）の API キーセクションに OpenAI を追加する。
- [ ] Ollama の稼働状態（接続成功/失敗）を表示する。

### 5. LLM Observability 対応
- [ ] `llm_observability.rs` の cost 計算に OpenAI モデルの料金情報を追加する。
- [ ] Ollama は無料のため、cost = 0 として記録する。

### 6. Cargo.toml の依存関係
- [ ] `rig-core` の OpenAI feature が有効になっていることを確認する（または必要なクレートを追加する）。

### 7. 動作確認
- [ ] OpenAI API Key 設定 → PO アシスタントで ChatGPT 応答が返ること。
- [ ] Ollama 起動済み環境 → PO アシスタントでローカル LLM 応答が返ること。
- [ ] 各プロバイダーでモデル一覧取得が動作すること。
- [ ] Team Leader の tool calling が OpenAI で動作すること。
- [ ] LLM 使用量に正しいプロバイダー名とコストが記録されること。
