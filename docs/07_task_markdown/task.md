# タスクリスト: 第7フェーズ タスク詳細・編集機能の拡充 (Markdownサポート)

- [x] 実装計画 (Implementation Plan) の作成とPO（ユーザー）承認の獲得
- [x] 必要なライブラリのインストール (`react-markdown`, `remark-gfm`, `@tailwindcss/typography`)
- [x] Tailwind CSS (v4) への Typography プラグインの設定追加 (`src/index.css`)
- [x] `TaskFormModal.tsx` の UI 改修
  - [x] Edit / Preview 切り替えタブコンポーネントの実装
  - [x] Markdown プレビュー機構 (`react-markdown` + `remark-gfm`) の実装
  - [x] Typography (`prose` クラス) を用いたスタイリング適用
- [x] SQLite DB 保存・読み込み検証（正常に文字列としてCRUDできることの確認）
- [x] Walkthroughドキュメント (`walkthrough.md`) の作成
