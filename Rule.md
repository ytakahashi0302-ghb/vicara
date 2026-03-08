# MicroScrum AI 開発ガイドライン (Rule.md)

## 1. プロジェクト概要
- **目的**: ローカル環境でセキュアに動作する、AIネイティブなスクラム開発カンバンツール。
- **コアドメイン**: 1スプリント=8時間の「マイクロスクラム」。Story（親）とTask（子）の強固な紐付けをGUIで可視化する。

## 2. 技術スタック
- **Frontend**: React 18+, TypeScript, Tailwind CSS
- **UI Library**: dnd-kit (カンバン機能用), lucide-react (アイコン用)
- **Backend/Desktop**: Tauri v2 (Rust)
- **Database**: SQLite (ローカルファイル)
- **DB Access**: `@tauri-apps/plugin-sql` およびRust側の `tauri-plugin-sql`

## 3. AI（Dev）へのコーディング規約
- **TypeScript**: 厳格な型定義（`any` の使用禁止）。インターフェースは `types/` ディレクトリ等に集約すること。
- **React**: 関数コンポーネントとHooksを使用すること。UIとビジネスロジックを適切に分離すること。
- **SQLite操作**: フロントエンドで直接複雑なSQLを組み立てるのではなく、極力Tauriのコマンド（Rust側）で安全に処理するか、プラグインの作法に則りSQLインジェクション対策（プレースホルダーの利用）を徹底すること。
- **状態管理**: まずはReact標準のContext/Stateで小さく始め、複雑化した段階で適切なライブラリ導入をPOに提案すること。

## 4. AIエージェントの振る舞い（重要）
- **思考の透明性**: 実装前に必ずアプローチをPOに提示し、承認を得てからコードを書くこと。
- **破壊的変更の禁止**: 既存のコードを大きく書き換える場合や、新しいnpmパッケージ/Rustクレートを追加する場合は、必ず事前にPOへ理由を説明し許可を得ること。
- **コンテキストの維持**: チャットセッションを終了する際は、必ず現在の進捗と次やるべきタスクをまとめた `handoff.md` を作成すること。

## 5. 命名規則・採番ルール
- **実装順序の可視化（採番ルール）**: 実装の順序やフェーズが後から見てわかるように、ドキュメント、各種計画ファイル、機能単位の作業ディレクトリ等を作成する際は、必ずプレフィックスとして採番（例: `01_setup/`, `02_frontend_db/`, `03_kanban_ui/`）を行うこと。
  - ※ただし、`src/components` のようなReact/Tauriの標準的なディレクトリ構造の命名作法は崩さない範囲で適用すること。

## 6. 技術的な知見と制約（Tips & Constraints）
- **Tailwind CSS v4**: Vite + Tailwind v4環境では `postcss.config.js` に `tailwindcss` ではなく `@tailwindcss/postcss` を指定する必要がある。また CSSファイルのエントリーポイントには `@tailwind base;` ではなく `@import "tailwindcss";` を使用する。
- **Tauri v2 SQL Permissions**: `@tauri-apps/plugin-sql` をフロントエンドから使用する際、`capabilities/default.json` にて `"sql:default"` を許可するだけでなく、実際にコマンドを叩くために `"sql:allow-execute"` および `"sql:allow-select"` も明示的に追加しなければならない。

## 7. 課題・技術的負債 (Tech Debt)
- **ESLintの本格導入**: 現状の `npm run lint` は `tsc --noEmit` による静的型チェックのみで代用している。次期フェーズなど適切なタイミングで `eslint` (+ `eslint-plugin-react-hooks` 等) を本格導入し、フォーマッター設定と合わせて堅牢なチェック環境を構築すること。
