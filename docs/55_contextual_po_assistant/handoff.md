# EPIC 55: コンテキスト連動型 POアシスタント — 引き継ぎメモ

## 1. 現在の到達点
- Epic 55 の主要実装は完了し、`main` へ push 済み。
- カンバン画面に加え、プロダクトバックログ画面からも PBI / Task を起点に PO アシスタントへ focus 付き相談を開始できる。
- Task focus では `## 提案` 形式の提案を返させ、`SuggestionReviewModal` と既存編集モーダル経由でのみ反映する。
- Story focus では prompt と parser の二段構えで `## 提案` を禁止し、テキスト助言のみを返す。
- AI による直接 DB 更新は禁止しており、focus 付き相談は backend 側で非 mutation モードに切り替えている。

## 2. 実装済みの主要項目
- `PoAssistantFocusContext` / `poAssistantFocusState` による focus 状態管理
- `TaskCard`, `StorySwimlane`, `BacklogView`, `TaskFormModal`, `StoryFormModal` からの相談導線
- `PoAssistantSidebar` の focus チップ、境界システムメッセージ、新しい会話リセット
- `team_leader.rs` / `prompts.rs` の focus 注入と非 mutation ガード
- Forgiving parser (`suggestionParser.ts`)
- `SuggestionReviewModal` による差分確認
- parser 単体テスト、focus state 単体テスト、Rust prompt snapshot テスト

## 3. 検証状況
- 完了:
  - `npm run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `node tests/suggestionParser.test.mjs`
  - `node tests/poAssistantFocusState.test.mjs`
- ユーザー実機確認済み:
  - バックログ画面からの相談導線
- 未完了:
  - Task カード → 相談 → 提案生成 → 差分モーダル → 編集モーダル → 保存 の手動 E2E
  - In Progress タスクで反映ボタンが disabled になることの手動 E2E

## 4. 既知の残課題
- `npm run lint` は未グリーン。
- Epic 55 の変更起因ではなく、既存の以下ファイルに `react-hooks/set-state-in-effect` error が残っている。
  - `src/components/ai/Avatar.tsx`
  - `src/components/ui/TeamSettingsTab.tsx`
  - `src/context/WorkspaceContext.tsx`

## 5. 次に着手するなら
1. 手動 E2E 2 件を実施して `task.md` の未チェック項目を消す。
2. 必要なら結果を `walkthrough.md` に追記して、完全クローズ版に更新する。
3. Epic 55 と無関係の既存 lint error を別タスクとして切り出して解消する。

## 6. 参照ドキュメント
- `docs/55_contextual_po_assistant/implementation_plan.md`
- `docs/55_contextual_po_assistant/task.md`
- `docs/55_contextual_po_assistant/walkthrough.md`
- `CHANGELOG.md`

## 7. 参照実装
- `src/components/ai/PoAssistantSidebar.tsx`
- `src/components/ai/SuggestionReviewModal.tsx`
- `src/components/ai/suggestionParser.ts`
- `src/components/kanban/BacklogView.tsx`
- `src/components/kanban/StorySwimlane.tsx`
- `src/components/kanban/TaskCard.tsx`
- `src/context/PoAssistantFocusContext.tsx`
- `src/context/poAssistantFocusState.ts`
- `src-tauri/src/ai/team_leader.rs`
- `src-tauri/src/ai/team_leader/prompts.rs`
