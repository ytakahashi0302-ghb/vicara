# AIタスク自動生成機能 実装の確認 (Walkthrough)

# AIタスク自動生成機能 実装の確認 (Walkthrough)

## 完了した変更 (Changes made)
本フェーズでは、Storyの要件からAIを利用して具体的な実装タスク(To Do)を自動分解し、カンバンボードに追加する機能を実装しました。

### 1. APIキーのセキュアなローカル保存
- [NEW] `src/components/SettingsModal.tsx` を作成し、POがAnthropic / Gemini のAPI Keyを入力・保存できる設定画面を追加。
- デフォルトのAIプロバイダ（Anthropic または Gemini）の選択UIを実装し、状態を永続化。
- [MODIFY] `Board.tsx` のボードヘッダーに歯車アイコンの「Settings」ボタンを配置。
- Tauri v2の `@tauri-apps/plugin-store` を用いて、`settings.json` にキー環境と設定を永続化する仕組みを構築しました。

### 2. Rust側での堅牢なAPI呼び出し (Tauri Command)
- [NEW] `src-tauri/src/ai.rs` を新規作成し、`generate_tasks_from_story` コマンドを実装。
- **マルチプロバイダ対応**: フロントエンドから渡されたプロバイダ名 (`anthropic` | `gemini`) をもとにAPIエンドポイントとリクエストJSON（ペイロード構造）を動的に切り替えるロジックを実装。
- `tauri-plugin-store` 経由でRust側から指定プロバイダのAPIキーのみを安全に読み出し、フロントエンドに露出させずに `reqwest` を使ってリクエストを送信。
- **堅牢なJSON抽出**: ClaudeやGeminiが返すレスポンス（Markdownブロックや会話テキストの混入がモデルにより異なる）から、純粋なJSON配列 `[ ... ]` のみを `regex` クレートを用いて抽出して `serde_json` でパースする安全な共通設計を組み込みました。
- `src-tauri/src/lib.rs` にてプラグインの初期化とコマンドの登録を行いました。

### 3. カンバンUIとの統合
- [MODIFY] `StorySwimlane.tsx` のヘッダー部分に、紫色の「✨ AI Generate」ボタンを追加。
- ボタン押下時にStoreから `default-ai-provider` を読み込み、対象プロバイダをAPIコマンドに紐づけて実行。
- 実行中のローディング表示 (`isGenerating` state と `Loader2` アイコン) を実装。
- API呼び出しに失敗した場合のインラインエラー表示とアラート通知によるエラーハンドリング。
- 生成されたタスク配列に対して `addTask` (`useScrum`フック) をループ実行し、SQLiteへの保存とUIの即時リアクティブ更新（「To Do」列への追加）を実現しました。

## 確認事項 (Validation results)
- フロントエンドコンポーネント (TypeScript / React) のLint・ビルドテストを通過。
- バックエンド (Rust) の `cargo add` コマンドおよび初期実装完了。

> [!NOTE]
> - 本アプリはTauri(デスクトップアプリ)として動作するため、API呼び出しやローカルStoreの検証はビルドされたアプリ上（または `npm run tauri dev` 上）で行う必要があります。
> - UI上で設定画面を開き、APIキーが問題なく保存・復元されるか、AI GenerateボタンからJSON生成・タスク追加が機能するかをお手元で確認ください。
