use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::completion::message::Message as RigMessage;
use rig::completion::Chat;
use rig::providers::anthropic;
use rig::providers::anthropic::completion::{
    CompletionModel as AnthropicModel, CLAUDE_3_5_HAIKU,
};
use rig::providers::gemini;
use rig::providers::gemini::completion::{CompletionModel as GeminiModel, GEMINI_2_0_FLASH};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

#[derive(Debug, Clone, PartialEq)]
pub enum AiProvider {
    Anthropic,
    Gemini,
}

impl AiProvider {
    pub fn from_str(s: &str) -> Self {
        match s {
            "gemini" => AiProvider::Gemini,
            _ => AiProvider::Anthropic,
        }
    }
}

/// Resolve the AI provider and API key from the Tauri store.
pub async fn resolve_provider_and_key(
    app: &AppHandle,
    provider_override: Option<String>,
) -> Result<(AiProvider, String), String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;

    let provider = match provider_override {
        Some(p) => AiProvider::from_str(&p),
        None => match store.get("default-ai-provider") {
            Some(val) => {
                let s = if let Some(obj) = val.as_object() {
                    obj.get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("anthropic")
                } else if let Some(s) = val.as_str() {
                    s
                } else {
                    "anthropic"
                };
                AiProvider::from_str(s)
            }
            None => AiProvider::Anthropic,
        },
    };

    let key_name = match provider {
        AiProvider::Gemini => "gemini-api-key",
        AiProvider::Anthropic => "anthropic-api-key",
    };

    let api_key = match store.get(key_name) {
        Some(val) => {
            if let Some(obj) = val.as_object() {
                obj.get("value")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("{} format mismatch", key_name))?
            } else if let Some(s) = val.as_str() {
                s.to_string()
            } else {
                return Err(format!("{} format mismatch", key_name));
            }
        }
        None => return Err(format!("{} is not set", key_name)),
    };

    Ok((provider, api_key))
}

/// Convert the app's Message type to Rig's Message type.
pub fn convert_messages(messages: &[crate::ai::Message]) -> Vec<RigMessage> {
    messages
        .iter()
        .map(|m| match m.role.as_str() {
            "user" => RigMessage::user(&m.content),
            "assistant" => RigMessage::assistant(&m.content),
            "system" => RigMessage::system(&m.content),
            _ => RigMessage::user(&m.content),
        })
        .collect()
}

async fn chat_anthropic(
    api_key: &str,
    system_prompt: &str,
    user_input: &str,
    chat_history: Vec<RigMessage>,
) -> Result<String, String> {
    let client = anthropic::Client::new(api_key)
        .map_err(|e| format!("Failed to create Anthropic client: {}", e))?;
    let agent: Agent<AnthropicModel> = client
        .agent(CLAUDE_3_5_HAIKU)
        .preamble(system_prompt)
        .max_tokens(4096)
        .build();
    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        agent.chat(user_input, chat_history),
    )
    .await
    .map_err(|_| "Anthropic API timed out after 60 seconds".to_string())?
    .map_err(|e| format!("Anthropic error: {}", e))
}

async fn chat_gemini(
    api_key: &str,
    system_prompt: &str,
    user_input: &str,
    chat_history: Vec<RigMessage>,
) -> Result<String, String> {
    let client = gemini::Client::new(api_key)
        .map_err(|e| format!("Failed to create Gemini client: {}", e))?;
    let agent: Agent<GeminiModel> = client
        .agent(GEMINI_2_0_FLASH)
        .preamble(system_prompt)
        .max_tokens(4096)
        .build();
    tokio::time::timeout(
        std::time::Duration::from_secs(60),
        agent.chat(user_input, chat_history),
    )
    .await
    .map_err(|_| "Gemini API timed out after 60 seconds".to_string())?
    .map_err(|e| format!("Gemini error: {}", e))
}

/// Send a prompt with conversation history via Rig and return the raw text response.
/// For single-turn prompts, pass an empty Vec for `chat_history`.
pub async fn chat_with_history(
    provider: &AiProvider,
    api_key: &str,
    system_prompt: &str,
    user_input: &str,
    chat_history: Vec<RigMessage>,
) -> Result<String, String> {
    match provider {
        AiProvider::Anthropic => {
            chat_anthropic(api_key, system_prompt, user_input, chat_history).await
        }
        AiProvider::Gemini => {
            chat_gemini(api_key, system_prompt, user_input, chat_history).await
        }
    }
}
