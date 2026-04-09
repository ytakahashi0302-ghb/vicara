# Epic 37: CLI Runner 抽象化レイヤー + DB 拡張 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 36 完了
- 作成日: 2026-04-09

## Epic の目的

`claude_runner.rs` は現在 Claude Code CLI にハードコードされており、他 CLI（Gemini CLI, Codex CLI）を追加するには大規模なコピペが必要になる。本 Epic では `CliRunner` trait を導入して CLI 固有ロジックを分離し、新しい CLI を「プラグイン的に」追加できるアーキテクチャに移行する。

## スコープ

### 対象ファイル（新規）
- `src-tauri/src/cli_runner/mod.rs` — trait 定義、CliType enum、ファクトリ
- `src-tauri/src/cli_runner/claude.rs` — ClaudeRunner 実装
- `src-tauri/migrations/XX_cli_type_support.sql` — cli_type カラム追加

### 対象ファイル（変更）
- `src-tauri/src/claude_runner.rs` — 共通ロジックの汎用化、CLI 固有部分の抽出
- `src-tauri/src/db.rs` — TeamRole 構造体 + クエリに cli_type 追加
- `src-tauri/src/lib.rs` — 新モジュール登録
- `src/types/index.ts` — TeamRoleSetting に cli_type 追加

### 対象外
- Gemini CLI / Codex CLI の Runner 実装（Epic 38）
- 設定 UI の変更（Epic 39, 40）
- `frontend-core` 配下の Context / Hooks（CLAUDE.md のルールに従い変更しない）

## 実装方針

### 1. CliRunner trait 設計

```rust
// cli_runner/mod.rs

pub mod claude;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CliType {
    Claude,
    Gemini,
    Codex,
}

impl CliType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "gemini" => CliType::Gemini,
            "codex" => CliType::Codex,
            _ => CliType::Claude,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            CliType::Claude => "claude",
            CliType::Gemini => "gemini",
            CliType::Codex => "codex",
        }
    }
}

pub trait CliRunner: Send + Sync {
    /// CLI 実行コマンド名
    fn command_name(&self) -> &str;

    /// プロンプト実行用の引数リストを構築
    fn build_args(&self, prompt: &str, model: &str, cwd: &str) -> Vec<String>;

    /// 環境変数の追加（必要な場合）
    fn env_vars(&self) -> Vec<(String, String)> { vec![] }
}

pub fn create_runner(cli_type: &CliType) -> Box<dyn CliRunner> {
    match cli_type {
        CliType::Claude => Box::new(claude::ClaudeRunner),
        CliType::Gemini => unimplemented!("Epic 38 で実装"),
        CliType::Codex => unimplemented!("Epic 38 で実装"),
    }
}
```

### 2. ClaudeRunner 実装

`claude_runner.rs` の L548-575 (Windows) と L696-742 (Unix) から CLI 固有のコマンド構築ロジックを抽出:

```rust
// cli_runner/claude.rs

pub struct ClaudeRunner;

impl CliRunner for ClaudeRunner {
    fn command_name(&self) -> &str { "claude" }

    fn build_args(&self, prompt: &str, model: &str, cwd: &str) -> Vec<String> {
        vec![
            "-p".to_string(),
            prompt.to_string(),
            "--model".to_string(),
            model.to_string(),
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
            "--add-dir".to_string(),
            cwd.to_string(),
            "--verbose".to_string(),
        ]
    }
}
```

### 3. claude_runner.rs の汎用化

主な変更点:
- `ClaudeState` → `AgentState`（内部名のみ、Tauri コマンド名は互換性維持）
- `ClaudeSession` → `AgentSession`
- `spawn_claude_process` の内部で `CliRunner::command_name()` + `CliRunner::build_args()` を使用
- Windows/Unix 分岐のプロセス起動は共通基盤として維持（PTY は Unix 全 CLI 共通）

**重要:** Tauri コマンド名 `execute_claude_task` はフロントエンドとの互換性のため本 Epic では変更しない。内部ロジックのみリファクタリングする。

### 4. DB マイグレーション

```sql
-- XX_cli_type_support.sql
ALTER TABLE team_roles ADD COLUMN cli_type TEXT NOT NULL DEFAULT 'claude';
```

デフォルト値 `'claude'` により、既存データはマイグレーション適用だけで自動的に Claude に設定される。

### 5. db.rs の変更

```rust
pub struct TeamRole {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub cli_type: String,        // 追加
    pub model: String,
    pub avatar_image: Option<String>,
    pub sort_order: i32,
}
```

`get_team_configuration` の SELECT 句と `save_team_configuration` の INSERT 句に `cli_type` を追加する。

### 6. タスク実行フローの変更

```rust
// claude_runner.rs 内 execute_claude_task()
let role = db::get_team_role_by_id(&app_handle, &role_id).await?;
let cli_type = CliType::from_str(&role.cli_type);
let runner = cli_runner::create_runner(&cli_type);
// runner を spawn_process に渡す
```

## テスト方針

- マイグレーション後、`SELECT cli_type FROM team_roles` が全行 `'claude'` を返すこと
- 既存の Claude CLI タスク実行フローが変更後も正常に動作すること（回帰テスト）
- `create_runner(CliType::Claude)` が `ClaudeRunner` を返し、正しい引数を構築すること
- `TeamConfiguration` API レスポンスに `cli_type` が含まれること
