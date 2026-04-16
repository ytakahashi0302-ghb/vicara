use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::completion::message::Message as RigMessage;
use rig::completion::Prompt;
use rig::providers::anthropic;
use rig::providers::anthropic::completion::CompletionModel as AnthropicModel;
use rig::providers::gemini;
use rig::providers::gemini::completion::CompletionModel as GeminiModel;
use rig::providers::openai;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const DEFAULT_OLLAMA_ENDPOINT: &str = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";
const DEFAULT_GEMINI_MODEL: &str = "gemini-3-flash-preview";
const GEMINI_MAX_RETRIES: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub enum AiProvider {
    Anthropic,
    Gemini,
    OpenAI,
    Ollama,
}

impl AiProvider {
    pub fn from_str(s: &str) -> Self {
        match s {
            "gemini" => AiProvider::Gemini,
            "openai" => AiProvider::OpenAI,
            "ollama" => AiProvider::Ollama,
            _ => AiProvider::Anthropic,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyStatus {
    pub name: String,
    pub display_name: String,
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub running: bool,
    pub models: Vec<String>,
    pub endpoint: String,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

fn extract_store_string_value(value: serde_json::Value) -> Option<String> {
    if let Some(obj) = value.as_object() {
        obj.get("value")
            .and_then(|inner| inner.as_str())
            .map(|inner| inner.to_string())
    } else {
        value.as_str().map(|inner| inner.to_string())
    }
}

fn has_configured_store_value(value: Option<serde_json::Value>) -> bool {
    value
        .and_then(extract_store_string_value)
        .map(|inner| !inner.trim().is_empty())
        .unwrap_or(false)
}

fn normalize_ollama_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    let normalized = if trimmed.is_empty() {
        DEFAULT_OLLAMA_ENDPOINT
    } else {
        trimmed
    };

    normalized.trim_end_matches('/').to_string()
}

fn build_ollama_openai_base_url(endpoint: &str) -> String {
    let normalized = normalize_ollama_endpoint(endpoint);
    if normalized.ends_with("/v1") {
        normalized
    } else {
        format!("{normalized}/v1")
    }
}

fn build_ollama_tags_url(endpoint: &str) -> String {
    let normalized = normalize_ollama_endpoint(endpoint);
    let root = normalized
        .strip_suffix("/v1")
        .unwrap_or(normalized.as_str());
    format!("{}/api/tags", root.trim_end_matches('/'))
}

fn is_retryable_gemini_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("503")
        || normalized.contains("unavailable")
        || normalized.contains("overloaded")
}

fn gemini_retry_delay(retry_index: usize) -> Duration {
    Duration::from_secs(2_u64.pow((retry_index as u32) + 1))
}

fn build_gemini_retry_exhausted_error(last_error: &str) -> String {
    format!(
        "Gemini API が継続的に UNAVAILABLE を返しました。{} 回再試行しましたが回復しませんでした。最後のエラー: {}",
        GEMINI_MAX_RETRIES,
        last_error
    )
}

fn build_openai_completion_client(api_key: &str) -> Result<openai::CompletionsClient, String> {
    openai::Client::builder()
        .api_key(api_key)
        .build()
        .map(|client| client.completions_api())
        .map_err(|e| format!("Failed to create OpenAI client: {}", e))
}

fn build_ollama_completion_client(endpoint: &str) -> Result<openai::CompletionsClient, String> {
    let base_url = build_ollama_openai_base_url(endpoint);

    openai::Client::builder()
        .api_key("ollama")
        .base_url(&base_url)
        .build()
        .map(|client| client.completions_api())
        .map_err(|e| format!("Failed to create Ollama client: {}", e))
}

async fn fetch_ollama_status(endpoint: &str) -> OllamaStatus {
    let normalized_endpoint = normalize_ollama_endpoint(endpoint);
    let url = build_ollama_tags_url(&normalized_endpoint);
    let client = reqwest::Client::new();

    match client
        .get(&url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
    {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let message = response
                    .text()
                    .await
                    .ok()
                    .filter(|body| !body.trim().is_empty())
                    .map(|body| format!("{} が {} を返しました: {}", url, status, body))
                    .or_else(|| Some(format!("{} が {} を返しました。", url, status)));

                return OllamaStatus {
                    running: false,
                    models: vec![],
                    endpoint: normalized_endpoint,
                    message,
                };
            }

            match response.json::<OllamaTagsResponse>().await {
                Ok(payload) => {
                    let mut models = payload
                        .models
                        .into_iter()
                        .map(|model| model.name)
                        .collect::<Vec<_>>();
                    models.sort();

                    OllamaStatus {
                        running: true,
                        models,
                        endpoint: normalized_endpoint,
                        message: None,
                    }
                }
                Err(error) => OllamaStatus {
                    running: false,
                    models: vec![],
                    endpoint: normalized_endpoint,
                    message: Some(format!("Ollama 応答の JSON 解析に失敗しました: {}", error)),
                },
            }
        }
        Err(error) => OllamaStatus {
            running: false,
            models: vec![],
            endpoint: normalized_endpoint,
            message: Some(format!("Ollama へ接続できませんでした: {}", error)),
        },
    }
}

/// Resolve the AI provider and API key from the Tauri store.
pub async fn resolve_provider_and_key(
    app: &AppHandle,
    provider_override: Option<String>,
) -> Result<(AiProvider, String, String), String> {
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

    let (key_name, model_key_name, default_model) = match provider {
        AiProvider::Gemini => ("gemini-api-key", "gemini-model", DEFAULT_GEMINI_MODEL),
        AiProvider::OpenAI => ("openai-api-key", "openai-model", "gpt-5.4-mini"),
        AiProvider::Ollama => ("ollama-endpoint", "ollama-model", DEFAULT_OLLAMA_MODEL),
        AiProvider::Anthropic => ("anthropic-api-key", "anthropic-model", "claude-haiku-4-5"),
    };

    let api_key = match provider {
        AiProvider::Ollama => {
            let endpoint = store
                .get(key_name)
                .and_then(extract_store_string_value)
                .unwrap_or_else(|| DEFAULT_OLLAMA_ENDPOINT.to_string());
            normalize_ollama_endpoint(&endpoint)
        }
        _ => match store.get(key_name) {
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
        },
    };

    let model = match store.get(model_key_name) {
        Some(val) => {
            if let Some(obj) = val.as_object() {
                obj.get("value")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(default_model)
                    .to_string()
            } else if let Some(s) = val.as_str() {
                if s.is_empty() {
                    default_model.to_string()
                } else {
                    s.to_string()
                }
            } else {
                default_model.to_string()
            }
        }
        None => default_model.to_string(),
    };

    Ok((provider, api_key, model))
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

#[derive(Debug, Clone)]
pub struct LlmTextResponse {
    pub content: String,
    pub provider: String,
    pub model: String,
    pub usage: crate::llm_observability::NormalizedUsage,
    pub raw_usage_json: serde_json::Value,
    pub started_at: i64,
    pub completed_at: i64,
}

fn current_timestamp_millis() -> Result<i64, String> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
}

async fn chat_anthropic(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    mut chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    let started_at = current_timestamp_millis()?;
    let client = anthropic::Client::new(api_key)
        .map_err(|e| format!("Failed to create Anthropic client: {}", e))?;
    let agent: Agent<AnthropicModel> = client
        .agent(model)
        .preamble(system_prompt)
        .max_tokens(4096)
        .build();
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        agent
            .prompt(user_input)
            .with_history(&mut chat_history)
            .extended_details(),
    )
    .await
    .map_err(|_| "Anthropic API timed out after 60 seconds".to_string())?
    .map_err(|e| format!("Anthropic error: {}", e))?;
    let completed_at = current_timestamp_millis()?;
    let usage = crate::llm_observability::NormalizedUsage::from(response.usage);

