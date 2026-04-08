# Epic 34: 新生「vicara」への全面変更 タスクリスト

## ステータス

- 状態: `Done`
- クローズ日: 2026-04-08

## 実行順序と完了結果

### 1. ドキュメント更新
- [x] `README.md` を最優先で更新し、vicara の由来・思想・人間中心の意思決定支援というコンセプトを冒頭に反映した。
- [x] `Architecture.md` を `ARCHITECTURE.md` へ整理し、`PRODUCT_CONTEXT.md`、`FUTURE_CONCEPT.md`、`Rule.md`、`BACKLOG.md` の主要ドキュメント表記を新ブランドへ統一した。
- [x] ルート設計書の実ファイル名と参照表記の不整合を解消した。

### 2. 設定ファイル更新
- [x] `src-tauri/tauri.conf.json` の `productName`、`title`、`identifier`、DB preload 名を vicara 系へ変更した。
- [x] `package.json`、`src-tauri/Cargo.toml`、`src-tauri/src/main.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/db.rs` を更新し、パッケージ名・crate 名・DB 接続先を新ブランドへ揃えた。
- [x] `src-tauri/src/claude_runner.rs`、`src-tauri/src/git.rs`、`src-tauri/src/worktree.rs`、`src-tauri/tests/worktree_test.sh` を更新し、一時ファイル名、Git identity、worktree ディレクトリ名、コミットメッセージなどの内部識別子を一掃した。

### 3. UI 更新
- [x] `index.html` の `<title>` を `vicara` に変更した。
- [x] `src/App.tsx` のヘッダー表示、サブタイトル、Git 必須画面文言、localStorage キー、usage 表示条件を新ブランドへ更新した。
- [x] `src/components/terminal/TerminalDock.tsx` の welcome message / 復元メッセージを vicara に変更し、完了セッションの整理操作も追加した。
- [x] `src/components/project/InceptionDeck.tsx` を更新し、Phase 巻き戻し後の再生成不整合と `Ctrl+Enter` 表記の問題を修正した。

### 4. ビルドと新規 DB 生成の確認
- [x] 旧ブランド文字列の残存検索を実施した。
- [x] `npm run build` を実行してフロントエンドビルド成功を確認した。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` を実行して Rust 側の回帰を確認した。
- [x] Tauri を起動し、ウィンドウタイトル・表示名・新 app identifier・新 DB 生成が正常であることを PO が確認した。

## 完了条件の充足

- [x] 主要ドキュメント、設定ファイル、UI 上の旧名称が `vicara` に統一されている。
- [x] 内部識別子が新ブランド系へ切り替わり、旧ローカルデータを引き継がない状態になっている。
- [x] ビルドと主要テストが通り、新しい識別子でアプリが起動する。

## 補足

- 旧 Identifier の実装上の現在値は `com.microscrum.dev` ではなく `com.green.ai-scrum-tool` だったため、実値から `com.vicara.app` へ移行した。
- 過去 Epic 履歴ドキュメントの全面改名は、履歴保全のため既定スコープ外として維持した。

