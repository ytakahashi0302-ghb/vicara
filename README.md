# vicara

<p align="center">
  <img src="./public/logos/banner.png" alt="vicara banner" width="800" />
</p>

<p align="center">
  <strong>ソロ開発者が複数のAIを"チーム"として率い、熟慮と意思決定を中心に据えて実装と検証を前へ進めるためのローカルファースト開発環境。</strong>
</p>

<p align="center">
  <a href="./README_en.md">🇺🇸 English</a>
</p>

---

## vicara とは？

**vicara** は、ソロ開発者が複数のAIを「チーム」として率い、熟慮と意思決定を中心に据えて実装と検証を前へ進めるためのローカルファーストなデスクトップアプリです。

vicara は、AIにすべてを委ねるためのツールではありません。
人間が **プロダクトオーナー** として「何を作るか」「どの順序で進むか」を決め、AIはその意思決定に従って実装・調査・検証を加速する。その関係を、**スクラム** という共通言語で自然に扱えるようにしました。

> 迷いなく最初の一歩を踏み出し、複数のAIを"チーム"として率いながら、ブラックボックスに飲み込まれず、正しい道筋を与えて前進する。

vicara は、アイデアの壁打ち、プロジェクト文脈の整理、スプリント計画、ロール分担、コーディングエージェント CLI による実装実行までをひとつのUIに統合しています。

![vicara overview](./docs/images/vicara-overview-v2_0_0.png)

---

## 主な機能

| 機能 | 説明 |
|------|------|
| **POアシスタント** | プロダクトオーナーの意思決定を補佐し、優先順位整理・要求具体化・進行判断を支援するサイドバー型AI |
| **Dev Agent** | ロールテンプレートに基づいてコーディングエージェント CLI（Claude Code / Gemini / Codex）を実行し、タスク実装と検証を進める実装担当AI |
| **AIレトロスペクティブ** | スプリント完了後にKPT形式で振り返りを実施。SM/POエージェントが実行ログから課題を自動抽出し、全体サマリを合成 |
| **改善ループ (Try to Rules)** | レトロで承認された「Try」をプロジェクトの `Rule.md` へ自動反映し、AIチームの振る舞いを継続的に改善 |
| **Inception Deck** | AIとの壁打ちを通して `PRODUCT_CONTEXT.md`、`ARCHITECTURE.md`、`Rule.md` を整備 |
| **プロジェクトノート** | AIエージェントと共有可能なプロジェクト固有の永続的なメモ・備忘録機能 |
| **Scaffold** | 技術スタック検出、初期ディレクトリ構築、`AGENTS.md` / `.claude/settings.json` 生成 |
| **AIタスク分解** | PBI (Product Backlog Item) を起点に実行しやすいタスク粒度へ落とし込む支援 |
| **インタラクティブカンバン** | PBI、タスク、スプリントを視覚的に管理。プロジェクト単位の自動採番（PBI-1, Task-5等）に対応 |
| **Terminal Dock** | VS Code ライクなタブ型ターミナルで複数AIの実行状態を可視化。CLI ストリーミング表示を改善 |
| **マルチエージェント実行** | ロールごとにコーディングエージェント CLI を起動し、タスクを並列実装 |
| **Git Worktree Review** | タスク単位の隔離環境、プレビュー、承認マージ、競合対応をひとつの流れに統合 |
| **LLM Observability** | token 使用量と概算コストを project / sprint 単位で可視化 |
| **リサイズ可能な 3 ペイン UI** | Kanban / Terminal / POアシスタント のレイアウトを作業スタイルに合わせて調整 |
| **ローカルファースト** | ローカルディレクトリとローカルDBを前提にした安全で透明な運用 |

---

## はじめかた

### 必要環境

- [Node.js](https://nodejs.org/)（LTS 推奨）
- [Rust](https://www.rust-lang.org/tools/install) / Cargo
- コーディングエージェント CLI（いずれか1つ以上）: [Claude Code](https://docs.anthropic.com/en/docs/claude-code) / [Gemini CLI](https://github.com/google-gemini/gemini-cli) / [Codex CLI](https://github.com/openai/codex)

### インストール & 起動

```bash
git clone https://github.com/ytakahashi0302-ghb/vicara.git
cd vicara
npm install
npm run tauri -- dev
```

### LLM セットアップ

vicara は Claude API、Gemini API、OpenAI API、Ollama など複数の LLM プロバイダーに対応しています。
詳しいセットアップ手順はこちらを参照してください：

👉 **[LLM セットアップガイド](./docs/llm-setup_ja.md)** | [English](./docs/llm-setup.md)

### 開発対象ディレクトリの設定

ヘッダー左側のプロジェクト領域からワークスペースを選択し、フォルダボタンで対象ディレクトリを設定します。
このローカルパスが、各 Dev Agent の実作業ディレクトリになります。

---

## 技術スタック

| レイヤー | 技術 |
|----------|------|
| フロントエンド | React 19, TypeScript, Tailwind CSS v4 |
| バックエンド | Tauri v2 (Rust) |
| データベース | SQLite（ローカル） |
| 状態管理 | React Context / Hooks |
| AI | Claude Code CLI, Gemini CLI, Codex CLI, Anthropic API, Gemini API, OpenAI API, Ollama |
| ターミナル | xterm.js |
| UIアイコン | Lucide React |

---

## 開発

```bash
npm run dev             # Vite 開発サーバー
npm run build           # フロントエンドビルド
npm run lint            # ESLint
npm run tauri -- dev    # Tauri アプリを開発起動
npm run tauri -- build  # Tauri アプリを本番ビルド
```

設計方針や開発ルールは [Rule.md](./Rule.md) を参照してください。
アーキテクチャ全体の考え方は [ARCHITECTURE.md](./ARCHITECTURE.md) に整理されています。

---

## 名前の由来

この名前には二つの由来があります。

1. **毘羯羅（ビカラ）大将** — 十二神将のひとつで、干支の始まりである子を象徴し、「光明普照」の徳によって世界を照らし、正しい始まりへ導く存在。
2. **Vicāra** — サンスクリット語 / 英語で「思考・熟慮・調査・計画」を意味する言葉。

vicara は、この二つの意味を重ね合わせて設計されています。

---

## ライセンス

このプロジェクトは [Apache License 2.0](./LICENSE) の下でライセンスされています。

---

## リリースノート

最新リリース：
- [vicara v2.2.0](./releases/v2.2.0.md)

他の言語のドキュメント：
- [🇯🇵 日本語 (メイン)](./README.md)
- [🇺🇸 English (英語版)](./README_en.md)

---

*vicara v2.2.0 — Human-led AI team orchestration for solo builders.*
