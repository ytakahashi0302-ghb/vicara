# Epic 42: PO アシスタント CLI/API 選択対応 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 41 完了（Phase 2 完了後）
- 作成日: 2026-04-09

## Epic の目的

PO アシスタントを API 従量課金に依存しない形で運用可能にする。CLI サブスクリプション（Claude Max, Google AI Premium 等）を持つユーザーが、追加コストなしで PO アシスタント機能を利用できるようにする。

## スコープ

### 対象ファイル（変更）
- `src-tauri/src/ai.rs` — 各関数に CLI transport 分岐を追加
- `src-tauri/src/rig_provider.rs` — PO transport 設定の解決関数追加
- `src/components/ui/GlobalSettingsModal.tsx` — PO アシスタント transport 設定 UI

### 対象外
- Dev エージェントの API 対応（ファイル操作が必要なため CLI が本命、API 対応は見送り）
- `cli_runner/` モジュール自体の変更（Epic 37-38 で完了済み）
- `frontend-core` 配下（CLAUDE.md ルールに従い変更しない）

## 実装方針

### 1. CLI 1ショット実行の共通関数

Dev エージェント（長時間実行 + ストリーミング）とは異なり、PO アシスタントの CLI 使用は **1ショット実行 → 全出力キャプチャ → JSON パース** パターンになる。

```rust
// ai.rs

async fn execute_po_cli_prompt(
    cli_type: &cli_runner::CliType,
    model: &str,
    prompt: &str,
    cwd: &str,
) -> Result<String, String> {
    let runner = cli_runner::create_runner(cli_type);
    let args = runner.build_args(prompt, model, cwd);

    let output = tokio::process::Command::new(runner.command_name())
        .args(&args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("{} が見つかりません。", runner.command_name())
            } else {
                format!("CLI 実行エラー: {}", e)
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("CLI がエラーで終了しました: {}", stderr));
    }
    Ok(stdout)
}
```

**Dev エージェントとの差異:**
- Dev: PTY + ストリーミング + セッション管理 + タイムアウト (180s) + worktree
- PO: `tokio::process::Command::output()` で全量取得 + JSON パース。60秒タイムアウト。

### 2. 各機能の transport 分岐パターン

全4機能に共通の分岐構造:

```rust
pub async fn refine_idea(
    app: AppHandle,
    // ... 既存パラメータ
) -> Result<RefinedIdeaResponse, String> {
    let transport = resolve_po_transport(&app).await?;

    match transport {
        PoTransport::Api { provider, api_key, model } => {
            // 既存の API 実装（変更なし）
        }
        PoTransport::Cli { cli_type, model, cwd } => {
            let prompt = format!(
                "{}\n\n---\nユーザー入力: {}\n\n必ず以下の JSON 形式で返答してください:\n{}",
                system_prompt, user_input, json_schema_hint
            );
            let raw = execute_po_cli_prompt(&cli_type, &model, &prompt, &cwd).await?;
            parse_json_response::<RefinedIdeaResponse>(&raw)
        }
    }
}
```

### 3. Transport 設定の解決

```rust
enum PoTransport {
    Api {
        provider: AiProvider,
        api_key: String,
        model: String,
    },
    Cli {
        cli_type: cli_runner::CliType,
        model: String,
        cwd: String,
    },
}

async fn resolve_po_transport(app: &AppHandle) -> Result<PoTransport, String> {
    let store = app.store("settings.json")?;
    let transport_kind = store.get("po-assistant-transport")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or("api".to_string());

    match transport_kind.as_str() {
        "cli" => {
            let cli_type_str = store.get("po-assistant-cli-type")...;
            let model = store.get("po-assistant-cli-model")...;
            let cwd = // プロジェクトの local_path を取得
            Ok(PoTransport::Cli { ... })
        }
        _ => {
            let (provider, api_key, model) = resolve_provider_and_key(app, None).await?;
            Ok(PoTransport::Api { provider, api_key, model })
        }
    }
}
```

### 4. chat_inception の会話履歴シリアライズ

CLI は 1ショット実行のため、会話履歴をプロンプト内に含める必要がある:

```rust
fn serialize_chat_history(messages: &[Message]) -> String {
    messages.iter().map(|m| {
        match m.role.as_str() {
            "user" => format!("## ユーザー\n{}\n", m.content),
            "assistant" => format!("## アシスタント\n{}\n", m.content),
            _ => String::new(),
        }
    }).collect::<Vec<_>>().join("\n")
}
```

サブスク定額のためトークン量増加はコスト問題にならない。

### 5. team_leader の CLI 対応

既存の `execute_fallback_team_leader_plan()` がまさにこのパターンの実装。CLI transport 時はフォールバックではなくメインフローとしてこのパターンを使用:

1. CLI に「JSON で計画を返して」とプロンプト
2. `PoAssistantExecutionPlan` をパース
3. `insert_story_with_tasks()` で DB 操作
4. `kanban-updated` イベント emit

### 6. 設定 UI

```
PO アシスタント設定
┌─────────────────────────────────────┐
│ 実行方式                             │
│   ○ API（従量課金）                   │
│   ○ CLI（サブスク / ローカル）         │
│                                     │
│ [CLI 選択時]                         │
│   CLI: [Claude Code ▾]              │
│   モデル: [claude-sonnet-4 ▾]        │
│                                     │
│ [API 選択時]                         │
│   Provider: ○ Anthropic ○ Gemini    │
│             ○ OpenAI   ○ Ollama     │
│   ... (既存の API 設定)              │
└─────────────────────────────────────┘
```

## テスト方針

- API モード → 全4機能が従来通り動作（回帰テスト最重要）
- CLI モード + Claude Code → refine_idea が RefinedIdeaResponse を返すこと
- CLI モード → generate_tasks が GeneratedTask 配列を返すこと
- CLI モード → chat_inception がフェーズ進行し、patch_target/patch_content を返すこと
- CLI モード → team_leader が JSON 計画を受け取り、DB にストーリー・タスクを登録すること
- CLI 未インストール時 → 明確なエラーメッセージが返ること
- transport 設定の保存・読み込みが正しく動作すること
