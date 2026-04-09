# Epic 41: 他 API プロバイダー対応 (OpenAI + Ollama) 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 40 完了（Phase 1 完了後）
- 作成日: 2026-04-09

## Epic の目的

PO アシスタントの API 選択肢を Anthropic / Gemini の2択から、OpenAI / Ollama を加えた4択に拡張する。特に Ollama 対応によりサブスク不要・完全無料での PO アシスタント利用を可能にする。

## スコープ

### 対象ファイル（変更）
- `src-tauri/src/rig_provider.rs` — AiProvider enum 拡張、OpenAI/Ollama 実装
- `src-tauri/src/llm_observability.rs` — OpenAI pricing 追加、Ollama cost=0 対応
- `src/components/ui/GlobalSettingsModal.tsx` — Provider 選択 UI 拡張
- `src/components/ui/SetupStatusTab.tsx` — OpenAI / Ollama ステータス追加
- `src-tauri/Cargo.toml` — 依存クレート追加（必要に応じて）

### 対象ファイル（新規）
- なし（既存ファイルの拡張で完結）

### 対象外
- Dev エージェント（CLI）側の変更（本 Epic はPOアシスタントのAPIレイヤーのみ）
- PO アシスタントの CLI 対応（Epic 42）

## 実装方針

### 1. AiProvider enum の拡張

```rust
// rig_provider.rs
pub enum AiProvider {
    Anthropic,
    Gemini,
    OpenAI,   // 追加
    Ollama,   // 追加
}

impl AiProvider {
    pub fn from_str(s: &str) -> Self {
        match s {
            "gemini" => AiProvider::Gemini,
            "openai" => AiProvider::OpenAI,
            "ollama" => AiProvider::Ollama,
            _ => AiProvider::Anthropic,
        }
    }
}
```

### 2. OpenAI 実装

Rig の OpenAI provider を使用:

```rust
use rig::providers::openai;

async fn chat_openai(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    mut chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    let client = openai::Client::new(api_key);
    let agent = client.agent(model)
        .preamble(system_prompt)
        .max_tokens(4096)
        .build();
    // ... 既存の Anthropic/Gemini と同パターン
}
```

`resolve_provider_and_key()` 追加分:
```rust
AiProvider::OpenAI => ("openai-api-key", "openai-model", "gpt-4o"),
```

### 3. Ollama 実装

Ollama は OpenAI 互換 API を提供するため、OpenAI クライアントのエンドポイントを上書きして使用する:

```rust
async fn chat_ollama(
    endpoint: &str,  // "http://localhost:11434"
    model: &str,
    system_prompt: &str,
    user_input: &str,
    mut chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    // OpenAI 互換エンドポイントを使用
    // endpoint + "/v1/chat/completions"
    let client = openai::Client::from_url(endpoint, "ollama"); // API key は任意値
    let agent = client.agent(model)
        .preamble(system_prompt)
        .max_tokens(4096)
        .build();
    // ...
}
```

**注意:** Rig の OpenAI provider がカスタムエンドポイント URL をサポートしているか実装時に確認する。サポートしていない場合は `reqwest` で直接 OpenAI 互換 API を呼ぶフォールバック実装が必要。

### 4. Ollama 接続確認

```rust
#[tauri::command]
pub async fn check_ollama_status(app: AppHandle) -> Result<OllamaStatus, String> {
    let endpoint = // settings.json から取得、デフォルト "http://localhost:11434"
    let url = format!("{}/api/tags", endpoint);
    match reqwest::Client::new().get(&url).timeout(Duration::from_secs(3)).send().await {
        Ok(res) => {
            let json: Value = res.json().await?;
            let models = // json["models"] からモデル名を抽出
            Ok(OllamaStatus { running: true, models })
        }
        Err(_) => Ok(OllamaStatus { running: false, models: vec![] })
    }
}
```

### 5. 設定画面の変更

PO アシスタント設定タブの Provider 選択を拡張:

```
現在:  ○ Anthropic  ○ Gemini
変更後: ○ Anthropic  ○ Gemini  ○ OpenAI  ○ Ollama
```

Ollama 選択時は API Key 入力の代わりに:
- エンドポイント URL 入力（デフォルト: `http://localhost:11434`）
- 接続テストボタン（`check_ollama_status` を呼び出し）
- モデル選択（Ollama から取得したモデル一覧）

### 6. Observability 対応

```rust
// llm_observability.rs - pricing 追加
"openai" => match model {
    m if m.starts_with("gpt-4o") => (2.50, 10.00, 0.0, 0.0),
    m if m.starts_with("gpt-4.1") => (2.00, 8.00, 0.0, 0.0),
    _ => (0.0, 0.0, 0.0, 0.0),
},
"ollama" => (0.0, 0.0, 0.0, 0.0), // ローカル LLM は無料
```

## テスト方針

- OpenAI API Key 設定 → `refine_idea` が正常動作すること
- OpenAI で `chat_team_leader_with_tools` (tool calling) が正常動作すること
- Ollama 起動状態で PO アシスタントチャットが応答を返すこと
- Ollama 未起動時に `check_ollama_status` が `running: false` を返すこと
- LLM Usage に正しいプロバイダー名が記録されること
- Ollama 使用時のコストが 0 として記録されること
- 既存の Anthropic / Gemini プロバイダーに影響がないこと（回帰テスト）
