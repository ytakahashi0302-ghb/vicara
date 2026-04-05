# Epic 23 タスクリスト

- [ ] `docs/23_tech_debt_resolution/implementation_plan.md` と `task.md` の作成（作業完了・PO確認待ち）

## 1. ハードコード系の解消
- [x] `src-tauri/src/rig_provider.rs` または `src-tauri/src/ai.rs` にAPIからモデル一覧を取得する `get_available_models` コマンドを実装する
- [x] `src-tauri/src/rig_provider.rs` を改修し、AIモデル名をストアから取得するよう変更
- [x] `src-tauri/src/main.rs` に新コマンドを登録する
- [x] `src/context/WorkspaceContext.tsx` を改修し、`currentProjectId` の初期化とフォールバック処理を実装
- [x] `src/context/WorkspaceContext.tsx` にプロジェクト削除用メソッド (`deleteProject`) を追加し、Tauri コマンドと連携させる

## 2. 揮発性の解消 (Inception Deck)
- [x] `src/components/project/InceptionDeck.tsx` に `@tauri-apps/plugin-store` の読み書き処理を追加
- [x] プロジェクト切り替え時・リロード時に、プロジェクト毎に保存されたチャット履歴と状態フェーズ (`currentPhase`) が復元されるようにする

## 3. UIのクリーンアップと設定の統合
- [x] `src/components/ui/GlobalSettingsModal.tsx` を新規作成し、タブやセクションでプロジェクト削除とAI設定（モデル選択含む）を配置する
- [ ] `src/components/kanban/StorySwimlane.tsx` から「AIで自動生成」ボタン関連のコードを削除
- [ ] `src/components/kanban/BacklogView.tsx` から「アイデア」ボタン関連のコードを削除
- [ ] `src/components/ai/IdeaRefinementDrawer.tsx` の削除
- [ ] `src/App.tsx` の Inception Deck ヘッダーを Kanban 側と共通のナビゲーション（設定アイコン等）を持つように統合・整理

## 4. 手動テスト・仕上げ
- [ ] ターミナルから `npm run tauri dev` でビルドが成功するか確認
- [ ] 自動選択フォールバック、AIモデル設定の適用、チャット履歴保持、ボタン群消滅を確認
- [ ] エラーが出ない（とくにAI呼び出し時のモデル名取得等）ことを目視確認
- [ ] `walkthrough.md` を作成しPOへ報告
