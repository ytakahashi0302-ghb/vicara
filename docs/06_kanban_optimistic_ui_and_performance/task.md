# タスクリスト: カンバンボードの楽観的UIとパフォーマンス最適化

## STEP 1: 楽観的UI (Optimistic UI) の導入とロールバック
- [x] `react-hot-toast` のインストール (PO承認後) とアプリケーションルートへの `Toaster` 設定。
- [x] `src/hooks/useTasks.ts` の `updateTaskStatus` の改修。
    - [x] DB更新前に `setTasks` を用いてフロントエンドのStateを先行更新する。
    - [x] 非同期でDB保存 `db.execute` を実行する。
    - [x] 保存失敗時は、元のStateにロールバックするロジックを追加。
    - [x] ロールバック時、トーストまたはアラートでエラー通知を表示する。
- [/] `src/context/ScrumContext.tsx` および `src/components/kanban/Board.tsx` で、新しい更新フローに合わせて呼び出し元を調整。

## STEP 2: カンバンボード (dnd-kit) のパフォーマンスチューニング
- [x] `src/components/kanban/Board.tsx` の最適化。
    - [x] `useScrum` から取得した `tasks` のフィルタリングを `useMemo` 化し、全タスクの再計算を防ぐ。
    - [x] `handleDragEnd` などの各種関数を `useCallback` でラップし、不要な関数再生成を防ぐ。
- [x] `src/components/kanban/StorySwimlane.tsx` の最適化。
    - [x] コンポーネント全体を `React.memo` でラップ。
    - [x] 渡された `tasks` をステータスごとに分割する処理 (`tasks.filter`) を `useMemo` でキャッシュ化。
- [x] `src/components/kanban/TaskCard.tsx` の最適化。
    - [x] コンポーネント全体を `React.memo` でラップし、タスク情報が同一であれば再レンダリングをスキップする構造にする。

## STEP 3: テストと検証
- [x] `npm run lint` の実行とエラーゼロ確認。
- [x] アプリケーション起動後の手動テストにより、以下を確認。
    - [x] D&D時の状態即時反映（ラクのない移動）。
    - [x] プロファイラ等による不要な再レンダリングの削減。
    - [x] 意図的なエラー発生時のロールバックと通知表示の確認。
