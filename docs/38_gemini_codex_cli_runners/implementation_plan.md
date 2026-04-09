# Epic 38: Gemini CLI / Codex CLI Runner 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 37 完了
- 作成日: 2026-04-09

## Epic の目的

Epic 37 で構築した `CliRunner` trait を活用し、Gemini CLI と Codex CLI の具体的な Runner 実装を追加する。これにより、チーム設定で各ロールに異なる CLI を割り当て可能になる。

## スコープ

### 対象ファイル（新規）
- `src-tauri/src/cli_runner/gemini.rs` — GeminiRunner 実装
- `src-tauri/src/cli_runner/codex.rs` — CodexRunner 実装

### 対象ファイル（変更）
- `src-tauri/src/cli_runner/mod.rs` — モジュール登録 + ファクトリ更新

### 対象外
- 設定 UI の変更（Epic 40 で対応）
- PO アシスタント対応（Epic 42 で対応）

## 実装方針

### 1. 各 CLI の引数マッピング調査

| 項目 | Claude Code | Gemini CLI | Codex CLI |
|------|------------|-----------|----------|
| コマンド | `claude` | `gemini` | `codex` |
| プロンプト渡し | `-p "..."` | `-p "..."` | 位置引数 `"..."` |
| モデル指定 | `--model X` | `--model X` | `--model X` |
| 自動実行 | `--permission-mode bypassPermissions` | `--sandbox permissive` | `--full-auto` |
| 作業ディレクトリ | `--add-dir <path>` + cwd | cwd のみ | cwd のみ |
| 追加オプション | `--verbose` | — | — |
| デフォルトモデル | `claude-sonnet-4-20250514` | `gemini-2.5-pro` | `o3` |

**注意:** 各 CLI は頻繁にアップデートされるため、引数仕様は実装時に最新の公式ドキュメントを確認すること。

### 2. GeminiRunner 実装

```rust
// cli_runner/gemini.rs

pub struct GeminiRunner;

impl CliRunner for GeminiRunner {
    fn command_name(&self) -> &str { "gemini" }

    fn build_args(&self, prompt: &str, model: &str, cwd: &str) -> Vec<String> {
        vec![
            "-p".to_string(),
            prompt.to_string(),
            "--model".to_string(),
            model.to_string(),
            "--sandbox".to_string(),
            "permissive".to_string(),
        ]
    }
}
```

### 3. CodexRunner 実装

```rust
// cli_runner/codex.rs

pub struct CodexRunner;

impl CliRunner for CodexRunner {
    fn command_name(&self) -> &str { "codex" }

    fn build_args(&self, prompt: &str, model: &str, _cwd: &str) -> Vec<String> {
        vec![
            "--full-auto".to_string(),
            "--model".to_string(),
            model.to_string(),
            prompt.to_string(),
        ]
    }
}
```

### 4. ファクトリ関数の更新

```rust
// cli_runner/mod.rs
pub fn create_runner(cli_type: &CliType) -> Box<dyn CliRunner> {
    match cli_type {
        CliType::Claude => Box::new(claude::ClaudeRunner),
        CliType::Gemini => Box::new(gemini::GeminiRunner),
        CliType::Codex => Box::new(codex::CodexRunner),
    }
}
```

### 5. 未インストール時のエラーメッセージ

各 Runner に `install_hint()` メソッドを trait に追加することを検討:

```rust
pub trait CliRunner: Send + Sync {
    // ... 既存メソッド

    /// CLI 未インストール時に表示するインストール手順
    fn install_hint(&self) -> &str;
}
```

`spawn_process` 内の `ErrorKind::NotFound` 分岐で `runner.install_hint()` を使用する。

## テスト方針

- 各 Runner の `build_args()` が期待される引数リストを返すこと（ユニットテスト可能）
- Gemini CLI インストール環境でプロンプト実行 → 出力ストリーミング → 正常終了
- Codex CLI インストール環境でプロンプト実行 → 出力ストリーミング → 正常終了
- 未インストール CLI での実行 → CLI 名入りの明確なエラーメッセージ
- Claude CLI での既存動作に影響がないこと（回帰テスト）
