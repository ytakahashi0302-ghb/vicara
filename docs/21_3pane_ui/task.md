# Epic 21 Task List

## Phase 1: パッケージ導入と基盤レイアウトの構築
- [x] 依存パッケージのインストール (`npm install xterm xterm-addon-fit`)
- [x] CSSエントリーポイント(`index.css` / Tailwind)やテーマのリファイン（全体のカラーパレット・タイポグラフィの最適化）
- [x] アプリケーションのルートとなるメインレイアウトコンポーネント(`App.tsx` または `MainLayout.tsx`)の分割
  - Left Pane (70%) と Right Pane (30%) への分割
  - Left Pane 内の Top (60%) と Bottom (40%) への分割
- [x] 既存の Kanban 関連コンポーネントを Left-Top エリアに埋め込み配置
  - カンバンの高さ制約とOverflow時の縦・横スクロールの挙動最適化

## Phase 2: Terminal Dock UI の実装とPTY統合
- [x] Tauri通信用カスタムフックの作成 (例: `src/hooks/usePtySession.ts`)
  - `pty_spawn` (初期化)
  - `pty_execute` (コマンド実行と出力のパース)
  - `pty_kill` (破棄)
- [x] ターミナル描画用コンポーネントの作成 (`src/components/terminal/TerminalDock.tsx`)
  - `xterm.js` と `xterm-addon-fit` を使ったターミナルの初期化処理と描画
  - Tailwind v4 を用いたターミナルコンテナのスタイリング (モダン・ダークテーマ風の装飾)
- [x] アプリ起動時のPTYマウント処理、終了時・アンマウント時のクリーンアップ処理の組み込み・確認

## Phase 3: PO Agent Sidebar の実装
- [x] 右サイドバー用コンポーネント (`src/components/sidebar/PoAgentSidebar.tsx`) の作成
- [x] チャットUI (履歴リスト領域、プロンプト入力エリア、送信ボタン) の構築
  - メッセージごとのUIコンポーネント作成（ユーザー側、AI側でスタイリングを切り替え）
- [x] バックエンド(`src-tauri/src/ai.rs`)で実装済みの Rig経由チャットAPI呼び出しの接続
- [x] APIリクエスト中のローディング表示、結果受領後のスクロール最下部への自動追従処理の追加

## Phase 4: UI/UXの磨き込みと最終テスト (Final Polish)
- [x] 3ペイン間の境界線（セパレータ）やシャドウ、Glassmorphism等のモダンUI要素の適用
- [x] マウスオーバー時のインタラクションや、各パネルの開閉時のマイクロアニメーション追加
- [x] デスクトップ版Tauriとしての総合的な結合テスト (E2Eに近い動作確認)
- [x] Windows特有の挙動確認 (必要に応じた改行コード対応、文字化けや `cd` の状態保持の検証)
- [x] 全体を通した正常系・異常系・エッジケースの検証
- [x] スプリントの完了確認と `walkthrough.md`, `handoff.md` の更新
