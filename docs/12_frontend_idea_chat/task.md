# タスクリスト: 12_frontend_idea_chat (UX改修)

- [x] バックエンド (`src-tauri/src/ai.rs`) の改修
  - [x] `refine_idea` コマンド用の戻り値構造体（`RefinedIdeaResponse`）を定義
  - [x] Anthropic/Gemini の `max_tokens` (2000) 設定の変更
  - [x] システムプロンプトを改修し、`reply` と `draft` のJSON出力を強制
  - [x] LLMのJSON出力を正規表現等で抽出・パースして返すロジックの実装（Markdown装飾のクリーニング処理を含む）
- [x] フロントエンドの不要ファイル削除
  - [x] `src/components/ai/IdeaRefinementModal.tsx` の削除と関連参照のクリア
- [x] 新規UI `IdeaRefinementDrawer.tsx` の作成
  - [x] 画面右側からスライドインするドロワーのベースレイアウト（背景オーバーレイ、アニメーションなど）実装
  - [x] ドロワー内部の2ペイン分割（左：チャット、右：ライブドキュメントプレビュー）
  - [x] Tauriコマンドからの返答 (`reply`, `draft`) を受け取り、それぞれのStateに反映するロジックの実装
  - [x] 「この内容でStoryを作成」アクションの繋ぎ込み
- [x] `Board.tsx` の更新
  - [x] 新しいドロワーコンポーネントへの差し替えとState渡し
- [x] 結合テストとビルド検証
  - [x] `cargo check`, `npm run lint` の実行とエラー解消
