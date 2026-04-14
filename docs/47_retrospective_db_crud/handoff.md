# Epic 47 Handoff

## 引き継ぎ要点

- レトロスペクティブ機能の根幹となるDBスキーマは整備済みであり、`retro_sessions` / `retro_items` / `retro_rules` / `project_notes` の4テーブルが利用可能である。
- バックエンドCRUDコマンドは `src-tauri/src/db.rs` と `src-tauri/src/lib.rs` に実装・登録済みであり、安全に invoke できる状態である。
- `complete_sprint` 実行後には、同一 `sprint_id` に既存セッションがない場合のみ、`draft` 状態の `retro_session` が自動生成される。
- 自動セッション作成はスプリント完了トランザクションのコミット後に実行されるため、retro 側の失敗がスプリント完了自体を妨げない設計になっている。

## Epic 48 でやること

- 次のフェーズは、このバックエンド基盤の上に React / TypeScript のUIを構築すること。
- あわせて、フロントエンドから利用するための型定義と invoke ラッパーを整備すること。
- UI 実装時は `frontend-core` を必ず参照し、既存の共通UI・型・Context のパターンに合わせて接続すること。

## 実装済み前提

- Retro セッション取得・作成・更新
- Retro アイテム取得・追加・更新・削除・承認
- Retro ルール取得・追加・更新・削除
- Project Note 取得・追加・更新・削除
- Migration 18 登録済み
- `cargo test` / `cargo build` 通過済み
