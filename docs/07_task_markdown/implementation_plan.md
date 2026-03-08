# 実装計画: 第7フェーズ タスク詳細・編集機能の拡充 (Markdownサポート)

## 概要
タスク詳細モーダル (`TaskFormModal`) の `description` フィールドを拡張し、Markdown形式での入力およびプレビュー表示機能を実装します。既存の SQLite DB はそのまま（文字列として）保存・読み込みを行うため、DBマイグレーションは不要です。純粋なフロントエンドのUI/UX向上を行う改修となります。

## 推奨ライブラリ
プレビューのレンダリングおよびスタイリングにおいて、以下のライブラリ群の導入を強く推奨します：

1. **`react-markdown`**: Reactの世界で最も標準的で安全かつ柔軟なMarkdownのレンダラーコアです。
2. **`remark-gfm`**: GitHub Flavored Markdown（表、タスクリスト `- [ ]`、URLの自動リンク、取り消し線など）に対応するため、実用的なチケット管理には必須のプラグインです。
3. **`@tailwindcss/typography`**: Tailwind CSS の公式プラグインです。Markdownなどをレンダリングした生のHTMLに対して、`prose`（とダークモード用の `dark:prose-invert`）クラスを指定するだけで、非常に美しく自動でスタイリングします。このプロジェクトのTailwindスタイルに最も適したアプローチです。

## PO（ユーザー）レビューを要求する項目
- **ライブラリの選定**: 上記3つのライブラリの採用について承認をお願いします。
- **UIレイアウト方針**: モーダル内で「Edit（編集）」「Preview（プレビュー）」の切り替えタブを設置するUIを想定しています。左右分割（Split）ではなく切り替えタブで進めてよろしいでしょうか？（モーダルの幅が限られているため、タブ切り替えの方が閲覧性が高いと判断しました）。

## 変更内容 (Proposed Changes)

### フロントエンド コンポーネント
- **[MODIFY] `src/components/board/TaskFormModal.tsx`**
  - `description` フィールドの UI を改修し、上部に `Edit | Preview` のタブボタンを追加します。
  - Editor 側は現状の `@styled textarea` をベースに操作性を改善。
  - Preview 側は `react-markdown` に `remarkPlugins={[remarkGfm]}` を適用し、`prose dark:prose-invert` でスタイリングして表示します。

### インフラ・設定ファイル
- **[MODIFY] `package.json`**
  - `$ npm install react-markdown remark-gfm @tailwindcss/typography` の実行。
- **[MODIFY] `src/index.css` (Tailwind v4想定)**
  - `@plugin "@tailwindcss/typography";` を追加し、`prose` クラスを有効化します。

## 検証計画 (Verification Plan)
### 手動検証 (Manual Verification)
1. Tauri 開発サーバー (`npm run tauri dev`) を立ち上げる。
2. 任意のチケットを開き、詳細画面を表示。
3. Previewタブに切り替え、見出し（`#`）、箇条書きリスト、チェックボックス形式のタスク、コードブロックが適切かつ美しくレンダリングされているか確認する。
4. Markdown文字列を編集して保存 (`Save`) する。
5. 保存後、再度該当チケットを開き、Markdownとして正しくデータベースから復元され表示されるかを検証する。
