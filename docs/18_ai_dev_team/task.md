# Task List: Epic 5 AI開発チーム (Pivot: AI Team Leader — Right Sidebar)

Epic 6にてプロダクトバックログとアクティブスプリントの分離基盤が完成したため、新しいアーキテクチャに準拠したAIリーダー機能の再開・実装を行います。
UIは「開閉式ドロワー」から「右側固定サイドパネル（Right Sidebar）」へ変更されています。

## Phase 1: DB再設計とBackendの更新
- [x] 1.1 `src-tauri/migrations/8_ai_team_leader.sql` の作成
  - [x] `task_messages` テーブルの `DROP TABLE IF EXISTS` 処理。
  - [x] `team_chat_messages` テーブルの `CREATE TABLE` 処理（`id`, `project_id`, `role`, `content`, `created_at`）。
- [x] 1.2 CRUD関数のリファクタリング（`db.rs`, `lib.rs`）
  - [x] `TaskMessage` 構造体を `TeamChatMessage` にリネーム（`task_id` → `project_id`）。
  - [x] 不要になった旧タスクチャット用の関数を削除 (`get_task_messages`, `add_task_message`, `clear_task_messages`)。
  - [x] プロジェクト共有のチャット用関数 `add_team_chat_message`, `get_team_chat_messages`, `clear_team_chat_messages` の実装。
  - [x] `lib.rs` のコマンド登録を更新（旧関数の登録削除 → 新関数の登録追加）。
- [x] 1.3 `ai.rs` のリファクタリング
  - [x] `chat_with_task_ai` を削除。
  - [x] 新コマンド `chat_with_team_leader` を実装し、システムプロンプトにスクラムマスター兼リードエンジニアのペルソナを設定。
- [x] 1.4 `build_project_context` の高度化（RAGコンテキストの構造化）
  - [x] ストーリーとタスクを「プロダクトバックログ (`sprint_id IS NULL`)」「アクティブスプリント (`sprint.status = 'Active'`)」「計画中スプリント (`sprint.status = 'Planned'`)」に分類して文字列化する処理に変更。
  - [x] アクティブスプリント内のタスクには完了マーク（✅/🔄）を付与し、AIが進捗状態を即座に把握できるようにする。
- [x] 1.5 Frontend用型定義の更新 (`src/types/index.ts` 等)
  - [x] `TeamChatMessage` 型の追加。
  - [x] 古い `TaskMessage` 型の削除。

## Phase 2: UIロールバックと右側固定サイドパネル実装
- [x] 2.1 古いUIのロールバック
  - [x] `TaskFormModal.tsx` を元のシンプルな1ペインレイアウトへ戻す。
  - [x] `TaskChatPane.tsx` ファイルの完全削除。
- [x] 2.2 `TeamLeaderSidebar` コンポーネントの新規作成
  - [x] 画面右側に固定表示されるサイドパネルUIを実装（`src/components/ai/TeamLeaderSidebar.tsx`）。
  - [x] チャットヘッダー（タイトル + 閉じるボタン）の実装。
  - [x] Markdown対応のメッセージリスト（スクロール可能）およびローディング表示の実装。
  - [x] テキスト入力エリア + 送信ボタン（`Ctrl+Enter` / `Cmd+Enter` 対応）の実装。
  - [x] チャット履歴クリアボタンの実装。
- [x] 2.3 アプリケーションレイアウトの変更 (`App.tsx`)
  - [x] `<main>` 要素を `flex` レイアウトに変更し、`ScrumDashboard`（左〜中央）と `TeamLeaderSidebar`（右）を横並びに配置。
  - [x] サイドパネルの開閉に応じてメインコンテンツの幅が動的に変化する構成を実装。
  - [x] ヘッダーにサイドパネル開閉トグルボタン（Botアイコン, lucide-react利用）を追加。

## Phase 3: テストと検証
- [ ] 3.1 サイドパネルUIの開閉・レイアウト確認
  - [ ] サイドパネルの開閉がスムーズに動作し、メインコンテンツの幅が適切に追従するか確認。
  - [ ] BacklogView / BoardView 両タブでサイドパネルが干渉なく利用可能か確認。
  - [ ] タイマー稼働中などの他要素との干渉がないか確認。
- [ ] 3.2 AI回答の精度確認（RAGコンテキスト検証）
  - [ ] バックログ・アクティブスプリント・計画中スプリントの三区分がAIコンテキストとして正しく渡っているか、対話テストを実行して確認。
- [ ] 3.3 ウォークスルーの作成
  - [x] 手動検証用に `walkthrough.md` を更新し、POに完了報告を行う。
