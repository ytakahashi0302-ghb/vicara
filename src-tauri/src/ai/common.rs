use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::Write;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedTask {
    pub title: String,
    pub description: String,
    pub priority: Option<i32>,
    pub blocked_by_indices: Option<Vec<usize>>,
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
    pub patch_target: Option<String>,
    pub patch_content: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTaskResponse {
    pub reply: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct PoAction {
    pub(crate) action: String,
    pub(crate) payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct PoAssistantExecutionPlan {
    pub(crate) reply: Option<String>,
    #[serde(default)]
    pub(crate) operations: Vec<crate::ai_tools::CreateStoryAndTasksArgs>,
    #[serde(default)]
    pub(crate) actions: Vec<PoAction>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProjectBacklogCounts {
    pub(crate) stories: i64,
    pub(crate) tasks: i64,
    pub(crate) dependencies: i64,
}

const PO_ASSISTANT_TRANSPORT_KEY: &str = "po-assistant-transport";
const PO_ASSISTANT_CLI_TYPE_KEY: &str = "po-assistant-cli-type";
const PO_ASSISTANT_CLI_MODEL_KEY: &str = "po-assistant-cli-model";
const CLI_OUTPUT_TAIL_MAX_CHARS: usize = 2048;

#[derive(Debug, Clone)]
pub(crate) enum PoTransport {
    Api {
        provider: crate::rig_provider::AiProvider,
        api_key: String,
        model: String,
    },
    Cli {
        cli_type: crate::cli_runner::CliType,
        model: String,
        cwd: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct CliExecutionMetadata {
    pub(crate) model: String,
    pub(crate) request_started_at: i64,
    pub(crate) request_completed_at: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct CliExecutionResult<T> {
    pub(crate) value: T,
    pub(crate) metadata: CliExecutionMetadata,
}

pub(crate) fn current_timestamp_millis() -> Result<i64, String> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
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

pub(crate) fn build_cli_not_found_message(runner: &dyn crate::cli_runner::CliRunner) -> String {
    format!(
        "{} ({}) が見つかりません。`{}` でインストールし、PATH に追加してください。",
        runner.display_name(),
        runner.command_name(),
        runner.install_hint()
    )
}

pub(crate) async fn resolve_project_cli_cwd(
    app: &AppHandle,
    project_id: &str,
) -> Result<String, String> {
    let mut projects = crate::db::select_query::<crate::db::Project>(
        app,
        "SELECT * FROM projects WHERE id = ? LIMIT 1",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;

    let project = projects
        .pop()
        .ok_or_else(|| format!("プロジェクトが見つかりません: {}", project_id))?;

    let local_path = project
        .local_path
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .ok_or_else(|| {
            "CLI モードで PO アシスタントを使うには、プロジェクトの Local Path 設定が必要です。"
                .to_string()
        })?;

    let path = std::path::Path::new(&local_path);
    if !path.exists() {
        return Err(format!(
            "CLI 実行ディレクトリが存在しません: {}",
            local_path
        ));
    }
    if !path.is_dir() {
        return Err(format!(
            "CLI 実行ディレクトリではありません: {}",
            local_path
        ));
    }

    Ok(local_path)
}

fn format_cli_args_for_error(args: &[String]) -> String {
    if args.is_empty() {
        return "(none)".to_string();
    }

    args.iter()
        .map(|arg| {
            if arg.chars().any(char::is_whitespace) {
                format!("{arg:?}")
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_cli_exit_code(status: &std::process::ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn truncate_output_tail(output: &str, max_chars: usize) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }

    let total_chars = trimmed.chars().count();
    if total_chars <= max_chars {
        return Some(trimmed.to_string());
    }

    let tail = trimmed
        .chars()
        .skip(total_chars.saturating_sub(max_chars))
        .collect::<String>();
    Some(format!("...(末尾 {max_chars} 文字)\n{tail}"))
}

pub(crate) fn build_gemini_trust_hint(
    cli_type: &crate::cli_runner::CliType,
    stderr: &str,
    stdout: &str,
) -> Option<&'static str> {
    if *cli_type != crate::cli_runner::CliType::Gemini {
        return None;
    }

    let normalized = format!("{stderr}\n{stdout}").to_ascii_lowercase();
    if normalized.contains("trust")
        || normalized.contains("trusted folder")
        || normalized.contains("trustedfolders.json")
    {
        Some("対象プロジェクトを `~/.gemini/trustedFolders.json` に追加してください。")
    } else {
        None
    }
}

fn build_cli_execution_context(cwd: &str, args: &[String]) -> String {
    format!("cwd: {cwd}\nargs: {}", format_cli_args_for_error(args))
}

fn create_cli_response_capture_path(
    cli_type: &crate::cli_runner::CliType,
    cwd: &str,
) -> std::path::PathBuf {
    std::path::Path::new(cwd).join(format!(
        "vicara-po-{}-{}.txt",
        cli_type.as_str(),
        uuid::Uuid::new_v4()
    ))
}

fn build_cli_timeout_error(
    display_name: &str,
    timeout_secs: u64,
    cwd: &str,
    args: &[String],
) -> String {
    format!(
        "{display_name} の実行が {timeout_secs} 秒でタイムアウトしました。\n{}",
        build_cli_execution_context(cwd, args)
    )
}

fn build_cli_nonzero_exit_error(
    cli_type: &crate::cli_runner::CliType,
    display_name: &str,
    status: &std::process::ExitStatus,
    cwd: &str,
    args: &[String],
    stderr: &str,
    stdout: &str,
) -> String {
    let mut lines = vec![
        format!("{display_name} がエラーで終了しました。"),
        format!("exit code: {}", format_cli_exit_code(status)),
        build_cli_execution_context(cwd, args),
    ];

    if let Some(stderr_tail) = truncate_output_tail(stderr, CLI_OUTPUT_TAIL_MAX_CHARS) {
        lines.push(format!("stderr:\n{stderr_tail}"));
    }

    if stderr.trim().is_empty() {
        if let Some(stdout_tail) = truncate_output_tail(stdout, CLI_OUTPUT_TAIL_MAX_CHARS) {
            lines.push(format!("stdout:\n{stdout_tail}"));
        }
    }

    if let Some(hint) = build_gemini_trust_hint(cli_type, stderr, stdout) {
        lines.push(hint.to_string());
    }

    lines.join("\n")
}

fn build_cli_json_parse_error(
    cli_type: &crate::cli_runner::CliType,
    display_name: &str,
    parse_error: &str,
    cwd: &str,
    args: &[String],
    stderr: &str,
    stdout: &str,
) -> String {
    let mut lines = vec![
        format!("{display_name} の出力から有効な JSON を抽出できませんでした: {parse_error}"),
        build_cli_execution_context(cwd, args),
    ];

    if let Some(stderr_tail) = truncate_output_tail(stderr, CLI_OUTPUT_TAIL_MAX_CHARS) {
        lines.push(format!("stderr:\n{stderr_tail}"));
    }

    if let Some(stdout_tail) = truncate_output_tail(stdout, CLI_OUTPUT_TAIL_MAX_CHARS) {
        lines.push(format!("stdout:\n{stdout_tail}"));
    }

    if let Some(hint) = build_gemini_trust_hint(cli_type, stderr, stdout) {
        lines.push(hint.to_string());
    }

    lines.join("\n")
}

pub(crate) async fn resolve_po_transport(
    app: &AppHandle,
    project_id: &str,
    provider_override: Option<String>,
) -> Result<PoTransport, String> {
    let store = app
        .store("settings.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;

    let transport_kind = store
        .get(PO_ASSISTANT_TRANSPORT_KEY)
        .and_then(extract_store_string_value)
        .unwrap_or_else(|| "api".to_string());

    if transport_kind.trim().eq_ignore_ascii_case("cli") {
        let cli_type = crate::cli_runner::CliType::from_str(
            &store
                .get(PO_ASSISTANT_CLI_TYPE_KEY)
                .and_then(extract_store_string_value)
                .unwrap_or_else(|| "claude".to_string()),
        );
        let runner = crate::cli_runner::create_runner(&cli_type)?;
        let model = runner.resolve_model(
            &store
                .get(PO_ASSISTANT_CLI_MODEL_KEY)
                .and_then(extract_store_string_value)
                .unwrap_or_default(),
        );
        let cwd = resolve_project_cli_cwd(app, project_id).await?;

        Ok(PoTransport::Cli {
            cli_type,
            model,
            cwd,
        })
    } else {
        let (provider, api_key, model) =
            crate::rig_provider::resolve_provider_and_key(app, provider_override).await?;

        Ok(PoTransport::Api {
            provider,
            api_key,
            model,
        })
    }
}

pub(crate) async fn execute_po_cli_prompt<T>(
    cli_type: &crate::cli_runner::CliType,
    model: &str,
    prompt: &str,
    cwd: &str,
) -> Result<CliExecutionResult<T>, String>
where
    T: DeserializeOwned,
{
    let runner = crate::cli_runner::create_runner(cli_type)?;
    let detected_command_path =
        crate::cli_detection::resolve_cli_command_path(runner.command_name())
            .ok_or_else(|| build_cli_not_found_message(runner.as_ref()))?;
    let resolved_model = runner.resolve_model(model);
    let mut base_args = runner.build_args(prompt, &resolved_model, cwd);
    let response_capture_path = if runner.prefers_response_capture_file() {
        let capture_path = create_cli_response_capture_path(cli_type, cwd);
        runner.prepare_response_capture(&mut base_args, &capture_path)?;
        Some(capture_path)
    } else {
        None
    };
    let (cli_command_path, args) = runner.prepare_invocation(&detected_command_path, base_args)?;
    let stdin_payload = runner.stdin_payload(prompt);
    let env_vars = runner.env_vars();
    let timeout_secs = runner.timeout_secs();
    let display_name = runner.display_name().to_string();
    let cli_not_found_message = build_cli_not_found_message(runner.as_ref());
    let cli_type = *cli_type;
    let cwd = cwd.to_string();
    let args_for_error = args.clone();
    let cwd_for_error = cwd.clone();
    let args_for_exec = args.clone();
    let cwd_for_exec = cwd.clone();

    let request_started_at = current_timestamp_millis()?;
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tauri::async_runtime::spawn_blocking(move || {
            let mut command = std::process::Command::new(&cli_command_path);
            command.args(&args_for_exec).current_dir(&cwd_for_exec);
            for (key, value) in env_vars {
                command.env(key, value);
            }
            if let Some(stdin_payload) = stdin_payload {
                command.stdin(std::process::Stdio::piped());
                let mut child = command.spawn()?;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(stdin_payload.as_bytes())?;
                }
                child.wait_with_output()
            } else {
                command.output()
            }
        }),
    )
    .await
    .map_err(|_| {
        build_cli_timeout_error(&display_name, timeout_secs, &cwd_for_error, &args_for_error)
    })?
    .map_err(|error| {
        format!(
            "{} の実行スレッドが失敗しました: {}\n{}",
            display_name,
            error,
            build_cli_execution_context(&cwd_for_error, &args_for_error)
        )
    })?
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "{}\n{}",
                cli_not_found_message,
                build_cli_execution_context(&cwd_for_error, &args_for_error)
            )
        } else {
            format!(
                "{} の実行に失敗しました: {}\n{}",
                display_name,
                error,
                build_cli_execution_context(&cwd_for_error, &args_for_error)
            )
        }
    })?;
    let request_completed_at = current_timestamp_millis().unwrap_or(request_started_at);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        if let Some(capture_path) = &response_capture_path {
            let _ = std::fs::remove_file(capture_path);
        }
        return Err(build_cli_nonzero_exit_error(
            &cli_type,
            &display_name,
            &output.status,
            &cwd,
            &args,
            &stderr,
            &stdout,
        ));
    }

    let response_content = if let Some(capture_path) = &response_capture_path {
        let content = std::fs::read_to_string(capture_path).map_err(|error| {
            format!(
                "{} の最終メッセージファイルを読み取れませんでした: {}\n{}",
                display_name,
                error,
                build_cli_execution_context(&cwd, &args)
            )
        })?;
        let _ = std::fs::remove_file(capture_path);
        content
    } else {
        stdout.clone()
    };

    let value = parse_json_response::<T>(&response_content).map_err(|error| {
        build_cli_json_parse_error(
            &cli_type,
            &display_name,
            &error,
            &cwd,
            &args,
            &stderr,
            &response_content,
        )
    })?;

    Ok(CliExecutionResult {
        value,
        metadata: CliExecutionMetadata {
            model: resolved_model,
            request_started_at,
            request_completed_at,
        },
    })
}

fn extract_json_candidates(input: &str) -> Vec<&str> {
    let mut candidates = Vec::new();

    for (start, opener) in input.char_indices() {
        if opener != '{' && opener != '[' {
            continue;
        }

        let mut stack = vec![opener];
        let mut in_string = false;
        let mut escaped = false;
        let slice = &input[start + opener.len_utf8()..];

        for (offset, ch) in slice.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' | '[' => stack.push(ch),
                '}' => {
                    if stack.last() == Some(&'{') {
                        stack.pop();
                    } else {
                        break;
                    }
                }
                ']' => {
                    if stack.last() == Some(&'[') {
                        stack.pop();
                    } else {
                        break;
                    }
                }
                _ => {}
            }

            if stack.is_empty() {
                let end = start + opener.len_utf8() + offset + ch.len_utf8();
                candidates.push(&input[start..end]);
                break;
            }
        }
    }

    candidates
}

pub(crate) fn parse_json_response<T>(content: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let trimmed = content.trim();

    if let Ok(parsed) = serde_json::from_str::<T>(trimmed) {
        return Ok(parsed);
    }

    let mut last_error: Option<String> = None;

    for candidate in extract_json_candidates(trimmed) {
        match serde_json::from_str::<T>(candidate) {
            Ok(parsed) => return Ok(parsed),
            Err(error) => last_error = Some(error.to_string()),
        }
    }

    Err(last_error.unwrap_or_else(|| "レスポンスから有効なJSONを抽出できませんでした".to_string()))
}

pub(crate) async fn get_project_backlog_counts(
    app: &AppHandle,
    project_id: &str,
) -> Result<ProjectBacklogCounts, String> {
    let stories = crate::db::select_query::<(i64,)>(
        app,
        "SELECT COUNT(*) as count FROM stories WHERE project_id = ?",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?
    .first()
    .map(|row| row.0)
    .unwrap_or(0);

    let tasks = crate::db::select_query::<(i64,)>(
        app,
        "SELECT COUNT(*) as count FROM tasks WHERE project_id = ?",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?
    .first()
    .map(|row| row.0)
    .unwrap_or(0);

    let dependencies = crate::db::select_query::<(i64,)>(
        app,
        "SELECT COUNT(*) as count FROM task_dependencies td JOIN tasks t ON td.task_id = t.id WHERE t.project_id = ?",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?
    .first()
    .map(|row| row.0)
    .unwrap_or(0);

    Ok(ProjectBacklogCounts {
        stories,
        tasks,
        dependencies,
    })
}

pub(crate) async fn record_provider_usage(
    app: &AppHandle,
    project_id: &str,
    source_kind: &str,
    response: &crate::rig_provider::LlmTextResponse,
) {
    if let Err(error) = crate::llm_observability::record_llm_usage(
        app,
        crate::llm_observability::RecordLlmUsageInput {
            project_id: project_id.to_string(),
            task_id: None,
            sprint_id: None,
            source_kind: source_kind.to_string(),
            transport_kind: "provider_api".to_string(),
            provider: response.provider.clone(),
            model: response.model.clone(),
            usage: response.usage,
            measurement_status: None,
            request_started_at: Some(response.started_at),
            request_completed_at: Some(response.completed_at),
            success: true,
            error_message: None,
            raw_usage_json: Some(response.raw_usage_json.clone()),
        },
    )
    .await
    {
        log::warn!(
            "Failed to record LLM usage for source_kind={} project_id={}: {}",
            source_kind,
            project_id,
            error
        );
    }
}

pub(crate) async fn record_cli_usage(
    app: &AppHandle,
    project_id: &str,
    source_kind: &str,
    cli_type: &crate::cli_runner::CliType,
    metadata: &CliExecutionMetadata,
) {
    if let Err(error) = crate::llm_observability::record_cli_usage(
        app,
        crate::llm_observability::CliUsageRecordInput {
            project_id: Some(project_id.to_string()),
            task_id: None,
            sprint_id: None,
            source_kind: source_kind.to_string(),
            cli_type: cli_type.as_str().to_string(),
            model: metadata.model.clone(),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            cached_input_tokens: None,
            request_started_at: metadata.request_started_at,
            request_completed_at: metadata.request_completed_at,
            success: true,
            error_message: None,
        },
    )
    .await
    {
        log::warn!(
            "Failed to record CLI usage for source_kind={} project_id={}: {}",
            source_kind,
            project_id,
            error
        );
    }
}

pub(crate) fn serialize_chat_history(messages: &[Message]) -> String {
    messages
        .iter()
        .map(|message| {
            let heading = match message.role.as_str() {
                "assistant" => "## アシスタント",
                "system" => "## システム",
                _ => "## ユーザー",
            };
            format!("{}\n{}", heading, message.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::{build_gemini_trust_hint, truncate_output_tail};
    use crate::cli_runner::CliType;

    #[test]
    fn truncate_output_tail_keeps_only_requested_suffix() {
        let output = truncate_output_tail("abcdef", 4).expect("tail should exist");

        assert!(output.contains("末尾 4 文字"));
        assert!(output.ends_with("cdef"));
    }

    #[test]
    fn gemini_trust_hint_is_only_returned_for_trust_related_errors() {
        let hint =
            build_gemini_trust_hint(&CliType::Gemini, "Project is not in a trusted folder.", "");

        assert_eq!(
            hint,
            Some("対象プロジェクトを `~/.gemini/trustedFolders.json` に追加してください。")
        );
        assert_eq!(
            build_gemini_trust_hint(&CliType::Gemini, "plain stderr", "plain stdout"),
            None
        );
        assert_eq!(
            build_gemini_trust_hint(&CliType::Claude, "Project is not in a trusted folder.", ""),
            None
        );
    }
}
