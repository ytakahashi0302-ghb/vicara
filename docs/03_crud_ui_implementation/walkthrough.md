# Phase 3 CRUD UI 実装の確認

## 変更内容
1. **共通UIコンポーネントの追加**
   - `@tailwindcss/postcss` などの環境に合わせ、`clsx` と `tailwind-merge` を用いた汎用コンポーネントを作成 (`Modal`, `Input`, `Textarea`, `Button`)。
   - アイコン用に `lucide-react` を導入し、各フォームのボタン等に活用。
2. **ストーリー作成・編集・削除機能の実装**
   - `StoryFormModal.tsx` を実装。Viewとしての役割に徹し、Logic（`useScrum` の呼び出し等）は持たせない設計を採用。
   - `Board.tsx` (Sprint Board 全体) 上部に「Add Story」ボタンを追加し、モーダルを呼び出して新規Storyを作成できるよう実装。
   - `StorySwimlane.tsx` (各Storyのヘッダー部) に編集用三点リーダーボタンを追加し、モーダルを呼び出してStoryの編集および削除機能を実装。
3. **タスク作成・編集・削除機能の実装**
   - `TaskFormModal.tsx` を実装。（Storyと同様のDumb Component設計）
   - `StorySwimlane.tsx` ヘッダー部に各Storyに紐づく「Add Task」ボタンを追加し、タスクを新規作成できるよう実装。
   - `TaskCard.tsx` 内に編集用三点リーダーボタンを追加し、タスクの内容変更（Status変更含む）および削除機能を実装。
4. **型安全性の向上**
   - 型定義（TS Type）の `status` プロパティにおけるリテラル型の不一致エラーを解消。
   - 未使用の `React` インポート等によるLinterエラーを修正。

## テスト内容
1. `npm run build` および TypeScript（`tsc`）コンパイラによる静的解析でエラーゼロを確認。
2. （POによるマニュアル確認事項）
   - 新規Story作成時、タイトル未入力だと保存できないこと。
   - Story/Task の各コンポーネントから編集・削除が正常に動作し、変更が即座にUI（ボード）に反映されること。

## 検証結果
- 静的解析およびビルドはエラーなく通過。
- ユーザーによる操作（View）とDB処理（Context/Custom Hooks）の責任が分離され、要求された設計制約が完全に守られていることを確認した。
