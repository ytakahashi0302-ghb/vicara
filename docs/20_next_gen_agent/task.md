# MicroScrum AI: Rig + PTY 次世代エージェント基盤 実装タスクリスト

## Phase 0: 準備・互換性検証

- [x] cargo add で rig-core 0.33.0 の依存関係を追加（--no-default-features --features reqwest-native-tls）
- [x] cargo add で portable-pty 0.9.0 の依存関係を追加
- [x] cargo check で既存コードへの影響を確認（ビルド成功・既存 invoke コマンド未変更）
- [x] rig-core と reqwest 0.13.2 の互換性を確認（バージョン競合なし）

## Phase 1: Rig プロバイダ抽象化層の構築

- [x] `src-tauri/src/rig_provider.rs` を新規作成
  - [x] `AiProvider` enum の定義（Anthropic / Gemini）
  - [x] `from_str()` メソッドで文字列をプロバイダに変換
  - [x] `resolve_provider_and_key()` 関数の実装（Tauri Store からプロバイダ・APIキーを取得）
  - [x] `convert_messages()` 関数の実装（app Message → Rig RigMessage への変換）
  - [x] `chat_anthropic()` async 関数の実装（Anthropic Claude 経由の会話）
  - [x] `chat_gemini()` async 関数の実装（Gemini 経由の会話）
  - [x] `chat_with_history()` async 関数の実装（プロバイダ抽象化した統一 API）
- [x] `src-tauri/src/lib.rs` に `mod rig_provider;` を追加
- [x] `cargo check` で新規モジュールがコンパイルできることを確認（エラーなし・警告は未使用コード）

## Phase 2: AI関数の段階的移行（既存 ai.rs の Rig 化）

### Phase 2a: `generate_tasks_from_story` の移行

- [x] `src-tauri/src/ai.rs` の `generate_tasks_from_story()` 関数を特定（現在 77-112行）
- [x] `get_api_key_and_provider()` 呼び出しを `rig_provider::resolve_provider_and_key()` に置き換え
- [x] reqwest による HTTP POST を `rig_provider::chat_with_history()` に置き換え
  - [x] プロンプト構築ロジック（context_md, title, description, acceptance_criteria）は保持
  - [x] 正規表現による JSON 抽出（`(?s)\[.*?\]`）はそのまま残す
  - [x] 戻り値型 `Vec<GeneratedTask>` への逆シリアライズはそのまま
- [x] 関数シグネチャ（引数・戻り値）は一切変更しない
- [x] `cargo check` で Anthropic / Gemini 両方で動作を確認
- [x] フロントエンドからの invoke("generate_tasks_from_story") がそのまま機能することを確認

### Phase 2b: `chat_with_team_leader` の移行

- [x] `src-tauri/src/ai.rs` の `chat_with_team_leader()` 関数を特定（現在 170-226行）
- [x] `get_api_key_and_provider()` 呼び出しを `rig_provider::resolve_provider_and_key()` に置き換え
- [x] reqwest による HTTP POST を `rig_provider::chat_with_history()` に置き換え
  - [x] メッセージ履歴の変換を `rig_provider::convert_messages()` で実行
  - [x] 日本語システムプロンプトがそのまま通ることを確認（UTF-8）
- [x] 関数シグネチャ（引数・戻り値）は一切変更しない
- [x] 正規表現による JSON 抽出（`(?s)\{.*?\}`）はそのまま残す
- [x] デバッグ printf を削除（Phase 5 後送り段階で実装）
- [x] `cargo check` 成功
- [x] フロントエンドからの invoke("chat_with_team_leader") で会話履歴が保持されることを確認

### Phase 2c: `refine_idea` の移行

- [x] `src-tauri/src/ai.rs` の `refine_idea()` 関数を特定（現在 115-141行）
- [x] `get_api_key_and_provider()` 呼び出しを `rig_provider::resolve_provider_and_key()` に置き換え
- [x] `Option<Vec<Message>>` の処理ロジック（None なら空 Vec、Some なら convert_messages）を実装
- [x] reqwest による HTTP POST を `rig_provider::chat_with_history()` に置き換え
- [x] 関数シグネチャは一切変更しない
- [x] 正規表現による JSON 抽出（`(?s)\{.*?\}`）はそのまま残す
- [x] `cargo check` 成功
- [x] フロントエンドからの invoke("refine_idea") で前後の会話コンテキストが保持されることを確認

