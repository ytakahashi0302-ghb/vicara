# Epic 34: 新生「vicara」への全面変更 実装計画

## ステータス

- 状態: `Done`
- クローズ判断: PO による最終動作確認完了
- Epic 完了日: 2026-04-08

## Epic の到達点

既存プロダクト **MicroScrum AI / ai-scrum-tool** を、新ブランド **vicara（ビカラ）** へ全面移行した。  
今回の Epic では、表示名だけでなく、ローカルデータ・ビルド成果物・内部識別子・パッケージ名に至るまで旧ブランドを残さない方針を採用し、アプリ再起動・内部 Identifier 変更・新規 DB 生成までを完了した。

## ブランド方針の実装結果

### 新名称
- 正式名称: `vicara`
- 読み: `ビカラ`

### README に反映したコンセプト
- 毘羯羅大将の「正しい始まり」「光で道を照らす」イメージを、ソロ開発者が迷わず前進する起点として言語化した。
- `vicara` が持つ「思考・熟慮・調査・計画」の意味を、人間中心の意思決定支援プロダクトとして接続した。
- AI が勝手に進めるのではなく、人間が複数 AI を率いて、実装と検証を正しい道筋に乗せる開発環境であることを README 冒頭で明確化した。

## 対象別の完了内容

### 1. ドキュメントレイヤー

#### 完了
- `README.md` を `vicara` 基準へ全面更新した。
- `ARCHITECTURE.md` へ名称統一し、旧 `Architecture.md` との不整合を解消した。
- `PRODUCT_CONTEXT.md`、`FUTURE_CONCEPT.md`、`Rule.md`、`BACKLOG.md` の主要表記を新ブランドへ統一した。
- README 冒頭に、由来・思想・人間中心の意思決定支援というコンセプトを反映した。

#### 補足
- `docs/` 配下の過去 Epic 履歴は、履歴保全のためスコープ外として維持した。

### 2. 設定・ビルドレイヤー（Tauri / Rust / パッケージ名）

#### 完了
- `src-tauri/tauri.conf.json` の `productName`、`title`、`identifier` を `vicara` / `com.vicara.app` へ更新した。
- `package.json` の npm パッケージ名を `vicara` へ更新した。
- `src-tauri/Cargo.toml` の crate 名 / lib 名を `vicara` / `vicara_lib` へ更新した。
- `src-tauri/src/main.rs` の lib 参照を新名称へ追随させた。
- `src-tauri/src/lib.rs`、`src-tauri/src/db.rs` の DB 名を `vicara.db` へ更新した。
- `src-tauri/src/claude_runner.rs`、`src-tauri/src/git.rs`、`src-tauri/src/worktree.rs`、`src-tauri/tests/worktree_test.sh` を更新し、一時ファイル名、Git identity、worktree ディレクトリ名、コミットメッセージ、競合再実行時の worktree 制御を新ブランド前提で整理した。

### 3. フロントエンドレイヤー（React / HTML）

#### 完了
- `index.html` の `<title>` を `vicara` に変更した。
- `src/App.tsx` のヘッダー表示、サブタイトル、Git 必須画面、usage pill 条件、localStorage キーを新ブランド基準に更新した。
- `src/components/terminal/TerminalDock.tsx` の welcome message、完了済みセッション整理 UI、スクロール視認性を改善した。
- `src/components/project/InceptionDeck.tsx` を更新し、フェーズ巻き戻し時の文書再生成と `Ctrl+Enter` 表記を正しく扱うよう修正した。

## ローカルデータ移行方針の結果

- PO 判断に基づき、旧 DB / 旧 localStorage / 旧 app identifier と互換を持たせない設計で実装した。
- その結果、DB 名・localStorage key・アプリ identifier・worktree ディレクトリ名はすべて `vicara` 系名称へ切り替えられた。
- PO による最終確認にて、内部 Identifier の変更および DB の再生成が正常に機能することを確認済み。

## スコープ判断

### 対象外として維持したもの
- `docs/` 配下の過去 Epic 履歴ドキュメント

### 判断理由
- 当時の意思決定と実装のスナップショットとして残す価値が高く、今回のブランド刷新後も履歴として参照できる状態を優先した。

## テスト方針と実施結果

### 文字列・識別子の残存確認
- 実施済み
- 主要コード・設定・ルートドキュメントに対し、`MicroScrum AI`、`ai-scrum-tool`、`ai_scrum_tool`、`.scrum-ai-worktrees`、`sqlite:ai-scrum.db`、`com.green.ai-scrum-tool` の残存チェックを実施した。

### ビルド・テスト確認
- `npm run build`: 完了
- `cargo test --manifest-path src-tauri/Cargo.toml`: 完了
- Tauri アプリの起動とブランド反映確認: PO による最終確認完了

### 手動確認
- ウィンドウタイトル、HTML タイトル、ヘッダー表示、Terminal Dock 文言が `vicara` に統一されていることを確認した。
- 新しい app identifier で起動し、旧ローカルデータと分離された新 DB が生成されることを確認した。
- worktree 作成・自動コミット・マージコミットのプレフィックスが新ブランド名に揃っていることを確認した。

## クローズ判断

Epic 34 の目的であった「ブランド名・内部識別子・主要ドキュメント・ローカルデータ基盤の完全刷新」は完了した。  
PO による最終動作確認も完了したため、本計画は **Done** としてクローズする。