    Ok(LlmTextResponse {
        content: response.output,
        provider: "anthropic".to_string(),
        model: model.to_string(),
        usage,
        raw_usage_json: json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
            "cached_input_tokens": usage.cached_input_tokens,
        }),
        started_at,
        completed_at,
    })
}

async fn chat_gemini(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    let started_at = current_timestamp_millis()?;
    let base_history = chat_history;
    let mut retry_count = 0;

    loop {
        let client = gemini::Client::new(api_key)
            .map_err(|e| format!("Failed to create Gemini client: {}", e))?;
        let mut attempt_history = base_history.clone();
        let agent: Agent<GeminiModel> = client
            .agent(model)
            .preamble(system_prompt)
            .max_tokens(4096)
            .build();
        let response = tokio::time::timeout(
            Duration::from_secs(60),
            agent
                .prompt(user_input)
                .with_history(&mut attempt_history)
                .extended_details(),
        )
        .await
        .map_err(|_| "Gemini API timed out after 60 seconds".to_string())
        .and_then(|result| result.map_err(|e| format!("Gemini error: {}", e)));

        match response {
            Ok(response) => {
                let completed_at = current_timestamp_millis()?;
                let usage = crate::llm_observability::NormalizedUsage::from(response.usage);

                return Ok(LlmTextResponse {
                    content: response.output,
                    provider: "gemini".to_string(),
                    model: model.to_string(),
                    usage,
                    raw_usage_json: json!({
                        "input_tokens": usage.input_tokens,
                        "output_tokens": usage.output_tokens,
                        "total_tokens": usage.total_tokens,
                        "cached_input_tokens": usage.cached_input_tokens,
                    }),
                    started_at,
                    completed_at,
                });
            }
            Err(error) if retry_count < GEMINI_MAX_RETRIES && is_retryable_gemini_error(&error) => {
                tokio::time::sleep(gemini_retry_delay(retry_count)).await;
                retry_count += 1;
            }
            Err(error) if is_retryable_gemini_error(&error) => {
                return Err(build_gemini_retry_exhausted_error(&error));
            }
            Err(error) => return Err(error),
        }
    }
}