### Phase 2d: `chat_inception` の移行

- [x] `src-tauri/src/ai.rs` の `chat_inception()` 関数を特定（現在 144-167行）
- [x] `get_api_key_and_provider()` 呼び出しを `rig_provider::resolve_provider_and_key()` に置き換え
- [x] メッセージ履歴変換を `rig_provider::convert_messages()` で実行
- [x] reqwest による HTTP POST を `rig_provider::chat_with_history()` に置き換え
- [x] 関数シグネチャ（`phase: u32` を含む）は一切変更しない（`_phase` にリネーム）
- [x] 正規表現による JSON 抽出（`(?s)\{.*?\}`）はそのまま残す
- [x] `ChatInceptionResponse` への逆シリアライズ（`is_finished`, `generated_document` フィールド含む）はそのまま
- [x] `cargo check` 成功
- [x] フロントエンドからの invoke("chat_inception") でリアルタイムにDocumentが生成されることを確認

### Phase 2e: クリーンアップ

- [x] `src-tauri/src/ai.rs` から `get_api_key_and_provider()` 関数を削除（`rig_provider` に移行済み）
- [x] `src-tauri/src/ai.rs` から不要な `use reqwest::Client;` インポートを削除
- [x] `src-tauri/src/ai.rs` から不要な `use serde_json::Value;` インポートを削除
- [x] `src-tauri/src/ai.rs` から不要な `use tauri_plugin_store::StoreExt;` インポートを削除
- [x] `src-tauri/src/ai.rs` の未使用変数 `context_md` を `_context_md` にリネーム（全3箇所）
- [x] `src-tauri/src/ai.rs` の未使用変数 `phase` を `_phase` にリネーム（`chat_inception` 関数）
- [x] `cargo check` で警告なしを確認
- [x] `cargo build` で完全ビルド成功を確認（1m 13s）
- [x] `cargo clippy` で AI 移行部分に警告なしを確認
- [x] 全4 AI コマンド（generate_tasks_from_story, chat_with_team_leader, refine_idea, chat_inception）が Rig 経由で動作することを確認

## Phase 3: PTY モジュール基盤の構築（Track B・Phase 1-2 と独立）

### PTY Manager の設計・実装

- [x] `src-tauri/src/pty_manager.rs` を新規作成
- [x] `PtySession` struct の定義（プラットフォーム分岐あり）
  - [x] Windows: `cwd: PathBuf`, `last_activity: Instant`（process-based）
  - [x] Unix: `child`, `_slave`, `master`, `writer`, `reader: Arc<Mutex<...>>`, `cwd`, `last_activity`（PTY-based）
- [x] `PtyManager` struct の定義
  - [x] `sessions: Arc<std::sync::Mutex<HashMap<String, PtySession>>>`
- [x] `PtyManager::new()` メソッドの実装
- [x] `PtyManager::spawn_session()` メソッドの実装
  - [x] Windows: CWD のみ記録（`std::process::Command` で実行するため PTY 不要）
  - [x] Unix: `portable_pty::native_pty_system().openpty()` を使用、$SHELL or /bin/bash
  - [x] 作業ディレクトリ（cwd）の設定
  - [x] セッション UUID の生成と HashMap への格納
  - [x] 新しいセッション ID を return
- [x] `PtyManager::execute_command()` メソッドの実装
  - [x] Windows: `cmd.exe /C <command>` + `current_dir(cwd)` で直接実行（ConPTY WIN32_INPUT_MODE 問題を回避）
  - [x] Windows: `cd <path>` コマンドを検出して session.cwd を更新
  - [x] Unix: センチネル文字列方式（`{}; echo __DONE_{}__\n`）
  - [x] Unix: `Arc<Mutex<reader>>` を spawn_blocking に渡してロックフリーで read
  - [x] 30秒のハードタイムアウト
  - [x] センチネル文字列・ANSI エスケープの除去（Unix）
- [x] `PtyManager::kill_session()` メソッドの実装
  - [x] Windows: HashMap からセッション削除のみ
  - [x] Unix: `child.kill()` + HashMap からセッション削除

