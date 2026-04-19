# Epic56: スプリント中のルートディレクトリ動作確認ボタン

## 実装内容
- スプリントボードのヘッダーに「動作確認」ボタンを追加し、アクティブスプリント中に現在の project root を対象とした動作確認を起動できるようにした。
- `TaskCard` に埋め込まれていた preview 判定ロジックを `src/components/kanban/projectPreview.ts` に切り出し、root preview と worktree preview の双方で再利用できるようにした。
- Tauri バックエンドに `start_project_root_preview` と `open_project_root_static_preview` を追加し、project root から `npm run dev` 系の起動、または `index.html` の直接オープンをサポートした。
- 起動中の root preview を取得・停止するコマンドを追加し、UI から再表示と停止を行えるようにした。
- task のマージ成功時には backend 側で該当 project の root preview を自動停止し、frontend 側もイベントで停止済み状態へ同期するようにした。

## 変更ポイント
- [Board.tsx](/C:/Users/green/Documents/workspaces/vicara/src/components/kanban/Board.tsx)
  - ヘッダー右側に root preview ボタンを追加
  - project root の preview preset を読み込み
  - 起動中 preview の状態表示、再表示、停止ボタンを追加
  - マージ成功時の停止通知イベントを受けて UI 状態を同期
  - アクティブスプリントが空でもヘッダーとボタンが表示されるように調整
- [projectPreview.ts](/C:/Users/green/Documents/workspaces/vicara/src/components/kanban/projectPreview.ts)
  - `ARCHITECTURE.md` / `package.json` / `index.html` を使う preview 判定ロジックを共通化
- [TaskCard.tsx](/C:/Users/green/Documents/workspaces/vicara/src/components/kanban/TaskCard.tsx)
  - 共通 preview 判定 utility を利用するように変更
  - マージ成功時に root preview invalidation イベントを dispatch
- [worktree.rs](/C:/Users/green/Documents/workspaces/vicara/src-tauri/src/worktree.rs)
  - project root preview の起動・取得・停止・静的オープン用 Tauri コマンドを追加
  - task マージ成功時に project root preview を自動停止
- [lib.rs](/C:/Users/green/Documents/workspaces/vicara/src-tauri/src/lib.rs)
  - 新コマンドを invoke handler に登録

## 確認結果
- `npm run build`: 成功
- `cargo test --manifest-path src-tauri/Cargo.toml`: 成功
- 2026-04-19: ユーザーより「動作確認できました」と受領

## 補足
- root preview は worktree preview のような DB 永続状態は持たず、アプリ稼働中の preview state を使って再表示する構成。
- 対応可否は既存と同様に `ARCHITECTURE.md` ベースの簡易判定に依存するため、未知の構成ではボタン押下時に案内エラーとなる。
