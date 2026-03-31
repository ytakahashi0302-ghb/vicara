use serde::{Deserialize, Serialize};
use tauri::AppHandle;

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTaskResponse {
    pub reply: String,
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
    let (provider_enum, api_key) = crate::rig_provider::resolve_provider_and_key(&app, Some(provider)).await?;
    let _context_md = crate::db::build_project_context(&app, &project_id).await.unwrap_or_default();
    let prompt = format!("Context: {}\nStory: {}\nDesc: {}\nAC: {}\nJSON Array Output Please.", _context_md, title, description, acceptance_criteria);

    let system_prompt = "You are a task decomposition expert. Generate a JSON array of tasks.";
    let response = crate::rig_provider::chat_with_history(
        &provider_enum,
        &api_key,
        system_prompt,
        &prompt,
        vec![], // No conversation history
    )
    .await?;

    let re = regex::Regex::new(r"(?s)\[.*?\]").map_err(|e| e.to_string())?;
    let json_str = re.captures(&response).and_then(|caps| caps.get(0)).map_or(response.as_str(), |m| m.as_str());
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refine_idea(
    app: AppHandle,
    idea_seed: String,
    previous_context: Option<Vec<Message>>,
    project_id: String,
) -> Result<RefinedIdeaResponse, String> {
    let (provider, api_key) = crate::rig_provider::resolve_provider_and_key(&app, None).await?;
    let _context_md = crate::db::build_project_context(&app, &project_id).await.unwrap_or_default();

    let chat_history = if let Some(ctx) = previous_context {
        crate::rig_provider::convert_messages(&ctx)
    } else {
        vec![]
    };

    let system_prompt = "PO Assist";
    let content = crate::rig_provider::chat_with_history(
        &provider,
        &api_key,
        system_prompt,
        &idea_seed,
        chat_history,
    )
    .await?;

    let re = regex::Regex::new(r"(?s)\{.*?\}").unwrap();
    let json_str = re.captures(&content).and_then(|caps| caps.get(0)).map_or(content.as_str(), |m| m.as_str());
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn chat_inception(
    app: AppHandle,
    project_id: String,
    _phase: u32,
    messages_history: Vec<Message>,
) -> Result<ChatInceptionResponse, String> {
    let (provider, api_key) = crate::rig_provider::resolve_provider_and_key(&app, None).await?;
    let _context_md = crate::db::build_project_context(&app, &project_id).await.unwrap_or_default();

    let chat_history = crate::rig_provider::convert_messages(&messages_history);
    let system_prompt = "Inception Guide";

    let content = crate::rig_provider::chat_with_history(
        &provider,
        &api_key,
        system_prompt,
        "", // Empty user input - using chat history instead
        chat_history,
    )
    .await?;

    let re = regex::Regex::new(r"(?s)\{.*?\}").unwrap();
    let json_str = re.captures(&content).and_then(|caps| caps.get(0)).map_or(content.as_str(), |m| m.as_str());
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn chat_with_team_leader(
    app: AppHandle,
    project_id: String,
    messages_history: Vec<Message>,
) -> Result<ChatTaskResponse, String> {
    let (provider, api_key) = crate::rig_provider::resolve_provider_and_key(&app, None).await?;
    let _context_md = crate::db::build_project_context(&app, &project_id).await.unwrap_or_default();

    let chat_history = crate::rig_provider::convert_messages(&messages_history);
    let system_prompt = "あなたはScrum TeamのAI Team Leaderです。必ず以下の形式のJSONのみで回答してください。\n\n{\"reply\": \"メッセージ内容\"}";

    let raw_text = crate::rig_provider::chat_with_history(
        &provider,
        &api_key,
        system_prompt,
        "", // Empty user input - using chat history instead
        chat_history,
    )
    .await?;

    let re = regex::Regex::new(r"(?s)\{.*?\}").unwrap();
    let json_str = if let Some(caps) = re.captures(&raw_text) {
        caps.get(0).unwrap().as_str()
    } else {
        &raw_text
    };

    let resp: ChatTaskResponse = serde_json::from_str(json_str).map_err(|e| {
        format!("JSON parse error: {}. Raw: {}", e, raw_text)
    })?;

    Ok(resp)
}
