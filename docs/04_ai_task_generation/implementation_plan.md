# Gemini API統合 実装計画 (追加要件)

## 目標 (Goal Description)
AnthropicのAPI利用制限への対応として、既存のAnthropic呼び出しロジックを保持したまま、新たに **Gemini API (`gemini-2.5-flash`)** を用いてタスク自動生成を行えるようにシステムを拡張します。ユーザーが任意にデフォルトのプロバイダを選択できるUIと、選択されたプロバイダに応じて処理を分岐・実行できるバックエンドを構築します。

## Proposed Changes

### 1. Settings UIの拡張 (Frontend)
#### [MODIFY] `src/components/SettingsModal.tsx`
- **Gemini API Key入力欄の追加**: 既存のAnthropic API Key入力欄の下にGemini API用の入力欄 (`gemini-api-key`) を設けます。
- **デフォルトプロバイダ選択UI**: `<select>` または Radio ボタンを用いて、タスク生成時に使用するデフォルトプロバイダ (`default-ai-provider`, 値: `anthropic` or `gemini`) を選択・保存できるようにします。

### 2. バックエンドAPI呼び出しの分岐 (Rust Tauri Command)
#### [MODIFY] `src-tauri/src/ai.rs`
- **引数の追加**: `generate_tasks_from_story` コマンドの引数に `provider: String` を追加します。
- **分岐処理**: 
  - `provider == "anthropic"` の場合は、既存のAPI呼び出しロジックを実行。
  - `provider == "gemini"` の場合は、Storeから `gemini-api-key` を取得し、以下の仕様でGemini APIを叩く新しいロジックを実行。
- **Geminiペイロード構造**:
  - **URL**: `https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={API_KEY}`
  - **メソッド**: `POST`
  - **Body (JSON)**:
    ```json
    {
      "systemInstruction": {
        "parts": [{ "text": "You are an expert Agile Scrum Master and Developer. Break down stories into practical, technical, actionable tasks." }]
      },
      "contents": [
        {
          "parts": [{ "text": <構築したプロンプト文字列> }]
        }
      ],
      "generationConfig": {
        "responseMimeType": "application/json"
      }
    }
    ```
- **Gemini用レスポンスパース**: 
  - `res_json["candidates"][0]["content"]["parts"][0]["text"]` を参照してテキストを抽出。
  - そのテキストに対して、**既存の正規表現（`\[.*?\]`）を適用**し、純粋なJSON配列のみを抽出・パースする共通ロジックを通して返却します。

### 3. フロントエンドからのプロバイダ指定
#### [MODIFY] `src/components/kanban/StorySwimlane.tsx`
- **Storeからの読み取り**: AI Generateボタン押下時、`taui-plugin-store` を用いて、直前にStoreから `default-ai-provider` を読み込みます（未設定時はフォールバックで任意の片方を指定）。
- **Tauri Invoke引数**: 取得したプロバイダ文字列を、`generate_tasks_from_story` の `provider` 引数として渡します。

## Verification Plan
1. **設定の保存と切り替え**: SettingsModalでAnthropic/Gemini双方のキーを入力し、デフォルトプロバイダをGeminiに切り替えて保存する。
2. **Gemini APIでの生成テスト**: Story上で生成ボタンを押し、裏側で適切にGemini APIが選択・実行され、パース処理を通過してボードにタスクが並ぶことを確認。
3. **Anthropic APIへのフォールバック（動作確認）**: 再度プロバイダをAnthropicに戻し、既存実装が破壊されず正常に動く（今回は制限下のためエラーが返るか）どうかを確認。
