# EPIC46 実装計画

## 概要

EPIC46 では、現在別々に見えている以下の2問題を、どちらも「ローカル状態をアプリ都合で壊さない」「クロスプラットフォーム前提の実装境界を明確にする」という観点でまとめて改善する。

1. worktree 作成時に tracked な `.gitignore` を変更してしまうため、タスク branch 側でも `.gitignore` を変更した場合に merge が停止する問題
2. `claude_runner.rs` の Unix PTY 実装が `portable-pty` の `MasterPty` / `SlavePty` 非 `Sync` 制約により macOS / Linux の Release ビルドを再有効化できない問題

## 現状整理

### 1. マージ失敗の根本原因

- `src-tauri/src/worktree.rs` の `create_worktree` は、worktree 作成前に `.vicara-worktrees/` を ignore 対象へ追加している
- その保存先が project root の tracked な `.gitignore` であるため、ユーザーが commit していない限り、main 側に未コミット変更が残る
- `merge_worktree` は task worktree 側のみ自動コミットし、project root の dirty 状態はそのまま `git merge` を実行する
- その結果、task branch 側も `.gitignore` を変更していると、Git が「local changes would be overwritten by merge」で停止する

### 2. Release の macOS / Linux が止まっている原因

- `.github/workflows/release.yml` では、macOS / Linux matrix がコメントアウトされ、Windows のみ有効化されている
- `src-tauri/src/claude_runner.rs` では `AgentSession.killer` が `Box<dyn ProcessKiller + Send + Sync>` になっている
- Unix 実装の `PtyChildKiller` は `portable-pty` の `MasterPty` / `SlavePty` を保持しているが、これらは `Sync` を満たさない
- そのため Unix ビルドでは `PtyChildKiller` を `Send + Sync` な trait object として保持できず、非 Windows ビルド再有効化の障害になっている

## 実装方針

### 方針 A: worktree 用 ignore は `.git/info/exclude` に移す

#### 目的

アプリ都合の ignore 追加を Git 管理対象ファイルから切り離し、project root を不要に dirty にしない。

#### 方針

- `.vicara-worktrees/` の管理先を `.gitignore` ではなく `.git/info/exclude` に変更する
- `ensure_gitignore_entry` 相当の責務を、`ensure_local_exclude_entry` のようなローカル専用処理へ置き換える
- 既存 repo で `.gitignore` に app 由来の追記だけが残っている場合は、安全条件を満たすときのみ移行・除去する

#### 移行条件の案

- `.gitignore` の差分が app 管理の `.vicara-worktrees/` 追加に限定される場合のみ自動掃除する
- 他の未コミット変更が混在している場合は自動修正せず、ユーザー向けメッセージで案内する

### 方針 B: マージ前 preflight check を追加する

#### 目的

`git merge` 実行後に低レベルな Git エラーを返すのではなく、事前に user-friendly な理由で止める。

#### 方針

- `merge_worktree` の開始時に project root の `git status --porcelain` を確認する
- dirty の場合は merge を実行せず、以下を含むメッセージで停止する
  - project root に未コミット変更があること
  - `.gitignore` の app 管理差分が残っている可能性
  - 必要なら commit / stash / cleanup を先に行うこと
- 競合による失敗と、dirty state による事前停止を明確に分ける

### 方針 C: `ProcessKiller` の同期境界を見直す

#### 目的

Unix の PTY ハンドルに `Sync` を強制しない設計へ整理し、Windows / Unix の両方で妥当な trait bound にする。

#### 方針

- `AgentSession` は `Arc<Mutex<HashMap<...>>>` 配下で排他管理されているため、`killer` 自体に `Sync` を要求する必要性を再確認する
- `ProcessKiller` の格納型は、原則 `Box<dyn ProcessKiller + Send>` を候補とする
- 既存の kill / wait_success 呼び出し箇所は、いずれもセッション取り出し後に単独所有で扱っているため、`&mut self` ベースのままでも整合が取れるかを確認する
- 必要に応じて Windows / Unix で補助ラッパーを分け、`Sync` を不要化したうえで責務を明確にする

