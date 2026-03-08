# バックエンド LLM API連携 実装計画

## 目的
Epic 1「AI要件定義アシスタント（対話型Story昇華機能）」の最初のステップとして、Tauriコマンド `refine_idea` を実装する。フロントエンドの入力と会話履歴をもとにLLM（Claude/Gemini）へ問い合わせを行い、返答を返すインタフェースを構築する。

## User Review Required
> [!IMPORTANT]
> APIキーの扱いおよびProviderの切り替え設計についての確認：
> 要件にある「APIキーや選ばれているプロバイダー（Claude/Gemini）の動的な読み込み」について、以下の**Rust側で直接Storeから設定を読み込むアプローチ**を採用します。
> 理由は、既存の `generate_tasks_from_story` コマンドで同様のパターンが確立されており、Tauriプラグインストア（`settings.json`）にはバックエンドから直接アクセス可能であるため、IPC経由でフロント側からセキュアな情報を都度渡すよりも効率的かつ安全であるためです。
> この方針で実装のExecutionに進めてよろしいでしょうか？

## Proposed Changes

### src-tauri/src/ai.rs
- `Message` 構造体の追加
  - `role: String`（"user" または "assistant" 等）と `content: String` を持つ、これまでの会話履歴を表現する構造体を定義。
- `refine_idea` メソッドの追加
  - シグネチャ: `pub async fn refine_idea<R: Runtime>(app: AppHandle<R>, idea_seed: String, previous_context: Option<Vec<Message>>) -> Result<String, String>`
  - 処理フロー:
    1. **Storeから設定の取得**:
       - `app.store("settings.json")` を使用。
       - `default-ai-provider` キーを取得し、無ければ `"anthropic"` をデフォルトとする。
       - 選択されたプロバイダーに応じて、`anthropic-api-key` または `gemini-api-key` を取得。無ければエラーを返す。
    2. **プロンプトおよびペイロード構築**:
       - 提示された要件どおりの「システムプロンプト（POアシスタントとしての振る舞い）」を規定する。Max Tokensは300。
       - `previous_context`（存在する場合）に `idea_seed` の内容を結合させ、API用のメッセージ配列を構築する。
       - **Anthropic API:** `claude-3-5-sonnet-20241022` を指定し、`system` パラメータと `messages` 配列を組み立てる。
       - **Gemini API:** `gemini-2.5-flash` （または `1.5` であればそちら）を指定し、`systemInstruction` と `contents` 配列を組み立てる。（Geminiの会話履歴フォーマット "user" / "model" への role マッピングも行う）
    3. **API呼び出しとレスポンスハンドリング**:
       - `reqwest::Client` でAPI呼び出しを実行し、成功時はレスポンスからAIの返答テキストを抽出して返す。
       - 失敗時は `Err(format!("..."))` 形式のエラー文字列を返す。

### src-tauri/src/lib.rs
- `refine_idea` コマンドを Tauriの `invoke_handler` に追加し、フロントエンドから呼び出せるようにする。

## Verification Plan

### Automated Tests
- バックエンドのコード追加後、`src-tauri` ディレクトリにて `cargo check` および `cargo build` を実行します。
- これにより、非同期処理、型定義、Serdeのパースなどがコンパイルレベルで問題ないことを保証します。

### Manual Verification
- 実装が完了しフロントエンド呼び出しの準備が整い次第、次のステップ（11_llm_pocフロント統合タスク）で実際にAPI通信を伴うフロントからの結合確認を行います。今回のタスクスコープ内では、Tauriコマンドとしてのコンパイル通過・ビルド成功を完了条件とします。
