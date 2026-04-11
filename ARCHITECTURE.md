# ARCHITECTURE.md — vicara 設計ドキュメント

> このドキュメントは、現在のシステムにおける技術水準と実装アーキテクチャをまとめたものです。
> 将来のエージェント実装などのアイデア構想は `FUTURE_CONCEPT.md` を参照してください。

---

## コアコンセプト

### スクラムを人間とAIの共通プロトコルとして使う

スクラムは「人間チームのためのフレームワーク」として生まれましたが、
vicara では**人間とAIが協働するための共通言語**として再解釈します。

- 人間は「何を作るか」と「どの道筋で進めるか」を決める
- AIは「どう作るか・どう検証するか」を担う
- スクラムのプロセスが、その協働の透明性と意思決定の一貫性を担保する

---

## 技術スタック

- **フロントエンド**: React + Vite + TypeScript + TailwindCSS
- **バックエンド**: Tauri (Rust)
- **データベース**: SQLite (tauri-plugin-sql + sqlx)
- **LLM統合**: rig crate (Anthropic / Gemini / OpenAI / Ollama)
- **外部CLI連携**: Claude Code CLI / Gemini CLI / Codex CLI

---

## 処理境界

```text
フロントエンド（React）
│  カンバン表示・進捗確認・人間の操作
│
│  invoke("command_name", { ... })
│
Rustバックエンド（Tauri）         ← 実際の処理はここで行う
├── AI API呼び出し（rig crate）
├── 外部CLI実行（std::process / portable-pty）
├── ファイル操作（std::fs）        ← バリデーション後に実行
├── Git操作（std::process経由）
└── SQLite読み書き（sqlx）
```

**設計方針：フロントエンドに直接のシェル実行権限は与えない。**
AIの指示をRust側で受け取り、バリデーションを挟んでから実行することで、
AIの誤操作によるファイル破壊などのリスクを防ぐ。

---

## AIロールの責務分離

vicara の AI は2つの責務に分かれる。

- **POアシスタント**: プロダクトオーナーの判断を補佐する。壁打ち・タスク生成・チャットを担う。API または CLI で動作（設定で切替可能）。
- **開発エージェント**: CLI経由でタスクを自律実行する。Git Worktree上で動作し、完了後に main へマージする。5つのロールテンプレートがあるが、アーキテクチャ上の責務は同一。

---

## マルチCLI実行基盤

- `CliRunner` trait で Claude / Gemini / Codex を抽象化。新しいCLIを追加する場合はこの trait を実装する。
- **Windows**: `std::process::Command` + パイプ方式（ConPTYの制限を回避）
- **Unix**: `portable-pty` による仮想ターミナル（PTY）実行
- 180秒の強制タイムアウト + フロントエンドからの Kill 機構あり

---

## Git Worktree によるタスク分離

- 開発エージェントはタスクごとに git worktree を作成し、隔離された環境で実行する。
- 完了後に main ブランチへ `--no-ff` マージ → worktree 削除。
- worktree の状態は DB (`worktrees` テーブル) で管理し、プレビューサーバーとも連携する。

---

## LLM使用量記録

- **すべてのLLM呼び出しは `llm_observability` モジュール経由で記録すること。**
- API経由の呼び出し: `record_llm_usage` でトークン数・コストを記録
- CLI経由の呼び出し: `record_claude_cli_usage` で実行時間を記録（トークン数は取得不可）
- プロジェクト / タスク / スプリント単位で集計可能

---

## 状態管理

### フロントエンド（React Context）

- `WorkspaceContext`: 現在のプロジェクト選択・プロジェクト一覧を管理
- `ScrumContext`: ストーリー・タスク・スプリントのCRUD操作と状態を管理
- `SprintTimerContext`: スプリントタイマーの実行状態（RUNNING / PAUSED / STOPPED）を管理

### バックエンド（Tauri State）

- `AgentState`: 実行中の開発エージェントセッション（タスクID → セッション情報）を保持
- `WorktreeState`: アクティブな git worktree の一覧を保持（最大5件）
- `PtyManager`: ターミナルDock用のPTYセッションを管理（5分ごとの自動クリーンアップあり）
- `PreviewState`: worktree 上のプレビューサーバー（port / pid）を管理

---

## 進行中アクションの安全機構（Interaction Guard）

スプリントタイマーやAIエージェントなどの「進行中のアクション」がある状態で、ユーザーがプロジェクトを切り替えようとした場合、データの不整合を防ぐための安全機構。

1. **状態の検知**: 各Contextからアクティブな処理（タイマーRUNNING、エージェント実行中など）がないかチェック
2. **ユーザーへの確認**: アクティブな処理が検出された場合、確認ダイアログを表示
3. **安全な停止と切り替え**: 同意があれば進行中の処理を停止してから切り替え、同意がなければキャンセル

**新しい「進行中のアクション」を追加する場合、この安全機構への統合を忘れないこと。**
