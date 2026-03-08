# Epic 1: AI要件定義アシスタント（対話型Story昇華機能） バックエンド実装

- [x] `src-tauri/src/ai.rs` に `refine_idea` コマンドを追加
  - [x] Storeからの設定（Provider, API Key）の動的読み込み処理を実装
  - [x] 引数に基づくプロンプト構築とチャット履歴（`previous_context`）のマッピング
  - [x] Claude用APIの呼び出し処理の実装
  - [x] Gemini用APIの呼び出し処理の実装
  - [x] エラーハンドリングとレスポンス返却 (`Result<String, String>`)
- [x] `src-tauri/src/lib.rs` に `refine_idea` コマンドを登録し、フロントエンドから呼べるようにする
- [x] ビルドテスト（`cargo check`等）とコード健全性の確認