async fn chat_openai_compatible(
    client: openai::CompletionsClient,
    provider_name: &str,
    error_label: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    mut chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    let started_at = current_timestamp_millis()?;
    let agent = client
        .agent(model)
        .preamble(system_prompt)
        .max_tokens(4096)
        .build();
    let response = tokio::time::timeout(
        Duration::from_secs(60),
        agent
            .prompt(user_input)
            .with_history(&mut chat_history)
            .extended_details(),
    )
    .await
    .map_err(|_| format!("{} API timed out after 60 seconds", error_label))?
    .map_err(|e| format!("{} error: {}", error_label, e))?;
    let completed_at = current_timestamp_millis()?;
    let usage = crate::llm_observability::NormalizedUsage::from(response.usage);

    Ok(LlmTextResponse {
        content: response.output,
        provider: provider_name.to_string(),
        model: model.to_string(),
        usage,
        raw_usage_json: json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
            "cached_input_tokens": usage.cached_input_tokens,
        }),
        started_at,
        completed_at,
    })
}

async fn chat_openai(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    let client = build_openai_completion_client(api_key)?;
    chat_openai_compatible(
        client,
        "openai",
        "OpenAI",
        model,
        system_prompt,
        user_input,
        chat_history,
    )
    .await
}

async fn chat_ollama(
    endpoint: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    let client = build_ollama_completion_client(endpoint)?;
    chat_openai_compatible(
        client,
        "ollama",
        "Ollama",
        model,
        system_prompt,
        user_input,
        chat_history,
    )
    .await
}

/// Send a prompt with conversation history via Rig and return the raw text response.
/// For single-turn prompts, pass an empty Vec for `chat_history`.
pub async fn chat_with_history(
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    chat_history: Vec<RigMessage>,
) -> Result<LlmTextResponse, String> {
    match provider {
        AiProvider::Anthropic => {
            chat_anthropic(api_key, model, system_prompt, user_input, chat_history).await
        }
        AiProvider::Gemini => {
            chat_gemini(api_key, model, system_prompt, user_input, chat_history).await
        }
        AiProvider::OpenAI => {
            chat_openai(api_key, model, system_prompt, user_input, chat_history).await
        }
        AiProvider::Ollama => {
            chat_ollama(api_key, model, system_prompt, user_input, chat_history).await
        }
    }
}

async fn chat_openai_compatible_with_tools(
    client: openai::CompletionsClient,
    app: &AppHandle,
    provider_name: &str,
    error_label: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    mut chat_history: Vec<RigMessage>,
    project_id: &str,
) -> Result<LlmTextResponse, String> {
    let tool = crate::ai_tools::CreateStoryAndTasksTool {
        app: app.clone(),
        project_id: project_id.to_string(),
    };

    let started_at = current_timestamp_millis()?;
    let agent = client
        .agent(model)
        .preamble(system_prompt)
        .max_tokens(4096)
        .tool(tool)
        .default_max_turns(5)
        .build();
    let response = tokio::time::timeout(
        Duration::from_secs(60),
        agent
            .prompt(user_input)
            .with_history(&mut chat_history)
            .max_turns(5)
            .extended_details(),
    )
    .await
    .map_err(|_| format!("{} API timed out after 60 seconds", error_label))?
    .map_err(|e| format!("{} error: {}", error_label, e))?;
    let completed_at = current_timestamp_millis()?;
    let usage = crate::llm_observability::NormalizedUsage::from(response.usage);
    let message_count = response.messages.as_ref().map(|messages| messages.len());

    Ok(LlmTextResponse {
        content: response.output,
        provider: provider_name.to_string(),
        model: model.to_string(),
        usage,
        raw_usage_json: json!({
            "input_tokens": usage.input_tokens,
            "output_tokens": usage.output_tokens,
            "total_tokens": usage.total_tokens,
            "cached_input_tokens": usage.cached_input_tokens,
            "messages_count": message_count
        }),
        started_at,
        completed_at,
    })
}

