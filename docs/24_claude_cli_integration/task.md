# Epic 24: Task List

## 1. バックエンド (Rust) - PTY基盤とコマンド実装
- [x] `claude_runner.rs` の作成（PTYを用いてプロセスSpawn）
- [x] 標準出力 / エラー出力をキャプチャし、Tauri Event経由でストリーミング配信するロジック
- [x] 実行ディレクトリの制限（対象のプロジェクトルートへの固定）
- [x] 長期間ハングアップ用のタイムアウトガード機構（3分/180秒）の実装
- [x] 手動でのプロセス強制切断（Kill）ロジックの提供
- [x] Tauri Commands の公開: `execute_claude_task`, `kill_claude_process`

## 2. フロントエンド (React) - Claudeトリガーとログ表示
- [x] TaskCard (またはタスク詳細UI) への「開発を実行」ボタンの実装
- [x] ボタン押下時のタスクタイトル・説明などのコンテキスト抽出と、コマンド呼び出し
- [x] TerminalDock への Tauri Eventリッスン処理追加（リアルタイム出力の描画）
- [x] TerminalDock への「実行を強制停止（Kill）」ボタンの追加と `kill_claude_process` の呼び出し
- [x] 進行中のプロセス管理と、複数回意図せずクリックされないためのUI上での無効化処理 (isExecuting)

## 3. ステータス管理と自動遷移
- [x] 実行開始時: タスクのStatusを `In Progress` に強制更新
- [x] 実行完了時: プロセスが正常終了した際、自動的に `Review` ステータス（現在仕様により `Done`）へ遷移
- [x] いずれの更新も即時UIへ反映されるよう Optimistic Update との整合性を担保

## 4. テスト・手動検証 (Walkthrough準備)
- [x] MOCK出力プログラムでのストリーミング・PTYイベント動作確認
- [x] タイムアウト・強制終了による安全な停止と、エラー画面のフィードバックテスト
- [x] 本番環境同等の簡単なタスクを与えての ClaudeCLI 疎通テスト
