# Epic 25: AI自律開発のための初期足場（Scaffolding）構築機能 — 実装計画

## 背景と課題

インセプションデッキ完了後、プロジェクトディレクトリには PRODUCT_CONTEXT.md / ARCHITECTURE.md / Rule.md の3ファイルしか存在しない。この状態で Claude CLI にタスクを実行させると、AIがディレクトリ構造を自己流で作成し、プロジェクト構造がカオスになる。

**解決策**: タスク実行前に「Scaffolding（初期足場構築）」と「AGENT.md（AIへの道しるべ）」を自動配置する仕組みを実装する。

---

## アーキテクチャ概要

### 処理フロー

```
InceptionDeck Phase 5 完了
    ↓
[Frontend] ScaffoldingPanel（Phase 6的な位置づけ）
    ↓
[Backend] detect_tech_stack → ARCHITECTURE.md解析
    ↓
[判定] CLIスキャフォールド or AI生成
    |                          |
    ↓                          ↓
pty_execute(npx create-vite)   execute_claude_task(構造生成)
    ↓                          ↓
[Backend] generate_agent_md → AGENT.md生成
    ↓
[Backend] generate_claude_settings → .claude/settings.json生成
    ↓
[Frontend] 完了表示 + ディレクトリツリープレビュー
```

### 新規モジュール: `src-tauri/src/scaffolding.rs`

`inception.rs`はファイルI/O特化のため、Scaffoldingは責務分離で新モジュールとする。プロセス実行制御・技術スタック検出・マルチステップワークフローという異なる責務を持つ。

---

## 設計詳細

### 1. 技術スタック検出エンジン

ARCHITECTURE.md をパースし、フレームワークキーワードで Scaffolding 戦略を決定する。

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechStackInfo {
    pub language: Option<String>,
    pub framework: Option<String>,
    pub meta_framework: Option<String>,
    pub raw_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ScaffoldStrategy {
    CliScaffold { command: String, args: Vec<String> },
    AiGenerated { prompt: String },
}
```

**検出マッピング:**

| 検出キーワード | 戦略 | コマンド例 |
|---|---|---|
| React + Vite | CliScaffold | `npx create-vite@latest . --template react-ts` |
| Next.js | CliScaffold | `npx create-next-app@latest . --ts --app` |
| Vue + Vite | CliScaffold | `npx create-vite@latest . --template vue-ts` |
| Rust (standalone) | CliScaffold | `cargo init .` |
| その他 / バニラ | AiGenerated | Claude CLIにプロンプトで生成指示 |

**安全策**: Inception出力の3ファイル以外が既に存在する場合、フロントエンドで警告ダイアログを表示。

### 2. AGENT.md生成（参照ポインタ方式）

AGENT.md は3つのInceptionファイルの**内容をコピーせず、参照リンクのみ**を持つ。これによりInceptionファイル編集時に二重管理が発生しない（Single Source of Truth）。

AGENT.md 固有の情報は「ディレクトリ構造ガイド」（Scaffolding後のtree出力）のみ。

```markdown
# AGENT.md — {project_name}
> AIコーディングエージェントへの統合指示書。作業前に必ず本ファイルと参照先を読むこと。

## 必読ドキュメント
- [PRODUCT_CONTEXT.md](./PRODUCT_CONTEXT.md) — プロジェクトの目的と方向性
- [ARCHITECTURE.md](./ARCHITECTURE.md) — システム構成と技術スタック
- [Rule.md](./Rule.md) — コーディング規約と開発ルール

## ディレクトリ構造ガイド
{Scaffolding完了後にtree出力を挿入}

## ワークフロー
- 実装前に上記3ファイルを必ず読むこと
- 変更完了時は walkthrough.md を出力すること
```

### 3. Claude専用設定ファイル (.claude/settings.json)

```json
{
  "customInstructions": "必ず AGENT.md を読んでから作業を開始してください。AGENT.md にはプロジェクトの概要、アーキテクチャ、コーディング規約が記載されています。"
}
```

### 4. UI設計

**InceptionDeck Phase 5を拡張** → ScaffoldingPanel を表示。

- Phase 5完了時に ScaffoldingPanel を表示
- 検出された技術スタックと実行戦略をプレビュー
- 実行中は TerminalDock に出力をストリーミング
- 完了後に AGENT.md の内容をプレビュー表示
- `ProjectSettings` にも手動トリガーボタンを追加（再実行用）

```typescript
type ScaffoldingStatus = 'idle' | 'executing' | 'generating' | 'completed' | 'error';
```

---

## 新規Tauriコマンド

| コマンド | 用途 |
|---|---|
| `detect_tech_stack(local_path)` | ARCHITECTURE.md解析→技術スタック検出 |
| `execute_scaffold_cli(local_path, command, args)` | PTY経由でCLIスキャフォールド実行 |
| `execute_scaffold_ai(local_path, tech_stack_info)` | Claude CLI経由でAI生成スキャフォールド |
| `generate_agent_md(local_path, project_name)` | AGENT.md生成 |
| `generate_claude_settings(local_path)` | .claude/settings.json生成 |
| `check_scaffold_status(local_path)` | 既存Scaffold有無チェック |

---

## 変更対象ファイル

| ファイル | 変更種別 | 内容 |
|---|---|---|
| `src-tauri/src/scaffolding.rs` | **NEW** | Scaffoldingモジュール本体 |
| `src-tauri/src/lib.rs` | MODIFY | mod宣言 + コマンド登録 |
| `src/components/project/ScaffoldingPanel.tsx` | **NEW** | ScaffoldingUI |
| `src/components/project/InceptionDeck.tsx` | MODIFY | Phase 5でScaffoldingPanel表示 |
| `src/components/ui/ProjectSettings.tsx` | MODIFY | 手動トリガーボタン追加 |

---

## リスクと対策

| リスク | 対策 |
|---|---|
| npx未インストール | 実行前にコマンド存在チェック。失敗時はAiGenerated戦略にフォールバック |
| 既存ファイル上書き | Inception出力以外のファイル検出時に確認ダイアログ |
| ARCHITECTURE.md非標準フォーマット | 検出失敗時はAiGenerated戦略をデフォルト適用 |
| スキャフォールドの長時間実行 | PTYストリーミングでリアルタイム進捗表示。タイムアウト300秒 |

---

## 再利用する既存基盤

- `inception.rs`: `read_inception_file()`, `write_inception_file()` — ファイルI/Oパターン
- `claude_runner.rs`: `execute_claude_task()` — Claude CLI実行パターン
- `pty_manager.rs` + `pty_commands.rs`: `pty_execute()` — シェルコマンド実行
- `InceptionDeck.tsx`: Phase管理UIパターン