pub async fn chat_team_leader_with_tools(
    app: &AppHandle,
    provider: &AiProvider,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    mut chat_history: Vec<RigMessage>,
    project_id: &str,
) -> Result<LlmTextResponse, String> {
    match provider {
        AiProvider::Anthropic => {
            let tool = crate::ai_tools::CreateStoryAndTasksTool {
                app: app.clone(),
                project_id: project_id.to_string(),
            };
            let note_tool = crate::ai_tools::AddProjectNoteTool {
                app: app.clone(),
                project_id: project_id.to_string(),
            };
            let retro_tool = crate::ai_tools::SuggestRetroItemTool {
                app: app.clone(),
                project_id: project_id.to_string(),
            };
            let started_at = current_timestamp_millis()?;
            let client = anthropic::Client::new(api_key)
                .map_err(|e| format!("Failed to create Anthropic client: {}", e))?;
            let agent = client
                .agent(model)
                .preamble(system_prompt)
                .max_tokens(4096)
                .tool(tool)
                .tool(note_tool)
                .tool(retro_tool)
                .default_max_turns(5)
                .build();
            let response = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                agent
                    .prompt(user_input)
                    .with_history(&mut chat_history)
                    .max_turns(5)
                    .extended_details(),
            )
            .await
            .map_err(|_| "Anthropic API timed out after 60 seconds".to_string())?
            .map_err(|e| format!("Anthropic error: {}", e))?;
            let completed_at = current_timestamp_millis()?;
            let usage = crate::llm_observability::NormalizedUsage::from(response.usage);
            let message_count = response.messages.as_ref().map(|messages| messages.len());

            Ok(LlmTextResponse {
                content: response.output,
                provider: "anthropic".to_string(),
                model: model.to_string(),
                usage,
                raw_usage_json: json!({
                    "input_tokens": usage.input_tokens,
                    "output_tokens": usage.output_tokens,
                    "total_tokens": usage.total_tokens,
                    "cached_input_tokens": usage.cached_input_tokens,
                    "messages_count": message_count
                }),
                started_at,
                completed_at,
            })
        }
        AiProvider::Gemini => {
            let started_at = current_timestamp_millis()?;
            let base_history = chat_history;
            let mut retry_count = 0;

            loop {
                let client = gemini::Client::new(api_key)
                    .map_err(|e| format!("Failed to create Gemini client: {}", e))?;
                let tool = crate::ai_tools::CreateStoryAndTasksTool {
                    app: app.clone(),
                    project_id: project_id.to_string(),
                };
                let note_tool = crate::ai_tools::AddProjectNoteTool {
                    app: app.clone(),
                    project_id: project_id.to_string(),
                };
                let retro_tool = crate::ai_tools::SuggestRetroItemTool {
                    app: app.clone(),
                    project_id: project_id.to_string(),
                };
                let mut attempt_history = base_history.clone();
                let agent = client
                    .agent(model)
                    .preamble(system_prompt)
                    .max_tokens(4096)
                    .tool(tool)
                    .tool(note_tool)
                    .tool(retro_tool)
                    .default_max_turns(5)
                    .build();
                let response = tokio::time::timeout(
                    Duration::from_secs(60),
                    agent
                        .prompt(user_input)
                        .with_history(&mut attempt_history)
                        .max_turns(5)
                        .extended_details(),
                )
                .await
                .map_err(|_| "Gemini API timed out after 60 seconds".to_string())
                .and_then(|result| result.map_err(|e| format!("Gemini error: {}", e)));

                match response {
                    Ok(response) => {
                        let completed_at = current_timestamp_millis()?;
                        let usage = crate::llm_observability::NormalizedUsage::from(response.usage);
                        let message_count =
                            response.messages.as_ref().map(|messages| messages.len());

                        return Ok(LlmTextResponse {
                            content: response.output,
                            provider: "gemini".to_string(),
                            model: model.to_string(),
                            usage,
                            raw_usage_json: json!({
                                "input_tokens": usage.input_tokens,
                                "output_tokens": usage.output_tokens,
                                "total_tokens": usage.total_tokens,
                                "cached_input_tokens": usage.cached_input_tokens,
                                "messages_count": message_count
                            }),
                            started_at,
                            completed_at,
                        });
                    }
                    Err(error)
                        if retry_count < GEMINI_MAX_RETRIES
                            && is_retryable_gemini_error(&error) =>
                    {
                        tokio::time::sleep(gemini_retry_delay(retry_count)).await;
                        retry_count += 1;
                    }
                    Err(error) if is_retryable_gemini_error(&error) => {
                        return Err(build_gemini_retry_exhausted_error(&error));
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        AiProvider::OpenAI => {
            let client = build_openai_completion_client(api_key)?;
            chat_openai_compatible_with_tools(
                client,
                app,
                "openai",
                "OpenAI",
                model,
                system_prompt,
                user_input,
                chat_history,
                project_id,
            )
            .await
        }
        AiProvider::Ollama => {
            let client = build_ollama_completion_client(api_key)?;
            chat_openai_compatible_with_tools(
                client,
                app,
                "ollama",
                "Ollama",
                model,
                system_prompt,
                user_input,
                chat_history,
                project_id,
            )
            .await
        }
    }
}

#[tauri::command]
pub async fn get_available_models(
    app: tauri::AppHandle,
    provider: String,
    api_key_override: Option<String>,
    endpoint_override: Option<String>,
) -> Result<Vec<String>, String> {
    use tauri_plugin_store::StoreExt;
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;

    match provider.to_lowercase().as_str() {
        "gemini" => {
            let api_key = api_key_override
                .filter(|value| !value.trim().is_empty())
                .or_else(|| match store.get("gemini-api-key") {
                    Some(val) => {
                        if let Some(obj) = val.as_object() {
                            obj.get("value")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        } else {
                            val.as_str().map(|s| s.to_string())
                        }
                    }
                    None => None,
                })
                .ok_or("Gemini API key is not set")?;

            let client = reqwest::Client::new();
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                api_key
            );
            let res = client
                .get(&url)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let json: serde_json::Value = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse JSON: {}", e))?;

            let mut models = vec![];
            if let Some(data) = json.get("models").and_then(|v| v.as_array()) {
                for m in data {
                    if let Some(name) = m.get("name").and_then(|v| v.as_str()) {
                        let display_name = name.strip_prefix("models/").unwrap_or(name);
                        models.push(display_name.to_string());
                    }
                }
            } else {
                return Err("Invalid response format from Gemini API".into());
            }

            Ok(models)
        }
        "openai" => {
            let api_key = api_key_override
                .filter(|value| !value.trim().is_empty())
                .or_else(|| match store.get("openai-api-key") {
                    Some(val) => {
                        if let Some(obj) = val.as_object() {
                            obj.get("value")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        } else {
                            val.as_str().map(|s| s.to_string())
                        }
                    }
                    None => None,
                })
                .ok_or("OpenAI API key is not set")?;

            let client = reqwest::Client::new();
            let res = client
                .get("https://api.openai.com/v1/models")
                .bearer_auth(api_key)
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                return Err(if body.trim().is_empty() {
                    format!("OpenAI API error: {}", status)
                } else {
                    format!("OpenAI API error: {}: {}", status, body)
                });
            }

            let json: serde_json::Value = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse JSON: {}", e))?;

            let mut models = vec![];
            if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
                for m in data {
                    if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
                        models.push(id.to_string());
                    }
                }
            } else {
                return Err("Invalid response format from OpenAI API".into());
            }

            models.sort();
            Ok(models)
        }
        "ollama" => {
            let endpoint = endpoint_override
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    store
                        .get("ollama-endpoint")
                        .and_then(extract_store_string_value)
                })
                .unwrap_or_else(|| DEFAULT_OLLAMA_ENDPOINT.to_string());
            let status = fetch_ollama_status(&endpoint).await;

            if status.running {
                Ok(status.models)
            } else {
                Err(status
                    .message
                    .unwrap_or_else(|| format!("Ollama is not running at {}", status.endpoint)))
            }
        }
        _ => {
            let api_key = api_key_override
                .filter(|value| !value.trim().is_empty())
                .or_else(|| match store.get("anthropic-api-key") {
                    Some(val) => {
                        if let Some(obj) = val.as_object() {
                            obj.get("value")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        } else {
                            val.as_str().map(|s| s.to_string())
                        }
                    }
                    None => None,
                })
                .ok_or("Anthropic API key is not set")?;

            let client = reqwest::Client::new();
            let res = client
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
                .await
                .map_err(|e| format!("Request failed: {}", e))?;

            let json: serde_json::Value = res
                .json()
                .await
                .map_err(|e| format!("Failed to parse JSON: {}", e))?;

            let mut models = vec![];
            if let Some(data) = json.get("data").and_then(|v| v.as_array()) {
                for m in data {
                    if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
                        models.push(id.to_string());
                    }
                }
            } else if json.get("type").and_then(|v| v.as_str()) == Some("error") {
                if let Some(msg) = json
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                {
                    return Err(format!("Anthropic API error: {}", msg));
                }
            }

            Ok(models)
        }
    }
}

