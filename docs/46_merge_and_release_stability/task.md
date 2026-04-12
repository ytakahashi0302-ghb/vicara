# EPIC46: Merge Stability and Cross-Platform Release Recovery

## 背景

InspectionDeck から実施した実開発検証において、`Sample project` の「PrismaによるマルチDB対応のタスク永続化層の構築」配下タスク「Prismaスキーマの定義とマイグレーション」のマージ時に、プロジェクトルートの `.gitignore` ローカル変更が原因でマージが停止した。

同時に、GitHub Actions の Release ワークフローは Windows のみ有効化されており、macOS / Linux は `claude_runner.rs` における `portable-pty` の `Sync` 制約問題が解消されるまで再有効化できない状態である。

本 EPIC では、以下の2つをまとめて解消する。

- Git worktree ベース開発時のマージ安定性改善
- GitHub Release のマルチプラットフォーム再有効化

## ゴール

- アプリ起因で tracked な `.gitignore` が汚れ続けないようにする
- マージ実行前に失敗要因を事前検知し、ユーザーに分かりやすく案内できるようにする
- 既存プロジェクトに残っている app 起因の `.gitignore` 差分を安全に移行できるようにする
- `claude_runner.rs` の Unix 実装を macOS / Linux でもビルド可能な形に整理する
- Release ワークフローで macOS / Linux を再び有効化できる状態に戻す

## スコープ

### 含む

- `src-tauri/src/worktree.rs` の worktree ignore 管理方法の見直し
- `src-tauri/src/git.rs` / `src-tauri/src/worktree.rs` のマージ前チェック改善
- `src-tauri/src/claude_runner.rs` の PTY プロセス管理の trait bound 見直し
- `.github/workflows/release.yml` の matrix 再有効化
- 上記に対応するテスト追加と動作確認

### 含まない

- Prisma 自体のスキーマ設計変更
- Release 用コード署名 / notarization の導入
- Claude Runner 全面刷新

## タスクリスト

### Story 1: worktree マージの安定化

- [ ] `create_worktree` 時の `.vicara-worktrees/` 管理先を `.gitignore` から `.git/info/exclude` に移す
- [ ] 既存 repo で app が追記した `.gitignore` 差分を安全に移行・掃除する条件を定義し実装する
- [ ] `merge_worktree` 実行前に project root の dirty 状態を検知し、競合ではない失敗を事前に防ぐ
- [ ] `.gitignore` が変更される task branch を merge するケースの回帰テストを追加する
- [ ] 既存挙動を壊さないことを確認するため、worktree 作成・削除・マージ成功・競合系の既存テスト観点を補強する

### Story 2: Release ワークフローの macOS / Linux 再有効化

- [ ] `AgentSession.killer` の `Sync` 要件が本当に必要かを再確認し、不要であれば trait object 制約を整理する
- [ ] `PtyChildKiller` が `portable-pty` の `MasterPty` / `SlavePty` 非 `Sync` 制約に引っかからない構造へ修正する
- [ ] Windows 実装への影響を避けつつ、Unix 実装の待機・kill・cleanup パスを再確認する
- [ ] `.github/workflows/release.yml` の macOS / Linux matrix を再有効化する
- [ ] ワークフロー内コメントを、再有効化後の運用メモへ更新する
- [ ] Ubuntu 依存パッケージ、macOS target 指定、Windows 既存挙動が成立することを確認する

## 完了条件

- [ ] `.gitignore` を触る task branch をマージしても、app 起因のローカル変更でマージが止まらない
- [ ] project root に未コミット変更がある場合、マージ前に理由が明確なエラーが返る
- [ ] macOS / Linux を含む Release ワークフローが再び定義されている
- [ ] `claude_runner.rs` の Unix ビルドが `portable-pty` の `Sync` 制約で失敗しない
- [ ] 必要な自動テストと手動確認手順が文書化されている
