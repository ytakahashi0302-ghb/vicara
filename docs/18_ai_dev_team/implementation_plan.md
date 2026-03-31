# Epic 5: AI開発チーム (AI Team Leader) 実装計画

Epic 6（スクラム基盤の再構築）の完了を受け、一時凍結していた「AI開発チーム」機能の開発を再開します。
タスクごとの横並びAIチャット（マイクロマネジメント型）を廃止し、プロジェクト全体（バックログとアクティブスプリント）を俯瞰する「AIチームリーダー」との対話インターフェースとして再構築します。

## 背景と目標
- **課題**: タスクごとに個別にAIと対話する従来のUXは、操作が煩雑になり、AIがプロジェクト全体の文脈を理解しにくいという問題がありました。
- **解決策**: スクラムマスター兼リードエンジニアとして振る舞う「AIチームリーダー」を配置する。
- **Epic 6の影響の統合**: Epic 6でバックログ（`sprint_id IS NULL`）とアクティブスプリント（`status = 'Active'`）が明確に分離されました。AIリーダーが的確なアドバイスをするためには、RAGコンテキスト生成時（`build_project_context`）において、これらを明確に区別してプロンプトに注入する必要があります。

## UIビジョン変更: ドロワー → 右側固定サイドパネル

> [!IMPORTANT]
> **POからの最終UIビジョンに基づく設計変更**
> 当初の計画では「画面右側からスライドインするドロワー（TeamChatDrawer）」を想定していましたが、POの判断により「**画面右側に常時固定表示されるサイドパネル（Right Sidebar）**」へとUI設計を変更します。
> 
> ### 目指すレイアウトイメージ
> ```
> ┌─────────────────────────────────────────────────────────────┐
> │  Header: [MicroScrum AI] [ProjectSelector] [Settings] [...] │
> │  [SprintTimer]                                              │
> ├──────────────────────────────────┬──────────────────────────┤
> │                                  │                          │
> │  メインコンテンツ領域              │  AI Team Leader          │
> │  (BacklogView / BoardView)       │  Right Sidebar           │
> │                                  │                          │
> │  ┌────────────────────────────┐  │  ┌──────────────────┐   │
> │  │  プロダクトバックログ /     │  │  │ 💬 チャット履歴   │   │
> │  │  アクティブスプリント       │  │  │                  │   │
> │  │  (タブ切替)                │  │  │  AI: 現在の...    │   │
> │  │                            │  │  │  You: ...         │   │
> │  │  [ストーリーカード群]       │  │  │  AI: ...          │   │
> │  │                            │  │  │                  │   │
> │  │                            │  │  ├──────────────────┤   │
> │  │                            │  │  │ [入力欄] [送信]   │   │
> │  └────────────────────────────┘  │  └──────────────────┘   │
> │                                  │                          │
> ├──────────────────────────────────┴──────────────────────────┤
> │  (将来的にターミナルが入る余地)                               │
> └─────────────────────────────────────────────────────────────┘
> ```
>
> **設計意図**: 左〜中央にカンバンボード（またはバックログ）、右側にAIアシスタントが常に並走する「AIネイティブIDE」のようなレイアウトを目指します。サイドパネルはトグルボタンで開閉可能とし、閉じた状態ではメインコンテンツが100%幅で表示されます。

## User Review Required

> [!IMPORTANT]
> **マイグレーション方針の確認**
> 前回の初期実装で生成された不要なテーブル `task_messages` を削除し、新たにプロジェクト全体のチャット履歴を保存する `team_chat_messages` を追加します。
> 既存のSQLite上のデータを安全に移行・整備するため、追記型のマイグレーションファイル `8_ai_team_leader.sql` を作成して対応します。
> ※ `7_scrum_foundation.sql` が既に存在するため、番号を `8` に変更しました。

> [!WARNING]
> **レイアウト構造の大幅変更**
> 右側固定サイドパネルの導入に伴い、`App.tsx` の `<main>` 要素のレイアウト構造を `flex` ベースに変更する必要があります。
> `ScrumDashboard` コンポーネントと `TeamLeaderSidebar` コンポーネントが横並びに配置される構成となるため、既存の高さ計算（`lg:h-[calc(100vh-120px)]`）やスクロール挙動に影響が出る可能性があります。