#[tauri::command]
pub async fn check_api_key_status(app: tauri::AppHandle) -> Result<Vec<ApiKeyStatus>, String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;

    Ok(vec![
        ApiKeyStatus {
            name: "anthropic".to_string(),
            display_name: "Anthropic".to_string(),
            configured: has_configured_store_value(store.get("anthropic-api-key")),
        },
        ApiKeyStatus {
            name: "gemini".to_string(),
            display_name: "Gemini".to_string(),
            configured: has_configured_store_value(store.get("gemini-api-key")),
        },
        ApiKeyStatus {
            name: "openai".to_string(),
            display_name: "OpenAI".to_string(),
            configured: has_configured_store_value(store.get("openai-api-key")),
        },
    ])
}

#[tauri::command]
pub async fn check_ollama_status(
    app: tauri::AppHandle,
    endpoint_override: Option<String>,
) -> Result<OllamaStatus, String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;
    let endpoint = endpoint_override
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            store
                .get("ollama-endpoint")
                .and_then(extract_store_string_value)
        })
        .unwrap_or_else(|| DEFAULT_OLLAMA_ENDPOINT.to_string());

    Ok(fetch_ollama_status(&endpoint).await)
}

#[cfg(test)]
mod tests {
    use super::{
        build_ollama_openai_base_url, build_ollama_tags_url, extract_store_string_value,
        gemini_retry_delay, has_configured_store_value, is_retryable_gemini_error, AiProvider,
    };
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn extract_store_string_value_reads_wrapped_value() {
        let result = extract_store_string_value(json!({ "value": "secret" }));
        assert_eq!(result.as_deref(), Some("secret"));
    }

