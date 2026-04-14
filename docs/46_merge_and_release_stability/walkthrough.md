# EPIC46 ドキュメント確認メモ

## このフォルダで整理したこと

`docs/46_merge_and_release_stability` には、EPIC46 向けの planning package として以下を配置した。

- `task.md`: EPIC の背景、スコープ、ストーリー別タスク一覧
- `implementation_plan.md`: 実装方針、実施ステップ、リスク、テスト方針
- `walkthrough.md`: 今回の調査結果と、計画へどう反映したかの確認メモ

## 調査結果の反映内容

### 1. worktree マージ失敗

今回のマージ失敗は、Prisma タスクそのものの内容だけが原因ではなく、アプリが project root の tracked な `.gitignore` を変更していたことが直接要因だった。

計画には、以下を反映している。

- `src-tauri/src/worktree.rs` の worktree ignore 管理先を `.gitignore` から `.git/info/exclude` へ移す
- `merge_worktree` 実行前に project root の dirty 状態を検知する
- 既存 repo に残っている app 起因の `.gitignore` 差分を安全条件付きで移行する

### 2. Release の macOS / Linux 停止

`.github/workflows/release.yml` では、Windows 以外の matrix がコメントアウトされており、コメント上も `claude_runner.rs` 側の修正後に再有効化する前提になっていた。

計画には、以下を反映している。

- `src-tauri/src/claude_runner.rs` の `ProcessKiller` 格納制約を見直す
- `portable-pty` の `MasterPty` / `SlavePty` が `Sync` を満たさない前提で Unix 実装を成立させる
- その後に `.github/workflows/release.yml` の macOS / Linux matrix を復帰する

## 現時点の推奨実装順

1. worktree ignore の保存先を `.git/info/exclude` へ移行する
2. merge preflight check を追加する
3. `claude_runner.rs` の `Sync` 制約問題を解消する
4. Release workflow の macOS / Linux を再有効化する

この順序にしておくと、ユーザー影響の大きい merge 失敗を先に止血しつつ、Release 復旧の前提条件も段階的に整えられる。

## 補足

- 本フォルダはドキュメント追加のみであり、実装変更はまだ行っていない
- Release 再有効化は、`claude_runner.rs` の Unix ビルド問題を先に解消する前提で計画している
- コード署名や notarization は本 EPIC の対象外として切り分けている

## 実装ログ

### Story 1 前半: worktree ignore 管理の切り替え

- `src-tauri/src/worktree.rs` に `.git/info/exclude` へ `.vicara-worktrees/` を追記する `ensure_local_exclude_entry` を追加した
- `create_worktree` は tracked な `.gitignore` を触らず、ローカル専用の ignore 管理へ切り替えた

### `.gitignore` 移行条件

- `migrate_legacy_worktree_gitignore` を追加し、先に `.git/info/exclude` を保証したうえで旧差分の掃除を行うようにした
- tracked な `.gitignore` は、HEAD と比較して差分が `.vicara-worktrees/` の追加だけに見える場合のみ自動で元へ戻す
- untracked な `.gitignore` は、内容が `.vicara-worktrees/` 由来だけの場合のみ削除する
- `.gitignore` に他の未コミット変更が混在している場合は自動修正せず、そのまま残して後段の preflight で止める方針にした

### merge preflight

- `merge_worktree` の開始時に legacy `.gitignore` 差分の移行を試し、その後 `git status --porcelain` で project root の dirty 状態を確認するようにした
- dirty な場合は Git merge を実行せず、`commit / stash / cleanup` を案内するエラーメッセージを返す
- `.gitignore` に旧 `.vicara-worktrees/` 行が残っている場合は、その可能性もメッセージに含めて原因を推測しやすくした

### Story 2: PTY 同期境界の整理

- `AgentSession.killer` は `Arc<Mutex<HashMap<...>>>` 配下で単独所有されており、trait object 自体へ `Sync` を課す必要はなかった
- そのため格納型を `Box<dyn ProcessKiller + Send>` に整理し、Unix の `PtyChildKiller` でも `portable-pty` の非 `Sync` 制約に引っかからない形へ変更した
- Windows 側の `StdChildKiller`、Unix 側の `wait_success` / `kill` / temp file cleanup の呼び出し順は維持しており、所有境界だけを絞る変更に留めた

### Release workflow 復旧

- `.github/workflows/release.yml` で macOS Intel / Apple Silicon、Ubuntu、Windows の matrix を再び有効化した
- macOS は matrix ごとに Rust target を追加する step を挟み、Ubuntu は Tauri v2 向けの system dependency を workflow 上で明示した
- 旧コメントは「一時停止中のメモ」から「draft release で全 desktop matrix を継続検証する」という運用メモへ更新した

### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml` は 75 件すべて成功した
- `npm run build` は成功した。Vite の chunk size warning は出るが、今回差分による build failure はない
- Story 1 では以下の回帰観点を追加確認した
  - `.git/info/exclude` への idempotent な追記
  - tracked / untracked `.gitignore` の legacy 差分移行
  - dirty project root の preflight 停止
  - task branch 側が `.gitignore` を変更する merge の正常系

### 未実施のローカル確認

- `rustup target list --installed` では `x86_64-pc-windows-msvc` のみが入っており、macOS / Linux target の cross-check はローカルでは実行できなかった
- そのため `claude_runner.rs` の Unix ビルド復旧は、今回の `Sync` 制約除去と release workflow 再有効化を反映したうえで、GitHub Actions の macOS / Linux job で最終確認する前提にしている
