# Epic 23: 技術的負債の解消とUIクリーンアップ

本Epicでは、これまでの開発で蓄積されたUIの不要なコンポーネントのクリーンアップ、ハードコードの解消、およびUX向上（永続化やアクセシビリティの改善）を一気に行います。

## User Review Required

> [!IMPORTANT]
> - **プロジェクト削除UIの配置場所**: 現在の `ProjectSelector`（ドロップダウン等）の横に設定アイコン（⚙️）を配置し、そこから開く「プロジェクト設定 / グローバル設定モーダル」内にプロジェクト削除機能とAIモデル設定機能を統合しようと考えていますが、このアプローチでよろしいでしょうか？
> - **Inception Deckの永続化先**: Tauriの `@tauri-apps/plugin-store` (`settings.json`) を用いて、`inception-chat-${projectId}` のようなキーでチャット履歴と現在のフェーズを保存・復元する方針です。これによりアプリ再起動時も履歴が保持されます。
> - **AIモデル一覧の動的取得**: APIキーを用いてAnthropicおよびGeminiのモデル一覧取得APIを叩き、利用可能なモデル名を動的に取得してドロップダウンに表示します。エラー時や未設定時のためにカスタムテキスト入力フォールバックも設けます。

## Proposed Changes

### 1. ハードコード系の解消

#### [NEW] APIからのモデル動的取得機能
- Tauri側 (`src-tauri/src/ai.rs` または `rig_provider.rs`) に、プロバイダーのAPI（Anthropic / Gemini）を直接叩いて利用可能なモデル一覧（文字列配列）を取得するコマンド `get_available_models` を新規実装します。

#### [MODIFY] src-tauri/src/rig_provider.rs
- `chat_anthropic`, `chat_gemini`, `chat_team_leader_with_tools` 内でハードコードされているAIモデル名（`claude-haiku-4-5-20251001`, `gemini-2.0-flash`）を、Tauri Store から取得する動的モデル名 (`anthropic-model`, `gemini-model`) を使うように改修します。
- ストアに値がない場合のデフォルト値として既存のモデル名をフォールバック指定します。

#### [MODIFY] src/context/WorkspaceContext.tsx
- `currentProjectId` の初期化を `'default'` の固定値ではなく、`fetchProjects()` 完了時点で 現在のIDが存在しない場合は `projects[0].id` を自動選択するようにフォールバック処理を実装します。
- バックエンドの `delete_project` を呼び出し、現在のプロジェクト状態をクリーンアップする `deleteProject` 関数を追加します。

---

### 2. 揮発性の解消 (Inception Deck)

#### [MODIFY] src/components/project/InceptionDeck.tsx
- `@tauri-apps/plugin-store` を利用し、`messages` および `currentPhase` の状態をプロジェクトごとに永続化・復元する処理を追加します。
- コンポーネントマウント時（または `currentProjectId` 変更時）に Store から履歴を読み込み、メッセージ追加やフェーズ変更のたびに Store に保存します。

---

### 3. UI / UXのクリーンアップとアクセシビリティ改善

#### [DELETE] / [MODIFY] 旧AI連携ボタンの削除
- **src/components/kanban/StorySwimlane.tsx**: 「AIで自動生成」ボタンと `handleGenerateTasks` の完全削除。
- **src/components/kanban/BacklogView.tsx**: 「アイデア」ボタンの削除。
- **src/components/ai/IdeaRefinementDrawer.tsx**: 不要となるためコンポーネント自体を削除。

#### [NEW] src/components/ui/GlobalSettingsModal.tsx (新規作成)
- 新しい設定用のモーダルを作成し、以下の機能を提供します。
  - AIモデル設定
    - ドロップダウンでプロバイダを選択
    - 選択したプロバイダの `get_available_models` コマンドを呼び出し、モデル一覧を動的に取得してリスト表示
    - エラー時用の手動入力（カスタムテキスト入力欄）のフォールバック
  - 現在選択中のプロジェクトの「削除」機能と確認ダイアログ（Interaction Guardは既存の仕組みと調整）

#### [MODIFY] src/App.tsx & ヘッダーコンポーネント
- Inception Deck と Kanban のヘッダーを共通化・整理し、どこからでもアクセスできる「設定（Settings）」アイコンを配置して `GlobalSettingsModal` を開けるようにします。
- Inception Deck への導線も分かりやすく配置します。

## Open Questions

- 全てのオープンクエスチョンに対する回答は受理し、反映済みです。

## Verification Plan

### Automated Tests
- 本プロジェクトでは自動テスト（ユニットテスト等）は最小限であり、目視での挙動確認をメインとします。Rust側のTauriコマンドに変更を入れるため動作コンパイルの成功を担保します。

### Manual Verification
1. **AIモデル設定**: 設定モーダルから適当なモデル名に変更し、AI呼び出し時にエラー（または正しいモデルで応答）になることを確認する。
2. **プロジェクトフォールバック**: 存在しないプロジェクトを指定している場合、一覧の先頭のプロジェクトが選択されること。
3. **プロジェクト削除**: 設定画面から削除を実行後、ディレクトリが消え、別のプロジェクトが自動選択されること。
4. **Inception Deck**: メッセージを送信後、別画面やアプリ再起動をしてもチャット内容が復元されること。
5. **ボタン削除**: カンバンやバックログの当該ボタンが消滅していること。
