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