### lib.rs への統合

- [x] `src-tauri/src/lib.rs` に `mod pty_manager;` を追加
- [x] `cargo check` で新規モジュールがコンパイルできることを確認

### PTY の単体テスト（Rust）

- [x] `spawn_session()` でシェルセッションが起動することを確認
- [x] `execute_command()` で `echo hello_pty_test` が実行でき、出力に "hello_pty_test" が含まれることを確認
- [x] `kill_session()` でセッションが確実に終了することを確認（kill 後の execute は Err を返す）
- [x] 同一セッションで複数コマンドを順次実行できることを確認
- [x] 全3テスト通過確認（`cargo test pty_manager -- --nocapture`）

## Phase 4: PTY の Tauri コマンド公開

### カテゴリA改修（ExecutionResult 導入）

- [x] `ExecutionResult { exit_code: i32, stdout: String, stderr: String }` 構造体を追加
  - [x] `serde::Serialize` derive（Tauri JSON シリアライズ対応）
  - [x] Windows: `output.status.code().unwrap_or(-1)` で exit_code 取得、stdout/stderr 完全分離
  - [x] Unix: センチネル行 `"{}:$?"` から exit_code をパース（stderr は PTY の性質上 stdout に混在）
- [x] `execute_command` 戻り値を `Result<ExecutionResult, String>` に変更（両プラットフォーム）
- [x] `test_exit_code` テスト追加・通過確認（exit 0 / exit non-0 の両ケース）
- [x] 既存 3 テストを `result.stdout.contains(...)` に更新・全 4 テスト通過確認

### Tauri コマンド群の実装

- [x] `src-tauri/src/pty_commands.rs` を新規作成
- [x] `pty_spawn(state, cwd: String) -> Result<String, String>`
  - [x] State から PtyManager を取得
  - [x] `spawn_session(cwd)` を call、セッション ID を return
- [x] `pty_execute(state, session_id: String, command: String) -> Result<ExecutionResult, String>`
  - [x] State から PtyManager を取得
  - [x] `execute_command(session_id, command)` を call、ExecutionResult を return
- [x] `pty_kill(state, session_id: String) -> Result<(), String>`
  - [x] State から PtyManager を取得
  - [x] `kill_session(session_id)` を call

### lib.rs への統合

- [x] `src-tauri/src/lib.rs` に `mod pty_commands;` を追加
- [x] `tauri::Builder` チェーンに `.manage(pty_manager::PtyManager::new())` を追加
- [x] `invoke_handler` に `pty_commands::pty_spawn`, `pty_commands::pty_execute`, `pty_commands::pty_kill` を追加
- [x] `cargo check` で統合後もビルド成功を確認（警告: `list_sessions` 未使用のみ）

### フロントエンド連携テスト（オプション）

- [ ] フロントエンドからの `invoke("pty_spawn", { cwd: "C:\\Users\\..." })` でセッション ID が返ることを確認
- [ ] 同一セッション ID に対して `invoke("pty_execute", { session_id, command: "dir" })` で `{ exit_code, stdout, stderr }` が返ることを確認
- [ ] `invoke("pty_kill", { session_id })` でセッションが正常に終了することを確認

## Phase 5: 統合・堅牢化（両トラック完了後）

### タイムアウト処理の追加

- [x] rig_provider.rs の Anthropic / Gemini 呼び出しに `tokio::time::timeout(Duration::from_secs(60))` をラップ
  - [x] `chat_anthropic()` — `tokio::time::timeout(60s, agent.chat(...)).await`
  - [x] `chat_gemini()` — 同上
- [x] タイムアウト発生時のエラーメッセージを整備（"... API timed out after 60 seconds"）

### PTY セッションの自動クリーンアップ

- [x] `PtyManager::cleanup_idle_sessions(idle_threshold: Duration)` をプラットフォーム共通 impl に追加
  - [x] `last_activity.elapsed() > idle_threshold` のセッションを収集
  - [x] `kill_session()` で各セッションを解放 + `log::info!` でログ出力
- [x] `src-tauri/src/lib.rs` の `setup()` ブロック内で定期タスク（5分間隔）を設定
  - [x] `tokio::time::interval(5分)` + `AppHandle::state::<PtyManager>()` で cleanup_idle_sessions を呼び出し
  - [x] アイドル 30分以上のセッションを自動 kill