    #[test]
    fn has_configured_store_value_rejects_blank_values() {
        assert!(!has_configured_store_value(Some(json!({ "value": "   " }))));
        assert!(!has_configured_store_value(Some(json!(""))));
        assert!(has_configured_store_value(Some(json!("configured"))));
    }

    #[test]
    fn ai_provider_from_str_supports_openai_and_ollama() {
        assert_eq!(AiProvider::from_str("openai"), AiProvider::OpenAI);
        assert_eq!(AiProvider::from_str("ollama"), AiProvider::Ollama);
    }

    #[test]
    fn ollama_urls_normalize_root_and_v1_paths() {
        assert_eq!(
            build_ollama_openai_base_url("http://localhost:11434"),
            "http://localhost:11434/v1"
        );
        assert_eq!(
            build_ollama_openai_base_url("http://localhost:11434/v1"),
            "http://localhost:11434/v1"
        );
        assert_eq!(
            build_ollama_tags_url("http://localhost:11434/v1"),
            "http://localhost:11434/api/tags"
        );
    }

    #[test]
    fn retryable_gemini_error_detects_unavailable_variants() {
        assert!(is_retryable_gemini_error("503 Service Unavailable"));
        assert!(is_retryable_gemini_error(
            "{\"error\":{\"status\":\"UNAVAILABLE\",\"message\":\"high demand\"}}"
        ));
        assert!(is_retryable_gemini_error("Gemini overloaded right now"));
        assert!(!is_retryable_gemini_error("Gemini error: invalid api key"));
    }

    #[test]
    fn gemini_retry_delay_uses_exponential_backoff() {
        assert_eq!(gemini_retry_delay(0), Duration::from_secs(2));
        assert_eq!(gemini_retry_delay(1), Duration::from_secs(4));
        assert_eq!(gemini_retry_delay(2), Duration::from_secs(8));
    }
}
