# Task List: Epic 2.5 コードベースの堅牢化（技術的負債の返済）

## 優先度「高」（Criticalバグ・アーキテクチャの修正）
- [x] `src-tauri/src/db.rs`: データベース接続時に外部キー制約を有効化する (`PRAGMA foreign_keys = ON;`)。
- [x] `src-tauri/migrations`: `stories` および `tasks` テーブルに `archived BOOLEAN DEFAULT FALSE` を追加するマイグレーションファイルを作成する。
- [x] `src-tauri/src/db.rs`: バックエンドのクエリを修正し、アクティブ状態の判定を `sprint_id IS NULL` から `archived = FALSE` へと移行する。
- [x] フロントエンド型定義・取得ロジックの修正: 
    - `Story`, `Task` 型へ `archived` プロパティを追加。
    - コンポーネントおよびフック内の「バックログ/アクティブ」判定を `archived === false` に修正する。
- [x] `src-tauri/src/db.rs`: `archive_sprint` 関数をトランザクション (`BEGIN TRANSACTION` ~ `COMMIT`) でラップし、整合性を保つ。
- [x] `src-tauri/src/db.rs`: `archive_sprint` 内の「タスクが0件のストーリー」が意図せずアーカイブされるSQL条件のバグを修正する。

## 優先度「中」（リファクタリング）
- [x] `src-tauri/src/ai.rs`: `generate_tasks_from_story` と `refine_idea` 内で重複しているAPIキー取得処理をヘルパー関数に抽出する。
- [x] ※ `Rule.md` に、今回のスコープ外となった Medium / Minor 指摘（Tech Debt）を追記する。