#### 期待効果

- `PtyChildKiller` が `MasterPty` / `SlavePty` の非 `Sync` 制約に引っかからなくなる
- `release.yml` で macOS / Linux matrix を戻せる前提が整う

### 方針 D: Release ワークフローを段階的に再有効化する

#### 目的

Windows の現行リリースを壊さずに、macOS / Linux を安全に戻す。

#### 方針

- `claude_runner.rs` の修正完了後に `.github/workflows/release.yml` の matrix を再有効化する
- macOS は Intel / Apple Silicon の両 target を定義する
- Ubuntu は Tauri ビルド前提の依存パッケージを維持する
- 現在のコメントは「再有効化条件のメモ」から「メンテナンス上の補足」へ更新する
- 初回は draft release のまま運用し、asset 出力確認を優先する

## 実施ステップ

### Step 1: worktree ignore 管理の置き換え

- `.gitignore` 書き込み処理を `.git/info/exclude` 書き込みへ置換する
- worktree 作成時に project root が dirty にならないことを確認する
- 既存 `.gitignore` 差分の移行条件を実装する

### Step 2: merge preflight check の実装

- `merge_worktree` 前に dirty check を入れる
- 競合時、dirty 時、branch 不在時のメッセージを整理する
- UI に返る文言の差別化を確認する

### Step 3: Unix PTY の trait bound 修正

- `AgentSession.killer` と `ProcessKiller` の格納制約を見直す
- Unix の `PtyChildKiller` がビルド可能になるように調整する
- timeout kill / manual kill / wait_success の経路を確認する

### Step 4: Release matrix 再有効化

- `release.yml` の macOS / Linux エントリを復帰させる
- Rust target / Ubuntu dependencies / release notes step が成立することを確認する
- コメントを最新の運用状態に合わせて更新する

## リスクと対策

### リスク 1: `.gitignore` 自動掃除がユーザー変更を巻き込む

- 自動掃除は差分限定時のみ実施する
- 判定不能時は自動変更せず、メッセージ案内へフォールバックする

### リスク 2: `Sync` 除去が別の共有前提を壊す

- `killer` 利用箇所を先に洗い出し、単独所有でしか使っていないことを確認してから変更する
- 共有参照が必要な箇所が見つかった場合は `Mutex` などで責務を明示する

### リスク 3: Release matrix 再開後に OS 固有依存で失敗する

- 初回は draft release 運用のままにする
- Ubuntu / macOS の失敗時に切り戻しやすいよう、workflow コメントと runbook を残す

## テスト方針

### 自動テスト

- worktree 作成後に project root の Git status が clean のままであることを確認するテストを追加する
- `.gitignore` を変更する branch の merge 前提ケースで、dirty check と正常 merge の両方を検証する
- 既存 repo の `.gitignore` 移行ロジックについて、対象差分のみ除去されることをテストする
- `cargo test` で既存の worktree / Git 関連テストが回帰しないことを確認する
- Release workflow では Windows / macOS / Linux の matrix が少なくとも build 開始可能な定義になっていることを確認する

### 手動確認

- InspectionDeck から `.gitignore` を変更するタスクを作成し、merge が app 起因の local change で止まらないことを確認する
- project root に意図的な未コミット変更を残した状態で merge を試し、分かりやすい案内が返ることを確認する
- tag push による draft release で、Windows / macOS / Linux の job が起動することを確認する
- Release asset とログを確認し、非 Windows でも `claude_runner.rs` 起因のビルド失敗が再発しないことを確認する

## 成果物

- `src-tauri/src/worktree.rs` の ignore 管理・merge preflight 改善
- `src-tauri/src/git.rs` の補助関数追加または整理
- `src-tauri/src/claude_runner.rs` の Unix PTY セッション管理修正
- `.github/workflows/release.yml` の macOS / Linux 再有効化
- 回帰テストと運用コメント更新
