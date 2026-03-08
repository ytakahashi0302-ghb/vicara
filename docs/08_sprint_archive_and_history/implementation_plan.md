# 第8フェーズ 実装計画: Sprint Archive & History View

## 1. 目的目標
スプリントのログ化（アーカイブ）機能と履歴閲覧ビュー（History View）を実装し、過去の成果を振り返ることができるようにする。また、完了したタスクをメインのカンバンボードから非表示にする。

## 2. DBスキーマ変更（マイグレーション）方針
TauriのSQLiteマイグレーション機能（`src-tauri/migrations/`）を利用し、新しいマイグレーションファイル `2_add_sprints.sql` を作成する。

### 変更点
1. **`sprints` テーブルの新規作成**
  スプリント履歴を保存するためのテーブルを追加する。
  ```sql
  CREATE TABLE sprints (
      id TEXT PRIMARY KEY,
      started_at DATETIME NOT NULL,
      completed_at DATETIME NOT NULL,
      duration_ms INTEGER NOT NULL
  );
  ```

2. **既存 `tasks` テーブルへのカラム追加**
  タスクがどのスプリントで完了したかを紐づけるため `sprint_id` カラムを追加し、アーカイブ状態の判定に用いる。
  ```sql
  ALTER TABLE tasks ADD COLUMN sprint_id TEXT REFERENCES sprints(id);
  ```

## 3. アプリケーションコードの実装方針

### 3.1. アーカイブされたタスクの非表示ロジック
- **判定方法**: `tasks` テーブルの `sprint_id` カラムが `NULL` であるタスクのみを「現在進行中（ボードに表示すべき）タスク」と定義する。
- **データ取得処理の修正 (`useTasks.ts`)**:
  `fetchTasks` のSQLを修正し、アーカイブ済みのタスクを除外する。
  ```sql
  SELECT * FROM tasks WHERE sprint_id IS NULL ORDER BY created_at ASC
  ```
  `fetchTasksByStoryId` も同様に `AND sprint_id IS NULL` を付与する。

### 3.2. スプリント完了（アーカイブ）処理
- カスタムフック `src/hooks/useSprintArchive.ts` (または既存フック) でアーカイブ処理を実装する。
- **UI連携**: `SprintTimer` コンポーネントでの「Complete」ボタン押下アクションにフックさせる。
- **DB処理 (実行順)**:
  1. 新しいUUIDを `sprint_id` として生成し、`sprints` テーブルにINSERT `(id, started_at, completed_at, duration_ms)`。
  2. メインボードに紐付く「DONE」ステータスのタスクを一括更新（UPDATE）し、生成した `sprint_id` をセット。
  ```sql
  UPDATE tasks SET sprint_id = $1, updated_at = CURRENT_TIMESTAMP WHERE status = 'Done' AND sprint_id IS NULL
  ```

### 3.3. 履歴閲覧ビュー (History View) の構築
- **UIコンポーネント**: `src/components/HistoryModal.tsx`（またはスクリーン遷移）を作成。
- **画面導線**: ヘッダー部分に「History」ボタン（Lucideの `History` アイコンなど利用）を配置し、クリックでモーダルやページとして表示。
- **表示内容**:
  - **スプリントのタイムライン**: 保存されたスプリント（`sprints`）を日付降順表示。
  - **完了タスクの表示**: 各スプリントのアコーディオンを展開すると、その時点でアーカイブされた（同じ `sprint_id` を持つ）完了済タスクが表示される。

## 4. テスト・検証方針 (Verification Plan)
- **手動での動作確認**:
  1. アプリを起動し、テスト用タスクを複数「DONE」列に移動させる。
  2. スプリントタイマーを開始し、「Complete」ボタンを押す。
  3. 「DONE」列にあったタスクがボード上から消え、「To Do」「In Progress」のタスクは残ることを確認する。
  4. アプリケーション再起動時にもDBが正しく反映されており、消えたタスクがボードへ復活しないことを確認する。
  5. ヘッダーの「History」ボタンから一覧を開き、完了したスプリント情報とそこでまとめられた「DONE」タスクが表示されることを確認する。

## 5. PO（ユーザー）への確認事項（User Review Required）
> [!IMPORTANT]
> 1. この仕様ではタスク（子要素）のみを「スプリント完了時にボードから非表示（アーカイブ）」する方針としています。完了タスクがアーカイブされた結果、紐づく「完了タスクが0個になった親ストーリー（Story）」をどう扱うべきか（ストーリーも一緒に完了扱いにして隠すか、手動でDoneにするままボードに残すか）についてご希望はありますでしょうか？（現状ではストーリー自体はボードに残る想定です）
> 2. マイグレーションファイル（`2_add_sprints.sql`）の実行において、現在のDBにすでにデータがある場合も破壊せず対応可能なALTER句を利用しますが、開発中でデータ不整合が起きる場合は、PO権限でのDB再初期化（TestPanel経由など）をお願いする場合があります。よろしいでしょうか。