## 提案する変更内容

### 1. DB拡張とバックエンド改修 (Phase 1)

#### [NEW] `src-tauri/migrations/8_ai_team_leader.sql`
- `task_messages` テーブルを `DROP TABLE IF EXISTS` で削除。
- `team_chat_messages` テーブルを作成 (`id`, `project_id`, `role`, `content`, `created_at`)。

#### [MODIFY] `src-tauri/src/db.rs`
- `TaskMessage` 構造体を `TeamChatMessage` にリネーム（`task_id` → `project_id`）。
- `team_chat_messages` に対するCRUD（`get_team_chat_messages`, `add_team_chat_message`, `clear_team_chat_messages`）を追加。
- 古い `task_messages` 用のRust関数 (`get_task_messages`, `add_task_message`, `clear_task_messages`) を全削除。

#### [MODIFY] `src-tauri/src/lib.rs`
- 古い `task_messages` 用コマンドの登録を削除し、新しい `team_chat_messages` 用コマンドを登録。

#### [MODIFY] `src-tauri/src/ai.rs` — RAGコンテキストの高度化
- `chat_with_task_ai` を削除し、新たに `chat_with_team_leader` を作成。
- **`build_project_context` の改修**: 現在のフラットなストーリー・タスクリストを構造化してプロンプトに注入。バックログ、アクティブスプリント、計画中スプリントを明確に分離。

---

### 2. UIロールバックと右側固定サイドパネル実装 (Phase 2)

#### [MODIFY] `src/components/board/TaskFormModal.tsx`
- 以前の2カラム表示を削除し、シンプルなフォームへロールバック。

#### [DELETE] `src/components/board/TaskChatPane.tsx`
- 不要になったタスク個別チャットコンポーネントを削除。

#### [NEW] `src/components/ai/TeamLeaderSidebar.tsx`
- 右側固定サイドパネル（トグル開閉対応、Markdown対応）の実装。

#### [MODIFY] `src/App.tsx` — レイアウト構造変更
- `<main>` 要素を `flex` に変更し、サイドパネルを横並びに配置。ヘッダーに開閉ボタンを追加。

---

### 3. テストと検証 (Phase 3)

#### 手動テスト（POによるマニュアル検証）
1. DBマイグレーションの成功確認。
2. 画面右側にサイドパネルが表示されることの確認。
3. AIリーダーが Epic 6 の新しいデータ構造を正しく理解して回答できるかの確認。

## Open Questions
- **サイドパネルの初期表示状態**: POの判断により「閉じた状態 (Closed)」をデフォルトとする。
- **サイドパネルの幅**: POの判断により「固定幅 (380px)」とする。

---

## 実装履歴と進捗状況 (Status Report)

### 2026/03/30 - Phase 1 & 2 完了
- **DBマイグレーション**: `8_ai_team_leader.sql` を実行し、`team_chat_messages` への移行を完了。
- **UI刷新**: サイドパネルの実装、`App.tsx` の `flex` レイアウト化を完了。
- **通信不具合の修正**: メッセージ送信時の空ペイロード、Anthropic API のモデル 404 エラー、JSON 解析不具合（正規表現抽出）を解決。
- **対話の開通**: AI チームリーダーとの多往復の対話が安定して動作することを確認。

---

## 追加計画 - Phase 3: AIによるタスク分解と自動登録の統合 (進行中)

### 目的
AI チームリーダーとの対話によって提案されたタスク分解案を、ユーザーの承認を経てデータベースへ一括登録できるようにします。

### 提案する変更内容
#### [MODIFY] `src-tauri/src/ai.rs`
- `ChatTaskResponse` に `suggested_tasks: Option<Vec<GeneratedTask>>` フィールドを追加。
- 分解依頼に対し、説明文と共に構造化されたタスクリストを生成するようプロンプトを強化。

#### [MODIFY] `TeamLeaderSidebar.tsx`
- AI からタスク案が返ってきた場合、チャット内に「提案タスクのプレビュー」を表示。
- 「これらのタスクを登録する」ボタンを実装し、SQLite へ一括保存するアクションをトリガーする。

#### [NEW] `bulk_add_tasks` コマンド
- 提案されたタスク群を、指定のストーリー ID に紐づけて一括 INSERT する Rust コマンド。
