# Epic 3: コンテキスト・アウェアなAI要件定義（簡易RAGの導入） 実装計画

## 背景・目的
Epic 1で作成したAIアシスタント機能（壁打ち、タスク自動分解）を強化します。新たに「Epic 3: コンテキスト・アウェアなAI要件定義」として、簡易的なRAG（Retrieval-Augmented Generation）を導入します。
フロントエンドで選択中のプロジェクト (`project_id`) をRust側に渡し、当該プロジェクトの既存の Story/Task の情報をLLMへ事前にコンテキストとして提供することで、重複のない文脈に沿った提案が行えるようにします。

## プロセス
この開発はAIと人間の協働開発ルールに基づき、以下の3ステップで進めます：
1. **Planning**: 本計画のPO（プロダクトオーナー）への承認
2. **Execution**: 実装・テスト
3. **Review**: `walkthrough.md` を活用した手動検証依頼

## 変更対象ファイルと方針

### フロントエンド
- **[MODIFY] `src/components/ai/IdeaRefinementDrawer.tsx`**
  - `WorkspaceContext` (または `useWorkspace`) から `currentProjectId` を取得。
  - `invoke('refine_idea', { input, conversationHistory, projectId: currentProjectId })` のように引数に追加。
- **[MODIFY] `src/components/kanban/StorySwimlane.tsx`**
  - 同じく `WorkspaceContext` から `currentProjectId` を取得。
  - `invoke('generate_tasks_from_story', { storyPrompt, projectId: currentProjectId })` のように引数に追加。

### バックエンド（Rust）
- **[MODIFY] `src-tauri/src/ai.rs`**
  - `refine_idea` および `generate_tasks_from_story` の関数シグネチャに `project_id: i32` (または `String`/`i64` ※DBスキーマに合わせる) を追加。
  - プロンプト生成時（APIリクエスト直前）に、後述のDB取得関数を呼び出してコンテキストを読み込み、System Prompt の末尾に追記。
- **[NEW / MODIFY] `src-tauri/src/db.rs` または `src-tauri/src/ai.rs` 内のヘルパー関数**
  - `fetch_project_context(pool, project_id)` のような関数を実装。
  - SQL: `SELECT id, title, description, status FROM stories WHERE project_id = ? AND archived = 0` （タスクも同様に取得 `WHERE project_id = ? AND archived = 0`）。
  - 取得結果をフォーマットし、以下のようなMarkdown形式の文字列を生成しLLMへ渡す。
    ```markdown
    【現在のプロジェクトコンテキスト】
    ## 既存のストーリーとタスク
    - Story: ログイン機能の実装 (Status: In Progress)
      - Task: UI作成 (Status: Done)
      - Task: API連携 (Status: To Do)
    ```

## テスト方針（検証計画）
- **自動テスト・静的解析**:
  - `npm run lint` や TypeScript、Rustのビルドエラー (`npm run tauri build` 前の `cargo check`) が発生しないことを確認する。
- **手動検証**:
  1. `npm run tauri dev` でアプリケーションを起動。
  2. 既存のプロジェクトに手動でテスト用のStoryとTaskを作成する。
  3. AIアイデア壁打ち機能（IdeaRefinementDrawer）を開き、「既存のストーリーに関連した〜」「かぶらないように〜」と指示を出して提案させる。
  4. 生成された提案内容に、すでに存在するStory/Taskの文脈が含まれていること（重複がない等の条件を満たすこと）を確認する。
  5. 稼働状況を `walkthrough.md` にまとめ、POに確認を依頼する。
