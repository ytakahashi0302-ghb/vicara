# MicroScrum AI: Rig + PTY 次世代エージェント基盤 PoC 実装計画

## Context

現在の `ai.rs` は reqwest による生の HTTP リクエストで Anthropic/Gemini API を叩いており、コンテキスト保持・ツール統合・エージェントロジックの拡張が困難。また、Devエージェントがローカル環境でコマンドを実行する基盤が存在しない。

本計画では、**Rig (rig-core)** によるPOエージェント基盤と、**portable-pty** によるDevエージェント基盤の2つを独立したトラックとして構築する。フロントエンド(React)には一切触れない。

---

## 依存関係グラフ

```
Phase 0 (準備・互換性検証)
    |
    +---> Track A: Phase 1 (Rig抽象化層) --> Phase 2 (AI関数の段階的移行)
    |
    +---> Track B: Phase 3 (PTYモジュール) --> Phase 4 (Tauriコマンド公開)
    |
    +---> Phase 5 (統合・堅牢化) [両トラック完了後]
```

Track A と Track B は完全に独立。並行開発可能。

---

## Phase 0: 準備（コード変更なし）

**目的:** 依存関係の互換性を検証し、技術選定を確定する。

1. `cargo add rig-core --dry-run` で reqwest バージョン競合を確認
   - rig-core 0.33.0 は reqwest ^0.13 を使用 → 現行 0.13.2 と互換性あり（要実機確認）
2. PTYクレートの選定確定
   - **推奨: `portable-pty` 0.9.0 を直接使用**（tauri-plugin-pty はフロントUI前提のため過剰）
3. フィーチャーブランチを作成

**検証:** `cargo check` が既存コードで通ること

---

## Phase 1: Rig プロバイダ抽象化層の構築

**目的:** rig-core を導入し、既存コードを壊さずに新しいプロバイダ層を作成する。

### 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src-tauri/Cargo.toml` | `rig-core` を追加（anthropic, gemini フィーチャー） |
| `src-tauri/src/rig_provider.rs` | **新規作成** |
| `src-tauri/src/lib.rs` | `mod rig_provider;` を追加 |

### `rig_provider.rs` の設計

```rust
// 1. プロバイダ列挙型（文字列比較を型安全に）
pub enum AiProvider { Anthropic, Gemini }

// 2. APIキー・プロバイダ解決（既存 get_api_key_and_provider のリファクタ）
pub async fn resolve_provider_and_key(
    app: &AppHandle, override: Option<String>
) -> Result<(AiProvider, String), String>

// 3. Rigエージェント構築
pub fn build_agent(
    provider: &AiProvider, api_key: &str, system_prompt: &str
) -> Result<impl Agent, String>
// Anthropic: ClientBuilder::new(key).anthropic_version("2023-06-01").build()
//   → .agent("claude-haiku-4-5-20251001").preamble(prompt).build()
// Gemini: Client::new(key)
//   → .agent("gemini-2.0-flash").preamble(prompt).build()

// 4. メッセージ変換（app の Message → Rig の Message 型）
pub fn convert_messages(messages: &[crate::ai::Message]) -> Vec<RigMessage>

// 5. 統一的なLLM呼び出し
pub async fn complete(agent, user_input: &str, history: &[RigMessage]) -> Result<String, String>
```

**検証:** `cargo check` 成功。既存の invoke コマンドは未変更。

---

## Phase 2: AI関数の段階的移行

**目的:** ai.rs の4関数を1つずつ Rig 経由に置き換える。各サブフェーズ単独でデプロイ可能。

### 移行順序（リスク昇順）

| サブフェーズ | 関数 | 理由 |
|-------------|------|------|
| 2a | `generate_tasks_from_story` | 単発呼び出し。会話履歴なし。最もシンプル |
| 2b | `chat_with_team_leader` | 会話履歴あり + 日本語システムプロンプト（i18n検証） |
| 2c | `refine_idea` | `Option<Vec<Message>>` の変換パターン |
| 2d | `chat_inception` | 構造化レスポンス（`is_finished`, `generated_document`） |

### 各サブフェーズの共通パターン

```
変更前: reqwest::Client::new() → provider分岐 → HTTP POST → JSON抽出
変更後: rig_provider::build_agent() → rig_provider::complete() → 同じJSON抽出
```

**重要:** コマンドの関数シグネチャ（引数・戻り値）は一切変更しない。フロントの `invoke()` 呼び出しとの互換性を維持する。

レスポンスの正規表現JSON抽出（`(?s)\{.*?\}` 等）はそのまま残す。Rig はプレーンテキストを返すので、後処理ロジックは不変。

### Phase 2e: クリーンアップ

- `get_api_key_and_provider` ヘルパー関数を削除（`rig_provider` に移行済み）
- `use reqwest::Client;` を ai.rs から削除（rig-core が内部で使用）
- `context_md` の未使用変数を整理（`refine_idea`, `chat_inception`, `chat_with_team_leader` で計算されるが未使用）

**検証:** 全4コマンドが Anthropic / Gemini で動作。`cargo clippy` クリーン。

---

## Phase 3: PTY モジュール基盤の構築（Track B・Phase 1-2 と独立）

**目的:** portable-pty を導入し、コマンド実行・出力取得の基盤を作る。

