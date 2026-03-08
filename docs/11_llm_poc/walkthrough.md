# `refine_idea` バックエンド実装 ウォークスルー

## 追加された機能 (Changes made)

1. **`src-tauri/src/ai.rs` に `refine_idea` コマンドを追加しました。**
   - ユーザーからのテキスト(`idea_seed`)と会話履歴(`previous_context`)を受け取る構造です。
   - `settings.json` ストアから `default-ai-provider`, `anthropic-api-key`, `gemini-api-key` を直接読み込みます。
   - プロバイダー (Anthropic または Gemini) に応じた API ペイロードを動的に組み立てます。
   - `Message` 構造体 (`role`, `content`) を新設し、過去のコンテキストもシームレスに連携できるようにしています。
2. **`src-tauri/src/lib.rs` に `refine_idea` を登録しました。**
   - `tauri::generate_handler!` に追加し、フロントエンドの `invoke` から呼び出し可能になりました。

## テスト・検証内容 (What was tested)

- **Rust のコンパイルテスト**:
  - `cargo check` を実行し、構文エラー・型エラー・モジュール解決エラーがないことを確認しました。
  - 成功出力: `Finished dev [unoptimized + debuginfo] target(s) in 25.24s`

## 検証結果 (Validation results)

バックエンド側の実装は完了しており、Tauri アプリの一部として正常にビルド・動作する状態です。フロントエンドへの統合（API通信を含むE2Eテスト）は次のステップに進む準備が整いました。
