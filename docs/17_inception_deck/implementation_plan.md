# 実装計画: Epic 4 AIインセプションデッキ（スプリント0）

概要:
新規プロジェクト立ち上げ時にAIと対話（壁打ち）を通じて方向性をすり合わせ、最終的にローカルディレクトリに成果物（PRODUCT_CONTEXT.md, ARCHITECTURE.md, Rule.md）として出力・保存する機能。POからの要件変更を反映し、全画面分割UI（右ペインはタブUI構成）、既存資産の流用、先行したベースルールの生成・追記アプローチで実装する。

## User Review Required
- **[IMPORTANT] プロンプト制御とステートマシン**:
  LLMは文脈を継続しやすいため、System Prompt内に「ユーザーの発言から情報を抽出し、最大1〜2回の深掘り質問後に現状のまとめを提示し『これで確定として次フェーズに進んでよいですか？』と必ず意思確認を行うこと」という強いインストラクションを含めます。加えて各Phase（例：Phase 1 -> 2）の切り替えトリガーを明確に実装します。

## Proposed Changes

### Database (SQLite)
#### [MODIFY] `src-tauri/src/db.rs`
- `projects` テーブルに新カラム `local_path` (TEXT DEFAULT NULL) を追加するマイグレーションロジックを実装する。

### Backend (Rust/Tauri)
#### [MODIFY] `src-tauri/src/ai.rs` など既存機能
- 簡易RAG統合: AI呼び出し時のコンテキスト生成処理に、`project.local_path` を参照する処理を追加。パスが存在すれば `PRODUCT_CONTEXT.md`, `ARCHITECTURE.md`, `Rule.md` を読み込み、プロンプトのテキストとして結合する。
#### [NEW] `src-tauri/src/commands/project.rs` 
- `update_project_path`: UIからの選択パスをプロジェクトに保存する。同時に、ディレクトリ内の既存3ファイルの存在状況をチェックし返す。
#### [NEW] `src-tauri/src/commands/inception.rs` (新規機能群)
- `generate_base_rule`: アプリ内で保持する「ベースルール」テキストをもとに、`Rule.md` をディレクトリ内に物理生成する（パス設定時またはPhase4開始時に呼ばれる）。
- `read_inception_file`: 再利用に向けて既存ファイル（PRODUCT_CONTEXT, ARCHITECTURE, Rule）を読み込む。
- `write_inception_file`: 各フェーズ完了時にドキュメントを書き込む/追記する（特にRule.mdへの追記）。

### Frontend (React)
#### [NEW] `src/components/project/InceptionDeck.tsx` (新規フルスクリーンUI)
- アプリの全画面レイアウトを占有するページ（モーダル/ドロワーではない）。
- **左ペイン (Chat&Wizard)**: AIとのチャットUI。Phase 1〜5の状態を持ち、プロンプト・履歴と連動して会話を進行する。
- **右ペイン (Live Document / Tabs UI)**:
  - 3つのファイル（`PRODUCT_CONTEXT.md`, `ARCHITECTURE.md`, `Rule.md`）を個別のタブとして切り替えられる Markdownプレビューエリア。手動でのタブ移動を可能とする。
  - **フェーズ連動自動切り替え**: 左ペインのPhaseにフックし、対応するタブを自動でアクティブ化する。
    - Phase 1 & 2 -> `PRODUCT_CONTEXT.md` タブ
    - Phase 3 -> `ARCHITECTURE.md` タブ
    - Phase 4 -> `Rule.md` タブ
#### [MODIFY] `src/components/kanban/Header.tsx` または `ProjectSelector.tsx`
- プロジェクト設定の一環としてローカルディレクトリを選択するUI（アイコン/ボタン）を追加。`@tauri-apps/plugin-dialog` の `open` 関数を使用。

### 既存ファイルの流用とルールの先行生成フロー
- **初期チェック**: ワークスペース設定時、Rust側でディレクトリ内を走査し、既存ファイルがあればフラグをフロントに返す。
- **ファイル再利用**: 既存ファイルがあるPhaseに入った際、ファイル内容を右ペインに表示。AIは「既存のファイルが見つかりました。この内容をベースに修正を加えますか？」という初期プロンプトでチャットを開始。
- **Rule.mdの生成タイミング**: プロジェクトパス設定直後に（既存のRule.mdがなければ）Rust側からベースルールを書き出す。Phase 4に到達時、右ペインにベースルールを表示し、「追加ルールはありますか？」とヒアリングして、最後に末尾へ追記（マージ）する。

## Verification Plan

### Automated Tests/Checks
- TypeScript 静的解析、Rustコンパイルチェック。

### Manual Verification
1. 右ペインのタブUIが正しく表示され、手動でファイルを切り替えられること。
2. 左の対話フェーズが進む（Phase 1/2 -> 3 -> 4）ごとに、右のアクティブタブが自動で切り替わること。
3. ワークスペース選択後、正しくDBにパスが保存され、かつ既存ファイルの有無が判定されること。
4. 既存ファイルがあるディレクトリを選択したとき、いきなりゼロからのヒアリングではなく、該当タブにファイル内容が表示され、それをベースに差分対話が可能であること。
5. ディレクトリがない状態から`Rule.md`が先行作成され、プロジェクト固有ルールのみが末尾に正しく追記されること。