### 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src-tauri/Cargo.toml` | `portable-pty = "0.9.0"` を追加 |
| `src-tauri/src/pty_manager.rs` | **新規作成** |
| `src-tauri/src/lib.rs` | `mod pty_manager;` を追加 |

### `pty_manager.rs` の設計

```rust
pub struct PtySession {
    id: String,               // UUID
    child: Box<dyn Child + Send>,
    reader: Box<dyn Read + Send>,
    writer: Box<dyn Write + Send>,
    cwd: PathBuf,
    last_activity: Instant,
}

pub struct PtyManager {
    sessions: Arc<tokio::sync::Mutex<HashMap<String, PtySession>>>,
}

impl PtyManager {
    pub fn new() -> Self;

    // セッション生成（シェルを起動）
    pub async fn spawn_session(&self, cwd: &str) -> Result<String, String>;

    // コマンド実行＋出力取得
    pub async fn execute_command(
        &self, session_id: &str, command: &str
    ) -> Result<String, String>;

    // セッション終了
    pub async fn kill_session(&self, session_id: &str) -> Result<(), String>;
}
```

### Windows ConPTY 対策（最重要の技術課題）

Windows の ConPTY はプロセス終了時に EOF を送信しない → ブロッキング read がハングする。

**対策: センチネル方式**
```
実行コマンド: "{command} && echo __DONE_{uuid}__"
読み取り: spawn_blocking 内でセンチネル文字列が出現するまでループ
タイムアウト: 30秒のハードタイムアウトをフォールバックとして設定
```

### プラットフォーム分岐

```rust
#[cfg(target_os = "windows")]
fn default_shell() -> CommandBuilder { CommandBuilder::new("cmd.exe") }

#[cfg(not(target_os = "windows"))]
fn default_shell() -> CommandBuilder {
    CommandBuilder::new(std::env::var("SHELL").unwrap_or("/bin/bash".into()))
}
```

**検証:** `echo hello` の実行と出力取得。Windows + Unix 両方で動作。

---

## Phase 4: PTY の Tauri コマンド公開

**目的:** PTY操作をフロントエンドから invoke 可能にする。

### 変更ファイル

| ファイル | 変更内容 |
|---------|---------|
| `src-tauri/src/pty_commands.rs` | **新規作成** |
| `src-tauri/src/lib.rs` | `mod pty_commands;` + `.manage(PtyManager::new())` + invoke_handler 追加 |

### Tauri コマンド

```rust
#[tauri::command] pub async fn pty_spawn(state, cwd) -> Result<String, String>
#[tauri::command] pub async fn pty_execute(state, session_id, command) -> Result<String, String>
#[tauri::command] pub async fn pty_kill(state, session_id) -> Result<(), String>
```

### lib.rs への統合

```rust
// Builder チェーンに追加
.manage(pty_manager::PtyManager::new())

// invoke_handler に追加
pty_commands::pty_spawn,
pty_commands::pty_execute,
pty_commands::pty_kill,
```

**検証:** `invoke("pty_spawn")` → `invoke("pty_execute", { command: "dir" })` で出力が返ること。

---

## Phase 5: 統合・堅牢化（両トラック完了後）

1. **タイムアウト処理**: Rig API 呼び出しに `tokio::time::timeout(60s)` を追加
2. **PTY セッションの自動クリーンアップ**: lib.rs の setup ブロックで定期タスク（5分間隔、30分アイドルで自動 kill）
3. **ログ整備**: `println!("DEBUG: ...")` を `log` or `tracing` クレートに置換
4. **エラー型統一**: `map_err(|e| e.to_string())` パターンを構造化エラー型に

**最終検証:**
- 全 AI コマンド（4種）が Anthropic / Gemini で動作
- PTY でコマンド実行＋出力取得が Windows 上で動作
- `cargo clippy` 警告なし
- `cargo build --release` 成功
- 既存フロントエンド機能の回帰テストなし（変更なし）

---

## リスク一覧

| リスク | 影響 | 緩和策 |
|--------|------|--------|
| rig-core と reqwest のバージョン競合 | ビルド不能 | Phase 0 で事前検証。互換バージョンを特定 |
| Rig の chat API のメッセージ形式不一致 | 会話履歴の受け渡し失敗 | convert_messages アダプタ層で吸収 |
| Windows ConPTY の read ハング | PTY 出力が返らない | センチネル方式 + ハードタイムアウト |
| Rig の Gemini プロバイダのモデル名形式差異 | Gemini 呼び出し失敗 | Phase 2a で早期検証 |
| portable-pty 0.9.0 の Windows ビルド | ビルド不能 | 0.8.x にフォールバック |

---

## 対象ファイル一覧

| ファイル | 操作 |
|---------|------|
| `src-tauri/Cargo.toml` | 編集（依存追加） |
| `src-tauri/src/lib.rs` | 編集（mod追加、manage追加、invoke_handler追加） |
| `src-tauri/src/ai.rs` | 編集（Rig経由に段階的書き換え） |
| `src-tauri/src/rig_provider.rs` | **新規** |
| `src-tauri/src/pty_manager.rs` | **新規** |
| `src-tauri/src/pty_commands.rs` | **新規** |

### 既存の再利用関数
- `db::build_project_context()` (`db.rs`) — プロジェクトコンテキストのMarkdown生成。Rig エージェントの preamble に注入
- `get_api_key_and_provider()` (`ai.rs`) — ロジックを `rig_provider.rs` に移行後、削除
