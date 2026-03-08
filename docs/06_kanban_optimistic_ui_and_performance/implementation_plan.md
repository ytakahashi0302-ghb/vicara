# 実装計画: カンバンボードの楽観的UIとパフォーマンス最適化

## 概要
- ドラッグ＆ドロップ時のラグを解消するため、SQLiteへの保存処理を待たずに画面のステータスを即時更新する「楽観的UI (Optimistic UI)」を導入します。
- 保存失敗時には元の設定にロールバックし、エラー通知を表示する安全設計とします。
- 各種コンポーネントにおける再レンダリングを `React.memo`, `useMemo`, `useCallback` で制御し、タスク数増加時もスムーズな操作感を実現します。

## User Review Required
> [!IMPORTANT]
> - エラー通知（トースト等）について、現在のプロジェクトにはToastコンポーネントがありません。最も手軽で高品質な `react-hot-toast` 等の軽量ライブラリを `npm install react-hot-toast` で導入することを推奨しますが、自作の簡易アラートロジックに留めるべきかご指示ください。本計画では `react-hot-toast` の導入を前提として記載しています。承認・または別の指示をお願いします。

## Proposed Changes

### 1. パッケージ追加 (Optional)
#### [MODIFY] `package.json`
- `react-hot-toast` の追加（POの承認が得られた場合）

### 2. React Context & Hooks (状態管理・DB通信ロジック)
#### [MODIFY] `src/hooks/useTasks.ts`
- `updateTaskStatus` メソッドを改修。
- 引数に渡された status で先に `setTasks` を用いてフロントエンドのStateを更新する（即時反映）。
- 続いて非同期でDBの `execute` を実行し、成功時は裏側で `fetchTasks` を呼ぶ、もしくはそのままにする。
- 失敗時は元の State（変更前）にロールバックし、トースト通知でエラーを表示する。

#### [MODIFY] `src/context/ScrumContext.tsx`
- 既存の `updateTaskStatus` メソッドの型定義や動作仕様を、楽観的UIに適合するよう微調整。

### 3. Kanbanコンポーネント最適化
#### [MODIFY] `src/components/kanban/Board.tsx`
- `handleDragEnd` 内部での `updateTaskStatus` 呼び出しを、楽観的UIに合わせた処理に最適化。
- `stories.map` 内で生成している `tasks.filter` を `useMemo` によるキャッシュ化、またはコンポーネント分割によって不要な全件走査を防ぐ。

#### [MODIFY] `src/components/kanban/StorySwimlane.tsx`
- コンポーネント全体を `React.memo` でラップし、Props（story, tasks）が変更された時のみ再描画する。
- 内部の `statuses.map` 内で生成している `tasks.filter` を `useMemo` によるキャッシュ化。
- 各種ハンドラ関数を `useCallback` でラップ。

#### [MODIFY] `src/components/kanban/TaskCard.tsx`
- コンポーネント全体を `React.memo` でラップし、タスク情報が同一であれば再レンダリングをスキップする構造にする。

### 4. UIコンポーネント
#### [MODIFY] `src/App.tsx` (または `main.tsx`)
- ToastProvider (`Toaster`) の配置（`react-hot-toast` を使用する場合のみ）。

## Verification Plan
### Automated Tests
- `npm run lint` の実行による依存配列等の警告ゼロ確認。
- `npm run tauri build` によるビルド正常確認。

### Manual Verification
1. **楽観的UIの確認**: アプリを起動し、カンバンボード上でタスクを別のステータスへドラッグ＆ドロップする。ドロップ直後、画面のタスク表示が「一瞬のラグもなく」即座に新ステータスのカラムに移動すること。
2. **非同期処理の確認**: Rust側のログまたはネットワークタブ等で、UI更新の裏側でDB保存ロジックが走っていることを確認する。
3. **ロールバックの確認**: （開発用に意図的にDBの保存処理に例外を発生させ）、保存失敗時にタスクが元のカラムに戻り、エラー通知（トースト）が表示されることを確認する。
4. **パフォーマンスの確認**: タスクを複数作成してD&Dを行い、以前よりもドラッグ中・ドロップ時のカクつきがないこと（React Profiler で不要なRenderが抑えられていること）を確認する。
