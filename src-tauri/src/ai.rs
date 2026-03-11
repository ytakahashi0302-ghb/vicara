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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoryDraft {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefinedIdeaResponse {
    pub reply: String,
    pub story_draft: StoryDraft,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatInceptionResponse {
    pub reply: String,
    pub is_finished: bool,
    pub generated_document: Option<String>,
}

async fn get_api_key_and_provider(app: &AppHandle, provider_override: Option<String>) -> Result<(String, String), String> {
    let store = app.store("settings.json").map_err(|e| format!("Failed to access store: {}", e))?;
    
    let provider = match provider_override {
        Some(p) => p,
        None => match store.get("default-ai-provider") {
            Some(val) => {
                if let Some(obj) = val.as_object() {
                    obj.get("value").and_then(|v| v.as_str()).unwrap_or("anthropic").to_string()
                } else if let Some(s) = val.as_str() {
                    s.to_string()
                } else {
                    "anthropic".to_string()
                }
            },
            None => "anthropic".to_string(),
        }
    };

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

    Ok((api_key, provider))
}

#[tauri::command]
pub async fn generate_tasks_from_story(
    app: AppHandle,
    title: String,
    description: String,
    acceptance_criteria: String,
    provider: String,
    project_id: String,
) -> Result<Vec<GeneratedTask>, String> {
    // 1. StoreからAPIキーを取得
    let (api_key, provider_resolved) = get_api_key_and_provider(&app, Some(provider)).await?;

    // プロジェクトのコンテキストを取得
    let context_md = match crate::db::build_project_context(&app, &project_id).await {
        Ok(md) => md,
        Err(e) => {
            println!("Failed to load project context: {}", e);
            String::new()
        }
    };

    // 2. プロンプト生成 (JSONを要求)
    let prompt = format!(
        "以下のユーザーストーリーをもとに、要件を満たすための具体的な実装タスク(To Do)を3〜5個に分解してください。\n\
        \n\
        【ストーリータイトル】\n{}\n\
        【説明】\n{}\n\
        【受け入れ基準】\n{}\n\
        {}\n\
        \n\
        出力は、以下の形式のJSON配列のみとしてください。前後の挨拶やマークダウンブロック(```json)は不要です。必ずパース可能なJSON配列を絶対に出力してください。\n\
        [\n\
          {{\n\
            \"title\": \"タスク名\",\n\
            \"description\": \"具体的なタスクの作業内容\"\n\
          }}\n\
        ]",
        title, description, acceptance_criteria, context_md
    );

    let client = Client::new();
    let content = if provider_resolved == "gemini" {
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

#[tauri::command]
pub async fn refine_idea(
    app: AppHandle,
    idea_seed: String,
    previous_context: Option<Vec<Message>>,
    project_id: String,
) -> Result<RefinedIdeaResponse, String> {
    // 1. StoreからAPIキーとProviderを取得
    let (api_key, provider) = get_api_key_and_provider(&app, None).await?;

    // プロジェクトのコンテキストを取得
    let context_md = match crate::db::build_project_context(&app, &project_id).await {
        Ok(md) => md,
        Err(e) => {
            println!("Failed to load project context: {}", e);
            String::new()
        }
    };

    // 2. システムプロンプト
    let system_prompt = format!("あなたは優秀なPOアシスタントです。ユーザーの入力から、プロダクトの要件を整理し、ユーザーストーリーの草案(draft)を作成する壁打ち相手です。

制約事項:
1. ユーザーの入力に対して過度な共感や感嘆符（！）の多用は避け、親しみやすさを保ちつつも事務的でスムーズな進行を心がけてください。
2. ユーザーの言葉を受け止めた上で、その背後にある「本当の課題」や「理想の体験」を深掘りする質問を1つだけ投げかけてください。「例えば〇〇のようなイメージですか？」と例を添えると良いです。
3. ユーザーとの対話履歴を踏まえ、現在までに判明している要件から「ストーリーの草案 (story_draft)」を作成・更新してください。
4. 出力は必ず以下のJSON形式のみとしてください。前後の挨拶やマークダウンブロック(```json)は一切不要です。絶対にパース可能なJSONを出力してください。
{}
{{
  \"reply\": \"ユーザーへの返答メッセージ（150文字程度）\",
  \"story_draft\": {{
    \"title\": \"ストーリーのタイトル\",
    \"description\": \"ストーリーの詳細な背景や説明\",
    \"acceptance_criteria\": \"- 受け入れ条件1\\n- 受け入れ条件2\"
  }}
}}", context_md);

    let client = Client::new();
    
    // 3. API呼び出し
    let content = if provider == "gemini" {
        // Build Gemini messages
        let mut contents = Vec::new();
        
        if let Some(ctx) = previous_context {
            for msg in ctx {
                let role = if msg.role == "user" { "user" } else { "model" };
                contents.push(serde_json::json!({
                    "role": role,
                    "parts": [{ "text": msg.content }]
                }));
            }
        }
        
        contents.push(serde_json::json!({
            "role": "user",
            "parts": [{ "text": idea_seed }]
        }));

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}", api_key);
        let body = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": 2000,
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
        
        // Geminiの返答抽出
        let text_content = res_json.get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
            .and_then(|arr| arr.get(0))
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| format!("Failed to extract text from Gemini response: {:?}", res_json))?
            .to_string();
            
        text_content
    } else {
        // Build Anthropic messages
        let mut messages = Vec::new();
        
        if let Some(ctx) = previous_context {
            for msg in ctx {
                // Anthropic is strict about user/assistant alternating, 
                // but usually passes through if structured correctly.
                let role = if msg.role == "user" { "user" } else { "assistant" };
                messages.push(serde_json::json!({
                    "role": role,
                    "content": msg.content
                }));
            }
        }
        
        messages.push(serde_json::json!({
            "role": "user",
            "content": idea_seed
        }));

        let body = serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 2000,
            "system": system_prompt,
            "messages": messages
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
        
        // Anthropicの返答抽出
        let text_content = res_json.get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| format!("Failed to extract text from Anthropic response: {:?}", res_json))?
            .to_string();
            
        text_content
    };

    // 4. JSONの抽出とパース (Markdownの不要な装飾を取り除く)
    // ```json と ``` を取り除く
    let cleaned_content = content
        .replace("```json\n", "")
        .replace("```json", "")
        .replace("\n```", "")
        .replace("```", "")
        .trim()
        .to_string();

    let response: RefinedIdeaResponse = serde_json::from_str(&cleaned_content)
        .map_err(|e| format!("Failed to parse JSON Object from AI output: {}\nExtracted String: {}", e, cleaned_content))?;

    Ok(response)
}

#[tauri::command]
pub async fn chat_inception(
    app: AppHandle,
    project_id: String,
    phase: u32,
    messages_history: Vec<Message>,
) -> Result<ChatInceptionResponse, String> {
    let (api_key, provider) = get_api_key_and_provider(&app, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id).await.unwrap_or_default();

    let phase_instruction = match phase {
        1 => "現在は Phase 1 (Why) です。プロダクトのコア価値とターゲットについて議論します。",
        2 => "現在は Phase 2 (Not List) です。やらないことリストについて議論します。",
        3 => "現在は Phase 3 (What) です。技術スタックやアーキテクチャの制約について議論します。",
        4 => "現在は Phase 4 (How) です。プロジェクト固有の開発ルールやAIへの追加ルールについて議論します。",
        _ => "現在は最終確認フェーズです",
    };

    let doc_target = match phase {
        1 | 2 => "PRODUCT_CONTEXT.md",
        3 => "ARCHITECTURE.md",
        4 => "Rule.md",
        _ => "",
    };

    let system_prompt = format!(
"あなたはアジャイル開発プロジェクトの開始時における「インセプションデッキ」作成をサポートする優秀なAIアシスタントです。
ユーザーと対話し、プロジェクトの方向性を明確化します。

【現在の状況】
{}

【既存のコンテキスト】
{}

【制約事項】
1. ユーザーとの対話を通じて要件を深掘りしてください。ただし、一問一答ではなく、回答を受けて1〜2つだけ簡潔に深掘りする質問を投げかけてください。
2. 議論が十分だと判断したら、自ら「次のフェーズへ進みますか？」と提案してください。
3. フェーズの終了が合意された場合、またはユーザーからの指示があった場合は、`is_finished` を true にし、該当するドキュメント（{}）に記載すべき内容をMarkdownフォーマットで `generated_document` に含めてください。
4. そうでない場合は `is_finished` は false にし、`generated_document` は null としてください。
5. 出力は必ず以下のJSON形式のみとし、前後の挨拶やマークダウンブロック(```json)は含めないでください。

{{
  \"reply\": \"ユーザーへの返信チャットメッセージ\",
  \"is_finished\": true または false,
  \"generated_document\": \"Markdown形式のドキュメント内容\" または null
}}", phase_instruction, context_md, doc_target);

    let client = Client::new();
    
    let content = if provider == "gemini" {
        let mut contents = Vec::new();
        for msg in messages_history {
            let role = if msg.role == "user" { "user" } else { "model" };
            contents.push(serde_json::json!({
                "role": role,
                "parts": [{ "text": msg.content }]
            }));
        }

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}", api_key);
        let body = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": 2048,
                "responseMimeType": "application/json"
            }
        });

        let res = client.post(&url).header("content-type", "application/json").json(&body).send().await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Gemini API Request Failed ({}) - {}", status, text));
        }

        let res_json: Value = res.json().await.map_err(|e| format!("Failed to parse JSON: {}", e))?;
        res_json.get("candidates").and_then(|c| c.as_array()).and_then(|arr| arr.get(0)).and_then(|c| c.get("content")).and_then(|c| c.get("parts")).and_then(|p| p.as_array()).and_then(|arr| arr.get(0)).and_then(|p| p.get("text")).and_then(|t| t.as_str())
            .ok_or_else(|| format!("Extract text failed"))?.to_string()
    } else {
        let mut messages = Vec::new();
        for msg in messages_history {
            let role = if msg.role == "user" { "user" } else { "assistant" };
            messages.push(serde_json::json!({ "role": role, "content": msg.content }));
        }

        let body = serde_json::json!({
            "model": "claude-3-5-sonnet-20241022",
            "max_tokens": 2048,
            "system": system_prompt,
            "messages": messages
        });

        let res = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key).header("anthropic-version", "2023-06-01").header("content-type", "application/json")
            .json(&body).send().await.map_err(|e| format!("Network request failed: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Anthropic API Request Failed ({}) - {}", status, text));
        }

        let res_json: Value = res.json().await.map_err(|e| format!("Failed to parse JSON: {}", e))?;
        res_json.get("content").and_then(|c| c.as_array()).and_then(|arr| arr.get(0)).and_then(|c| c.get("text")).and_then(|t| t.as_str())
            .ok_or_else(|| format!("Extract text failed"))?.to_string()
    };

    let cleaned_content = content.replace("```json\n", "").replace("```json", "").replace("\n```", "").replace("```", "").trim().to_string();
    let response: ChatInceptionResponse = serde_json::from_str(&cleaned_content)
        .map_err(|e| format!("Failed to parse JSON: {}\nExtracted String: {}", e, cleaned_content))?;

    Ok(response)
}
