# Epic 34: 新生「vicara」への全面変更 Workthrough

## 概要

Epic 34 では、既存プロダクト `ai-scrum-tool` / **MicroScrum AI** を、新ブランド **vicara** へ全面移行した。  
単なる表示名の置換ではなく、Tauri の app identifier、DB 名、worktree ディレクトリ、crate 名、npm パッケージ名、localStorage key を含む内部識別子まで刷新し、将来の技術的負債を残さない形でリブランディングを完了した。

## 実施した流れ

### 1. 現状調査

- 旧サービス名の残存箇所を主要コード・設定・ルートドキュメントから洗い出した。
- PO 指示の旧 Identifier は `com.microscrum.dev` だったが、実装上の現在値は `com.green.ai-scrum-tool` であることを確認した。
- 旧 DB 名が `ai-scrum.db`、旧 localStorage key が `microscrum.layout.*`、旧 worktree ディレクトリが `.scrum-ai-worktrees` であることを確認した。
- ルートの主要設計書ファイルが `Architecture.md` で、本文や参照側に `ARCHITECTURE.md` 表記が混在していたため、名称統一も同 Epic の対象に含めた。

### 2. ブランド思想の反映

- `README.md` 冒頭に、毘羯羅大将の「正しい始まり」「光で道を照らす」イメージを盛り込んだ。
- `vicara` の語源である「思考・熟慮・調査・計画」を、人間中心の意思決定支援という思想と接続した。
- 「AI が勝手に進めるのではなく、人間が複数 AI を率いる」というプロダクトの立ち位置を明文化した。

### 3. ルートドキュメントの更新

- `README.md`
- `ARCHITECTURE.md`
- `PRODUCT_CONTEXT.md`
- `FUTURE_CONCEPT.md`
- `Rule.md`
- `BACKLOG.md`

上記の主要ドキュメントを `vicara` 基準へ統一した。  
あわせて `Architecture.md` は `ARCHITECTURE.md` へリネームし、参照側との不整合を解消した。

### 4. 設定・パッケージ・内部 Identifier の刷新

- `src-tauri/tauri.conf.json`
  - `productName` を `vicara`
  - window `title` を `vicara`
  - `identifier` を `com.vicara.app`
- `package.json`
  - npm パッケージ名を `vicara`
- `src-tauri/Cargo.toml`
  - crate 名を `vicara`
  - lib 名を `vicara_lib`
- `src-tauri/src/lib.rs` / `src-tauri/src/db.rs`
  - DB 名を `vicara.db`
- `src-tauri/src/worktree.rs`
  - worktree ディレクトリを `.vicara-worktrees`
- `src-tauri/src/git.rs`
  - 既定 Git identity を `vicara` 系名称へ更新

この方針により、表示名だけでなく、ローカルストレージ・ローカル DB・ビルド成果物・Git 上の識別も新ブランドに統一された。

### 5. UI の反映

- `index.html` のタイトルを `vicara` へ変更
- `src/App.tsx` のヘッダー・サブタイトル・Git 必須画面・usage pill 条件・localStorage key を更新
- `src/components/terminal/TerminalDock.tsx` の welcome message を更新
- その後の不具合修正として、Terminal Dock の完了セッション整理 UI、Inception Deck の Phase 巻き戻し挙動、usage 表示、競合 rerun 周りの worktree 制御も改善した

### 6. 履歴保全の判断

`docs/` 配下には旧ブランド名を含む過去 Epic の履歴が残っている。  
今回の Epic では、これらを過去時点のスナップショットとして保持する判断を採用し、**履歴保全のためにあえて変更しない**設計とした。

この判断により、現在の公式名称は `vicara` に統一しつつ、過去の意思決定ログはその時点の名称のまま追跡できる。

## 検証の軌跡

- 旧ブランド文字列の残存チェックを実施した。
- `npm run build` を実行し、フロントエンドビルド成功を確認した。
- `cargo test --manifest-path src-tauri/Cargo.toml` を実行し、Rust 側の回帰テスト成功を確認した。
- PO による最終動作確認で、`vicara` として正常起動・正常動作し、内部 Identifier の変更および DB の再生成が完璧に機能することを確認した。

## 結果

Epic 34 により、プロダクトは名称・思想・内部識別子のすべてにおいて **vicara** へ刷新された。  
以後、本プロダクトを旧名称で扱う必要はなく、今後の Epic はすべて `vicara` を正式名称として継続する。

