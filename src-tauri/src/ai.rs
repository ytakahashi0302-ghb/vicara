use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;
use reqwest::Client;

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedTask {
    pub title: String,
    pub description: String,
}

#[tauri::command]
pub async fn generate_tasks_from_story<R: Runtime>(
    app: AppHandle<R>,
    title: String,
    description: String,
    acceptance_criteria: String,
    provider: String,
) -> Result<Vec<GeneratedTask>, String> {
    // 1. StoreからAPIキーを取得
    let store = app.store("settings.json").map_err(|e| format!("Failed to access store: {}", e))?;
    
    let key_name = if provider == "gemini" { "gemini-api-key" } else { "anthropic-api-key" };
    let api_key = match store.get(key_name) {
        Some(val) => {
            if let Some(obj) = val.as_object() {
                if let Some(v) = obj.get("value").and_then(|v| v.as_str()) {
                    v.to_string()
                } else {
                    return Err(format!("{} is not a valid string in value object", key_name));
                }
            } else if let Some(s) = val.as_str() {
                s.to_string()
            } else {
                return Err(format!("{} has invalid format in store", key_name));
            }
        },
        None => return Err(format!("{} is not set. Please configure it in Settings.", key_name)),
    };

    if api_key.trim().is_empty() {
        return Err(format!("{} is empty. Please configure it in Settings.", key_name));
    }

    // 2. プロンプト生成 (JSONを要求)
    let prompt = format!(
        "以下のユーザーストーリーをもとに、要件を満たすための具体的な実装タスク(To Do)を3〜5個に分解してください。\n\
        \n\
        【ストーリータイトル】\n{}\n\
        【説明】\n{}\n\
        【受け入れ基準】\n{}\n\
        \n\
        出力は、以下の形式のJSON配列のみとしてください。前後の挨拶やマークダウンブロック(```json)は不要です。必ずパース可能なJSON配列を絶対に出力してください。\n\
        [\n\
          {{\n\
            \"title\": \"タスク名\",\n\
            \"description\": \"具体的なタスクの作業内容\"\n\
          }}\n\
        ]",
        title, description, acceptance_criteria
    );

    let client = Client::new();
    let content = if provider == "gemini" {
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}", api_key);
        let body = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": "You are an expert Agile Scrum Master and Developer. Break down stories into practical, technical, actionable tasks." }]
            },
            "contents": [
                {
                    "parts": [{ "text": prompt }]
                }
            ],
            "generationConfig": {
                "responseMimeType": "application/json"
            }
        });

        let res = client.post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Gemini API Request Failed ({}) - {}", status, text));
        }

        let res_json: Value = res.json().await.map_err(|e| format!("Failed to parse response JSON: {}", e))?;
        
        let text_content = res_json["candidates"][0]["content"]["parts"][0]["text"].as_str()
            .ok_or("Failed to extract text from Gemini response")?
            .to_string();
            
        text_content
    } else {
        let body = serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 1500,
            "system": "You are an expert Agile Scrum Master and Developer. Break down stories into practical, technical, actionable tasks.",
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let res = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Anthropic API Request Failed ({}) - {}", status, text));
        }

        let res_json: Value = res.json().await.map_err(|e| format!("Failed to parse response JSON: {}", e))?;
        
        let text_content = res_json["content"][0]["text"].as_str()
            .ok_or("Failed to extract text from Anthropic response")?
            .to_string();
            
        text_content
    };

    // 3. レスポンスからのJSON抽出（Markdownコードブロック除去・正規表現パース）
    // JSON配列 "[ ... ]" を部分抽出して安全にパースする
    let re = regex::Regex::new(r"(?s)\[.*?\]").map_err(|e| e.to_string())?;
    
    let json_str = if let Some(caps) = re.captures(&content) {
        caps.get(0).map_or(content.as_str(), |m| m.as_str())
    } else {
        content.as_str()
    };

    let tasks: Vec<GeneratedTask> = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse JSON array from AI output: {}\nExtracted String: {}", e, json_str))?;

    Ok(tasks)
}
