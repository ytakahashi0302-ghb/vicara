# Epic 43: トランスポート層統一 タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: Epic 42 完了（Phase 3-A 完了後）
- 作成日: 2026-04-09

## 概要

Phase 1〜3-A で段階的に追加してきた CLI / API / Local の各トランスポートを、統一された設定モデルで管理できるように整理する。全てのロール（PO アシスタント・Dev エージェント）で transport + provider + model を自由に選択できる最終形を完成させる。

## 実行順序

### 1. 設定モデルの統一
- [ ] 現状の設定が分散している問題を解決する:
  - PO アシスタント: `default-ai-provider`, `po-assistant-transport`, `po-assistant-cli-type` ...
  - Dev エージェント: `team_roles.cli_type`, `team_roles.model`
- [ ] 統一された設定スキーマを定義する:
  ```
  transport: "api" | "cli" | "local"
  provider:  "anthropic" | "gemini" | "openai" | "ollama" | "claude-cli" | "gemini-cli" | "codex-cli"
  model:     string
  ```

### 2. PO アシスタントのロール化（オプション）
- [ ] PO アシスタントを `team_roles` テーブルで管理するか検討する。
- [ ] 現状は settings.json でグローバル管理されているが、プロジェクトごとに PO の設定を変えたいケースがあるか評価する。
- [ ] 必要であれば、`team_roles` に `role_type` カラム（`"dev"` | `"po"`）を追加し、PO ロールも同一テーブルで管理する。

### 3. セットアップ状況タブの最終形
- [ ] `SetupStatusTab.tsx` を更新し、全トランスポート種別の状態を統一表示する:
  - CLI ツール: Claude Code / Gemini CLI / Codex CLI
  - API キー: Anthropic / Gemini / OpenAI
  - ローカル: Ollama 接続状態
  - 環境: Git
- [ ] 「推奨構成」セクションを追加し、ユーザーの環境に基づいた推奨設定を提案する。

### 4. チーム設定タブの最終形
- [ ] 各ロールの transport 設定を CLI / API / Local から選択可能にする。
- [ ] transport 変更時に provider とモデルの選択肢を動的に切り替える:
  - CLI → claude / gemini / codex + 各 CLI のモデル
  - API → anthropic / gemini / openai + 各 API のモデル
  - Local → ollama + ローカルモデル一覧

### 5. Observability の統合ビュー
- [ ] LLM Usage ダッシュボードに transport 別の集計を追加する。
- [ ] CLI（コスト不明）、API（従量課金）、Local（無料）の区別が分かるようにする。

### 6. ドキュメントと既定値の整理
- [ ] 全設定キーの一覧と既定値を整理する。
- [ ] マイグレーションで不要になった旧設定キーがあればクリーンアップする。

### 7. 動作確認
- [ ] 以下の組み合わせでチームを構成し、全機能が動作することを確認する:
  - PO: Ollama (ローカル) + Dev1: Claude Code CLI + Dev2: Gemini CLI
  - PO: OpenAI API + Dev1: Codex CLI + Dev2: Claude Code CLI
  - PO: Claude Code CLI + Dev1: Gemini CLI
- [ ] 設定変更が即座に反映されること。
- [ ] LLM Usage に正しい transport / provider / model が記録されること。
