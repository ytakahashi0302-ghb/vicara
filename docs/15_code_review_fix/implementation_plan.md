# Implementation Plan: Epic 2.5 コードベースの堅牢化

## ゴールの説明
CODE_REVIEW.md にて指摘されたCriticalバグおよび将来のアーキテクチャ破綻リスク（技術的負債）を解消します。これにより、データ不整合や意図しないアーカイブを防ぎ、今後の機能追加（特にAIスプリント機能など）に耐えうる堅牢なコードベースを構築します。

## ユーザーレビューが必要な項目
特にありませんが、データベースのマイグレーションによって既存のテーブル構成が変更されます（`archived`カラムの追加）。

## 提案される変更点

### データベース・バックエンド (Rust)
#### [NEW] `src-tauri/migrations/2026-03-09_add_archived_column.sql` (日付は仮)
- `stories` テーブルと `tasks` テーブルに対して、`archived INTEGER DEFAULT 0` (SQLiteのBOOLEAN表現) を追加します。

#### [MODIFY] `src-tauri/src/db.rs`
- 初期化関数 (`init_db` や `establish_connection` など) にて、`PRAGMA foreign_keys = ON;` を実行します。
- 各種 SELECT / UPDATE 条件において、「アクティブ・バックログ」判定を `sprint_id IS NULL` ではなく `archived = 0` (FALSE) に変更します。
- `archive_sprint` 関数を `BEGIN TRANSACTION` と `COMMIT` / `ROLLBACK` で囲み、トランザクション化します。
- `archive_sprint` 関数のストーリーアーカイブ判定 SQL (`NOT EXISTS (SELECT 1 FROM tasks WHERE tasks.story_id = stories.id AND tasks.sprint_id IS NULL)`) を見直し、正しく「スプリント完了時に未完了タスクが残っていないストーリー・または完了対象として含めるべきストーリー」のみがアーカイブされるように修正します。(タスク0件のときに対する考慮など)

#### [MODIFY] `src-tauri/src/models.rs` または関連する型定義
- `Story` と `Task` の Struct に `archived: bool` を追加します。

#### [MODIFY] `src-tauri/src/ai.rs`
- APIキーとプロバイダ取得ロジックが20行ほど重複しているため、`get_api_key_and_provider(app_handle)` のようなヘルパー関数を定義し、呼び出し側をシンプルにします。

### フロントエンド (React/TypeScript)
#### [MODIFY] `src/types/index.ts` (型定義ファイル)
- `Story` と `Task` インターフェースに `archived: boolean` を追加します。

#### [MODIFY] フロントエンドのコンポーネントおよびフック群 
- `useStories`, `useTasks` リポジトリフック等で、バックログやカンバンの表示条件を `sprint_id` の有無から `!archived` (または `archived === false`) によって適切にフィルタリングするように修正します。

#### [MODIFY] `Rule.md`
- スコープ外となった各種コードレビュー指摘（AIのモデルハードコード、JSONパースの脆弱性など）を `Tech Debt` セクションに記録します。

## 検証計画 (テスト方針)

### 自動テスト / ビルド検証
- Rustバックエンドの変更語、`cargo check` および `npm run tauri dev` にてコンパイルエラーが出ないことを確認します。
- フロントエンドのTypeScript型エラーが発生しないことを確認します。

### 手動検証
- マイグレーションが正常に適用され、以前のデータが消えないことを確認します。
- Tauriアプリを手動で操作し、以下の基本CRUDが動作することを検証します：
  - プロジェクト削除時に外部キー制約により子レコードもカスケード削除の挙動になっているか（または制約エラーとなる仕様か）の簡易確認。
  - ストーリーやタスクがバックログに正しく表示されること。
  - スプリントタイマーを開始・完了し、スプリントのアーカイブ時にタスク0件のストーリーが意図せず消えたり、途中エラーで中途半端にデータが残らないこと。
  - AI機能（タスク生成やアイデア洗練）が、リファクタリング後もAPIキーを正しく読み取って動作すること。
