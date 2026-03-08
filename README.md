# MicroScrum AI (AI Scrum Tool)

MicroScrum AIは、ローカル環境でセキュアに動作する、AIネイティブなスクラム開発支援ツールです。
1スプリント=1~8時間の「マイクロスクラム」という概念に基づき、AIによるタスク分割と直感的なカンバン操作を通じて、個人の開発効率を最大化します。

## 🚀 主な機能

- **AI Task Decomposition**: AI（Anthropic API,Gemini API）を利用して、User Storyから最適なタスクを自動生成・分割。
- **Interactive Kanban Board**: Story（親）とTask（子）の紐付けを可視化。ドラッグ＆ドロップによる直感的な操作。
- **Sprint Management**: 1スプリントを1時間~8時間の「マイクロスクラム」として管理。専用のタイマー機能とステータス追跡。
- **Markdown Support**: タスク詳細やStoryの説明にMarkdownを利用可能。リッチなプレビュー機能付き。
- **Sprint History & Archive**: 完了したスプリントをアーカイブし、過去のベロシティや成果をいつでも確認。
- **Secure Local Storage**: 全データはローカルのSQLiteデータベースに保存され、外部クラウドへの不要なデータ流出を防ぎます。

## 🛠 技術スタック

- **Frontend**: React 19, TypeScript, Tailwind CSS v4
- **Backend**: Tauri v2 (Rust)
- **Database**: SQLite (local)
- **State Management**: React Context / Hooks
- **Icons**: Lucide React

## 📦 セットアップ方法

プロジェクトをローカルで実行するには、以下の環境が必要です。

1. **Rust**: [Rust公式サイト](https://www.rust-lang.org/)からインストール。
2. **Node.js**: [Node.js公式サイト](https://nodejs.org/)からインストール。

### 手順

```bash
# 1. 依存関係のインストール
npm install

# 2. 開発モードでの起動（Tauri）
npm run tauri dev
```

## ⌨️ 開発コマンド

- `npm run dev`: Vite開発サーバーの起動
- `npm run build`: フロントエンドのビルド
- `npm run lint`: ESLintと型チェックの実行
- `npm run tauri dev`: Tauriアプリの開発モード起動
- `npm run tauri build`: Tauriアプリの本番用パッケージ作成

## 📝 開発ガイドライン

詳細な開発ルールや設計方針については、[Rule.md](Rule.md) を参照してください。

---
Created as MVP for AI-native agile development.
