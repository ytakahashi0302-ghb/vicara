# Epic56: スプリント中のルートディレクトリ動作確認ボタン

## 実装計画

## 実装結果サマリ（2026-04-19）
- [x] スプリントボードから project root 向けの動作確認ボタンを起動可能にした
- [x] preview 判定ロジックを `TaskCard` / `Board` 間で共通化した
- [x] root preview の起動・取得・停止・静的オープン用 Tauri コマンドを追加した
- [x] root preview の再表示・停止導線を UI に追加した
- [x] task マージ成功時に root preview を自動停止するようにした
- [x] `npm run build` / `cargo test --manifest-path src-tauri/Cargo.toml` を通過した
- [x] PO による動作確認完了

## クローズ判断
- PO より「動作確認できました」と受領済みのため、本 Epic56 は受け入れ完了としてクローズ可能。

### 1. UI導線の追加
- `src/components/kanban/Board.tsx` のヘッダー右側に「動作確認」ボタンを追加する。
- ボタンは現在選択中プロジェクトの `local_path` を利用し、利用不可の場合は非活性またはエラートーストで案内する。

### 2. プレビュー判定ロジックの共通化
- 既存の `TaskCard` に内包されている `ARCHITECTURE.md` / `package.json` / `index.html` を用いた判定を `frontend-kanban` 配下へ切り出す。
- `TaskCard` と `Board` が同じ判定ロジックを参照することで、root preview と worktree preview の挙動差分を最小化する。

### 3. バックエンド導線の追加
- `src-tauri/src/worktree.rs` に、project root を直接対象にする preview 起動コマンドを追加する。
- 既存の `preview.rs` の起動・URL検出・ブラウザ起動処理を再利用し、worktree 専用実装を壊さない形で拡張する。
- 静的サイト向けには root の `index.html` を直接開くコマンドを追加する。

### 4. 回帰防止
- `TaskCard` 側は共通化後も既存の Review フローで同じコマンド・表示文言が維持されるようにする。
- worktree preview と root preview のキーを分離し、互いのセッションを上書きしないようにする。

## テスト方針
- フロントエンド:
  - スプリントボード表示時にボタンが正しく表示されること
  - `local_path` 未設定時に安全なエラー表示になること
  - プレビュー判定ロジック共通化後も `TaskCard` の UI が維持されること
- バックエンド:
  - root preview コマンド追加後も既存の worktree preview コマンドがビルドできること
  - 静的 `index.html` 直開きと dev サーバー起動の両ケースが成立すること
- 総合確認:
  - `npm run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml`

## 想定リスク
- `ARCHITECTURE.md` に依存した簡易判定のため、未知の構成ではボタンが利用不可になる可能性がある
- root preview は worktree preview と違って DB 永続化を持たないため、画面再生成時は都度再オープン前提になる

## 完了メモ
- 当初リスクとしていた「停止導線不足」は、UI 側の停止ボタンと task マージ成功時の自動停止で解消した。
- これにより、スプリント中の確認とマージ後の再確認の両方で、古い preview プロセスを引きずらない運用に揃えられた。
