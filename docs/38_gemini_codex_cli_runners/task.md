# Epic 38: Gemini CLI / Codex CLI Runner 実装 タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: Epic 37 完了
- 作成日: 2026-04-09

## 概要

Epic 37 で構築した `CliRunner` trait の具体実装として、Gemini CLI と Codex CLI の Runner を追加する。これにより Dev エージェントが Claude Code 以外の CLI でもタスクを実行できるようになる。

## 実行順序

### 1. Gemini CLI Runner の実装
- [ ] `src-tauri/src/cli_runner/gemini.rs` を新規作成する。
- [ ] `GeminiRunner` 構造体に `CliRunner` trait を実装する。
- [ ] Gemini CLI の引数マッピングを実装する:
  - コマンド: `gemini`
  - プロンプト: `-p "..."`
  - モデル: `--model X`
  - 自動実行: `--sandbox permissive`
- [ ] Gemini CLI 固有の環境変数設定があれば `env_vars()` で返却する。

### 2. Codex CLI Runner の実装
- [ ] `src-tauri/src/cli_runner/codex.rs` を新規作成する。
- [ ] `CodexRunner` 構造体に `CliRunner` trait を実装する。
- [ ] Codex CLI の引数マッピングを実装する:
  - コマンド: `codex`
  - プロンプト: 位置引数
  - モデル: `--model X`
  - 自動実行: `--full-auto`

### 3. ファクトリへの登録
- [ ] `cli_runner/mod.rs` の `create_runner()` に Gemini / Codex 分岐を追加する。
- [ ] `mod.rs` に `pub mod gemini;` と `pub mod codex;` を追加する。

### 4. CLI 固有のデフォルトモデル定義
- [ ] 各 Runner にデフォルトモデル名を定義する:
  - Claude: `claude-sonnet-4-20250514`
  - Gemini: `gemini-2.5-pro`
  - Codex: `o3`
- [ ] ロールの `model` が空の場合にデフォルトモデルにフォールバックする処理を共通ロジックに追加する。

### 5. エラーハンドリングの改善
- [ ] 各 CLI が未インストールの場合の `NotFound` エラーメッセージを CLI 名に応じてカスタマイズする。
  - 例: 「Gemini CLI が見つかりません。`npm install -g @anthropic-ai/gemini-cli` でインストールしてください。」
- [ ] Epic 36 の `detect_installed_clis` 結果と連携し、未インストール CLI が選択されたロールでのタスク実行時に事前チェックを行う。

### 6. 動作確認
- [ ] Gemini CLI がインストールされた環境でタスク実行が完了し、出力がストリーミングされることを確認する。
- [ ] Codex CLI がインストールされた環境でタスク実行が完了し、出力がストリーミングされることを確認する。
- [ ] 未インストール CLI のロールでタスク実行時に適切なエラーメッセージが表示されることを確認する。