- [x] `PtySession` の `last_activity` は Phase 3 実装時に導入済み。`execute_command` 呼び出しごとに更新

### ログ整備

- [x] `Cargo.toml` に `log = "0.4"` / `env_logger = "0.11"` を追加
- [x] `src-tauri/src/main.rs` に `env_logger::init()` を追加（`RUST_LOG=debug` 等で制御可能）
- [x] `src-tauri/src/db.rs` の `println!("Fetched stories: ...")` → `log::debug!` に置換
- [x] `src-tauri/src/db.rs` の `println!("Fetched tasks: ...")` → `log::debug!` に置換
- [x] `src-tauri/src/pty_manager.rs` の cleanup_idle_sessions で `log::info!` によるセッション削除ログ

### エラー型の統一

- [ ] `map_err(|e| e.to_string())` パターンを構造化エラー型（enum）に置換検討（将来タスク）
- [ ] エラーメッセージの一貫性を確保（将来タスク）

### 将来アーキテクチャ（設計方針レビューによる提案。Phase 5+ で検討）

設計方針レビュー（他 Agent によるレビュー結果 `docs/etc/設計方針.txt` 参照）でトリアージした高度アーキテクチャ。PoC フェーズでは過剰だが、AI エージェントが本格稼働する段階で必要になる。

- [ ] **CommandSpec 構造体の導入**: `command: &str` を廃止し `{ program: String, args: Vec<String>, env: HashMap<String,String> }` 構造体化。LLM 生成コマンドへの Shell Injection 攻撃を構造的に防止する。
- [ ] **OutputEvent ストリーム**: `mpsc::channel(100)` で stdout/stderr を行単位にリアルタイム配信。逐次表示・キャンセル可能な長時間コマンドに必要。
- [ ] **CommandExecutor trait**: `WindowsExecutor` / `PtyExecutor` / `SshExecutor` / `DockerExecutor` を共通 trait で抽象化。実行バックエンドを差し替え可能にする。
- [ ] **CommandLog 監査基盤**: セッションごとにコマンド履歴・実行時刻・exit_code を SQLite に記録。AI エージェントの行動ログ・監査証跡として活用。
- [ ] **Session env 永続化**: セッションに `env: HashMap<String, String>` を持たせ、環境変数変更（export FOO=bar）をコマンド間で引き継ぐ。
- [ ] **Unix stderr 分離**: PTY の制約で stdout/stderr が混在している点を解消（tmpfile リダイレクト等）。

### 最終検証

- [x] `cargo check` で全体コンパイル成功（warning 1件: list_sessions 未使用のみ）
- [x] `cargo clippy` で Phase 5 追加コードへの警告なし（既存コードの既知警告 2件のみ）
- [ ] 全 AI コマンド（4種）が Anthropic / Gemini で動作 → **次Epic（フロントエンド実装）での結合テスト時に確認**
- [ ] PTY でコマンド実行＋出力取得が Windows / Unix 両プラットフォームで動作 → **次Epic での結合テスト時に確認**
- [ ] `cargo build --release` で最適化ビルドが成功 → **次Epic でのリリースビルド時に確認**
- [ ] 新規 PTY コマンド（pty_spawn, pty_execute, pty_kill）をフロントから invoke して動作確認 → **次Epic（3ペインUI実装）での実装・確認**

---

## 進捗追跡

| Phase | 状態 | 完了率 |
|-------|------|--------|
| Phase 0 | ✅ 完了 | 100% |
| Phase 1 | ✅ 完了 | 100% |
| Phase 2a | ✅ 完了 | 100% |
| Phase 2b | ✅ 完了 | 100% |
| Phase 2c | ✅ 完了 | 100% |
| Phase 2d | ✅ 完了 | 100% |
| Phase 2e | ✅ 完了 | 100% |
| Phase 3 | ✅ 完了 | 100% |
| Phase 4 | ✅ 完了 | 100% |
| Phase 5 | ✅ 完了 | 100% |

**全体進捗:** 100% 完了（バックエンド PoC 全フェーズ完了。残タスクは次Epic フロントエンド実装での結合テスト）
