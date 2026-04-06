# Epic 24: ClaudeCLI 連携機能 — 引き継ぎ書 (Handoff)

本ドキュメントは、カンバンタスクから Claude CLI (Claude Code) を起動して自律的に開発を実行する「自律実行基盤」の引き継ぎ用資料です。

## 🔍 機能概要
カンバン上のタスクカードに表示される「開発を実行」ボタンをクリックすることで、バックグラウンドで Claude CLI プロセスを起動し、タスクのタイトルと説明をプロンプトとして供給します。実行中のログはターミナル (`TerminalDock`) にリアルタイムでストリーミング表示され、完了するとタスクステータスが自動的に更新されます。

## 🏗️ 主要コンポーネントとその役割

### 1. [claude_runner.rs](file:///c:/Users/green/Documents/workspaces/ai-scrum-tool/src-tauri/src/claude_runner.rs) (Rust)
*   **役割**: プロセスの Spawn、PTY/Pipe の管理、出力ストリーミング、タイムアウト監視。
*   **技術詳細**:
    *   **プラットフォーム分岐**: Windows では `std::process::Command` + Pipe を、Unix では `portable-pty` を使用。
    *   **stdin プロンプト供給**: 改行・日本語・クォート等のエスケープ問題を回避するため、プロンプトは stdin 経由でプロセス起動直後に流し込みます。最後に EOT (`\x04`) を送ることで Claude に処理開始を促します。
    *   **抽象化**: `ProcessKiller` trait により、Windows/Unix のプロセス管理差異を吸収。

### 2. [TerminalDock.tsx](file:///c:/Users/green/Documents/workspaces/ai-scrum-tool/src/components/terminal/TerminalDock.tsx) (React)
*   **役割**: `xterm.js` によるログ描画と、実行中プロセスへの「強制停止 (Kill)」インターフェース提供。
*   **イベント**: `claude_cli_output` をリッスンし、`claude_cli_exit` で完了通知を受け取ります。

## ⚠️ 重要な技術的知見 (ConPTY 問題)
開発の過程で、Windows の仮想ターミナル (ConPTY) 実装 (`portable-pty`) が標準出力のキャプチャにおいて特定条件下でハングすることが判明しました。これに伴い、Windows 環境では PTY をあえて使用せず、標準の `Child` プロセスとスレッドベースのパイプ読み取りを選択しています。今後、同様のインタラクティブツールを統合する際も、Windows では PTY を避けるか、十分に検証する必要があります。

## 🔧 依存関係とセットアップ
*   **必須ツール**: Anthropic の **Claude Code CLI (`@anthropic-ai/claude-code`)**
*   **初期設定**:
    1.  `npm install -g @anthropic-ai/claude-code` (または同等の手順) でインストール。
    2.  `claude login` コマンドで事前に認証を済ませておく必要があります。
    3.  プロジェクト設定 (Settings) で、対象プロジェクトの `Local Path` が正しく設定されていることを確認。

## 🚀 今後の課題（技術的負債）
*   **ユーザー入力へのブリッジ**: 現状は `bypassPermissions` モードによる全自動実行ですが、将来的にユーザーがターミナル上で Y/N 等の入力を行うための PTY 書き込みブリッジの拡張が検討されています。
*   **ステータス遷移の柔軟化**: 現在は完了時に一律 `Done` へ遷移しますが、プロジェクトの設定やルールにより `Review` への遷移を選択可能にする余地があります。
