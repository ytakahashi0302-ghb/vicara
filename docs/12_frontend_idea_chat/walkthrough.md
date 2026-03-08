# 12_frontend_idea_chat_walkthrough.md

## 概要
AIを用いた要件定義アシスタント（Epic 1）のフロントエンドUI実装およびバックエンドの連携・改修を完了しました。旧来のモーダルUIから、よりモダンでシームレスな「サイドドロワー」と「リアルタイムドキュメント（2ペイン分割）」のUXへ刷新しています。

## 変更内容
### バックエンド (`src-tauri/src/ai.rs`)
- LLM呼び出し時の `max_tokens` を 2000 に引き上げ、長文出力が途切れないように改善。
- システムプロンプトを改修し、構造化されたJSONデータ出力を指示（`reply` と `story_draft`）。
- `story_draft` の内部構造をさらに細分化し、`title`, `description`, `acceptance_criteria` を個別に取得するように設定。
- LLMが親切心で付与する ` ```json ` 等のマークダウン装飾を除去して安全にJSONパースするクリーニング処理を実装。

### フロントエンド (`src/components/ai/IdeaRefinementDrawer.tsx`, `Board.tsx`)
- 古い `IdeaRefinementModal.tsx` を削除。
- 新規UIとして、画面右側からスライドインする `IdeaRefinementDrawer.tsx` を実装。
- ドロワー内を2ペイン構成とし、左側にチャットUI、右側に生成中の要件（Live Document）を独立してリアルタイム表示。
- 右ペインのLive Documentでは、JSONから受け取った Title / Description / Acceptance Criteria を構造化されたレイアウトで表示（Descriptionは `react-markdown` でリッチテキスト表示）。
- 「この内容でStoryを作成」アクション時に、生成された構造化データを `StoryFormModal` の各対応フィールド（title, description, acceptanceCriteria）に正しくプリフィル（事前入力）されるようマッピングを実装。
- `Board.tsx` の「💡 アイデアから作成」ボタンからの呼び出し先を Drawer コンポーネントへ差し替え。

## テスト手順（PO向けマニュアルテスト）
1. `npm run tauri dev` でアプリケーションを起動する。
2. 画面右上の「💡 アイデアから作成」ボタンをクリックし、サイドドロワーが開くことを確認する。
3. 左側のチャット入力欄に、「〇〇な機能を作りたい」等のフワッとしたアイデアを入力して送信する。
4. AIからの返答（考えを深掘りする逆質問等）がチャット欄に表示されることを確認する。
5. **同時に**、右ペイン（Live Document領域）に、以下の情報が構造化されて表示されることを確認する。
    - Title
    - Description (Markdownレンダリング)
    - Acceptance Criteria
6. 何度かチャットのやり取りを行い、右ペインのドキュメントが文脈に合わせて更新されていくこと、文章が途中で途切れていないことを確認する。
7. 内容に満足したら、右下のアクションボタン「この内容でStoryを作成」をクリックする。
8. ドロワーが閉じ、自動的に「ストーリーを追加」モーダル（StoryFormModal）が開くことを確認する。
9. StoryFormModal の「タイトル」「説明」「受け入れ条件」の各入力フィールドに、先ほどのドキュメント内容がマッピング・事前入力されていることを確認する。
10. そのまま「作成」ボタンを押し、カンバンボード上にStoryが登録されることを確認する。

## 検証結果（エージェント）
- [x] Rust (Tauri) バックエンドの `cargo check`, `cargo clippy` 通過確認済み。
- [x] React (TypeScript) フロントエンドの `npm run lint` 通過確認済み。
- [x] アプリへのUIマッピング、ドロワー開閉、データフローの型整合性を検証済み。
