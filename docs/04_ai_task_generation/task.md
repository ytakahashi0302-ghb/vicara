# 第4.5フェーズ: Gemini API 統合タスクリスト (Epic 3 拡張)

## 1. 準備・設定 (Settings & Store)
- [x] UI: `SettingsModal.tsx` にGemini API Keyの入力欄を追加
- [x] UI: `SettingsModal.tsx` にデフォルトのAIプロバイダ(Anthropic/Gemini)選択用ラジオボタン/セレクトボックスを追加
- [x] UI: Gemini API KeyとデフォルトプロバイダをStoreへ保存/取得するロジックの追加

## 2. バックエンド実装 (Rust - Tauri Command)
- [x] `generate_tasks_from_story` コマンドがフロントエンドから `provider` (プロバイダ名) を受け取るように改修
- [x] プロバイダが "gemini" の場合、Storeから `gemini-api-key` を取得する処理を追加
- [x] Gemini API (`gemini-2.5-flash`) のペイロード構造の構築ロジックを実装
- [x] Gemini API エンドポイントに対するPOSTリクエスト処理の実装
- [x] Gemini APIのレスポンスからテキストを抽出し、既存の正規表現ロジックでJSONをパースする処理を追加

## 3. フロントエンド連携 (StorySwimlane)
- [x] `StorySwimlane.tsx` にて、タスク生成前にStoreから「デフォルトプロバイダ」を読み込む
- [x] 未設定時のフォールバック処理 (`anthropic` 等) を実装
- [x] Tauriコマンド `generate_tasks_from_story` 呼び出し時に引数 `provider` を渡すよう修正

---

## 【完了済み】第4フェーズ: Anthropic実装 (アーカイブ)
<details>
<summary>完了したタスクを表示</summary>

### 1. 準備・設定 (Settings & Store)
- [x] `tauri-plugin-store`クレートとフロントエンドパッケージのインストール
- [x] `src-tauri/src/lib.rs` 等でStoreプラグインの初期化設定
- [x] UI: `SettingsModal.tsx` の作成 (APIキー入力・保存用フォーム)
- [x] UI: ヘッダーまたはサイドバーにSettingsを開くボタン追加
- [x] フロントエンドからStoreへAPIキーを保存/取得するロジックの実装

### 2. バックエンド実装 (Rust - Tauri Command)
- [x] `reqwest` および関連クレート (`serde_json`等) のインストール
- [x] Rust側に `generate_tasks_from_story` Tauriコマンドを実装
  - [x] StoreからAPIキーを取得
  - [x] Anthropic APIへのリクエスト組み立て (システムプロンプトによるJSON出力の強制)
  - [x] API呼び出しおよびエラーハンドリング
  - [x] レスポンスのJSON文字列の抽出・バリデーション

### 3. フロントエンド実装 (連携とUI)
- [x] `StorySwimlane.tsx` に「AIタスク生成（✨）」ボタンを追加
- [x] ボタン押下時のローディング状態管理 (`isGenerating` state)の実装
- [x] Tauriコマンド (`generate_tasks_from_story`) の呼び出しロジック実装
- [x] 取得したタスク配列(JSON)を `useScrum` を経由してSQLiteに一括INSERTする処理の実装
- [x] 保存完了後、UI（ローカルのTask状態）を即座に更新し、ボード上に反映させる
</details>
