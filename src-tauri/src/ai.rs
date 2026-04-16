use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::Write;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
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

// generated_document を廃止し patch_target + patch_content 方式に移行
// フロントエンドは patch_target に指定されたファイルへ patch_content を書き込む
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatInceptionResponse {
    pub reply: String,
    pub is_finished: bool,
    pub patch_target: Option<String>, // 書き込み先ファイル名 (e.g. "PRODUCT_CONTEXT.md")
    pub patch_content: Option<String>, // 書き込む内容（そのフェーズの差分のみ）
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTaskResponse {
    pub reply: String,
}

/// CLI マルチアクション用 action ペイロード
///   action: "create_story" | "add_note" | "suggest_retro"
#[derive(Debug, Serialize, Deserialize, Clone)]
struct PoAction {
    pub action: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PoAssistantExecutionPlan {
    pub reply: Option<String>,
    /// 旧フォーマット（後方互換のために維持）
    #[serde(default)]
    pub operations: Vec<crate::ai_tools::CreateStoryAndTasksArgs>,
    /// 新マルチアクションフォーマット
    #[serde(default)]
    pub actions: Vec<PoAction>,
}

#[derive(Debug, Clone, Copy)]
struct ProjectBacklogCounts {
    stories: i64,
    tasks: i64,
    dependencies: i64,
}

const PO_ASSISTANT_TRANSPORT_KEY: &str = "po-assistant-transport";
const PO_ASSISTANT_CLI_TYPE_KEY: &str = "po-assistant-cli-type";
const PO_ASSISTANT_CLI_MODEL_KEY: &str = "po-assistant-cli-model";
const CLI_OUTPUT_TAIL_MAX_CHARS: usize = 2048;

#[derive(Debug, Clone)]
enum PoTransport {
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
struct CliExecutionMetadata {
    model: String,
    request_started_at: i64,
    request_completed_at: i64,
}

#[derive(Debug, Clone)]
struct CliExecutionResult<T> {
    value: T,
    metadata: CliExecutionMetadata,
}

fn current_timestamp_millis() -> Result<i64, String> {
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

fn build_cli_not_found_message(runner: &dyn crate::cli_runner::CliRunner) -> String {
    format!(
        "{} ({}) が見つかりません。`{}` でインストールし、PATH に追加してください。",
        runner.display_name(),
        runner.command_name(),
        runner.install_hint()
    )
}

async fn resolve_project_cli_cwd(app: &AppHandle, project_id: &str) -> Result<String, String> {
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

fn truncate_output_tail(output: &str, max_chars: usize) -> Option<String> {
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

fn build_gemini_trust_hint(
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

async fn resolve_po_transport(
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

async fn execute_po_cli_prompt<T>(
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

fn parse_json_response<T>(content: &str) -> Result<T, String>
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

async fn get_project_backlog_counts(
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

async fn record_provider_usage(
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

async fn record_cli_usage(
    app: &AppHandle,
    project_id: &str,
    source_kind: &str,
    cli_type: &crate::cli_runner::CliType,
    metadata: &CliExecutionMetadata,
) {
    if let Err(error) = crate::llm_observability::record_cli_usage(
        app,
        crate::llm_observability::ClaudeCliUsageRecordInput {
            project_id: Some(project_id.to_string()),
            task_id: None,
            sprint_id: None,
            source_kind: source_kind.to_string(),
            cli_type: cli_type.as_str().to_string(),
            model: metadata.model.clone(),
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

fn serialize_chat_history(messages: &[Message]) -> String {
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

fn looks_like_backlog_mutation_request(message: &str) -> bool {
    let normalized = message.to_lowercase();
    let has_action = [
        "追加", "作成", "登録", "生成", "append", "create", "add", "register",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));
    let has_target = [
        "バックログ",
        "ストーリー",
        "story",
        "stories",
        "タスク",
        "task",
        "tasks",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));

    has_action && has_target
}

fn looks_like_generic_backlog_creation_request(message: &str) -> bool {
    let normalized = message.to_lowercase();
    let mentions_story_scope = ["バックログ", "ストーリー", "story", "stories", "backlog"]
        .iter()
        .any(|keyword| normalized.contains(keyword));
    let mentions_existing_target = [
        "既存",
        "このストーリー",
        "そのストーリー",
        "story id",
        "target_story_id",
        "id:",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));
    let asks_task_only = normalized.contains("タスク")
        && !normalized.contains("バックログ")
        && !normalized.contains("ストーリー")
        && !normalized.contains("story");

    looks_like_backlog_mutation_request(message)
        && mentions_story_scope
        && !mentions_existing_target
        && !asks_task_only
}

fn has_product_context_document(context_md: &str) -> bool {
    context_md.contains("--- PRODUCT_CONTEXT.md ---")
}

fn build_missing_product_context_reply() -> String {
    "PRODUCT_CONTEXT.md を含むプロジェクト文脈を取得できないため、コンテキスト起点のバックログ生成は実行できません。プロジェクトの Local Path 設定と対象フォルダを確認してください。".to_string()
}

fn build_contextual_backlog_generation_system_prompt(context_md: &str) -> String {
    format!(
        "あなたはバックログ登録計画を JSON で返すプランナーです。ユーザー依頼が『バックログを1つ作成してください』のように抽象的でも、context 内の PRODUCT_CONTEXT.md / ARCHITECTURE.md / Rule.md と既存バックログを読み取り、次に取り組む価値が高く、既存バックログと重複しない具体的なバックログ項目を 1 件だけ提案してください。\n\nルール:\n- `story_title` `story_description` `acceptance_criteria` `tasks[*].title` `tasks[*].description` は必ずプロダクト固有の語彙を使う\n- 「新しいバックログ項目」「要求詳細を整理する」などの汎用プレースホルダは禁止\n- `PRODUCT_CONTEXT.md` の課題、対象ユーザー、目標、主流入力、Not To Do を優先して具体案を選ぶ\n- `ARCHITECTURE.md` の技術制約と矛盾させない\n- 新規バックログを 1 件作る前提で `target_story_id` は null にする\n- `tasks` は必ず 1 件以上含める\n- 各 task には `title`, `description`, `priority`, `blocked_by_indices` を入れる\n- priority は整数 1〜5\n- 実行不要と判断して空配列にせず、必ず 1 件の具体案を返す\n- 出力は必ず JSON オブジェクトのみ\n\n返却形式:\n{{\"reply\":\"ユーザー向け要約\",\"operations\":[{{\"target_story_id\":null,\"story_title\":\"...\",\"story_description\":\"...\",\"acceptance_criteria\":\"...\",\"story_priority\":3,\"tasks\":[{{\"title\":\"...\",\"description\":\"...\",\"priority\":2,\"blocked_by_indices\":[]}}]}}]}}\n\n【既存ドキュメントとバックログ】\n{}",
        context_md
    )
}

fn backlog_counts_changed(before: ProjectBacklogCounts, after: ProjectBacklogCounts) -> bool {
    before.stories != after.stories
        || before.tasks != after.tasks
        || before.dependencies != after.dependencies
}

async fn get_changed_backlog_counts_with_retry(
    app: &AppHandle,
    project_id: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ProjectBacklogCounts>, String> {
    let after_counts = get_project_backlog_counts(app, project_id).await?;
    if backlog_counts_changed(before_counts, after_counts) {
        return Ok(Some(after_counts));
    }

    tokio::time::sleep(Duration::from_millis(200)).await;
    let retry_counts = get_project_backlog_counts(app, project_id).await?;
    if backlog_counts_changed(before_counts, retry_counts) {
        return Ok(Some(retry_counts));
    }

    Ok(None)
}

async fn detect_backlog_change_with_retry(
    app: &AppHandle,
    project_id: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<bool, String> {
    Ok(
        get_changed_backlog_counts_with_retry(app, project_id, before_counts)
            .await?
            .is_some(),
    )
}

fn build_backlog_counts_reply(
    reply_prefix: String,
    before_counts: ProjectBacklogCounts,
    after_counts: ProjectBacklogCounts,
) -> Option<ChatTaskResponse> {
    let added_stories = after_counts.stories.saturating_sub(before_counts.stories);
    let added_tasks = after_counts.tasks.saturating_sub(before_counts.tasks);
    let added_dependencies = after_counts
        .dependencies
        .saturating_sub(before_counts.dependencies);

    if added_stories == 0 && added_tasks == 0 && added_dependencies == 0 {
        return None;
    }

    Some(ChatTaskResponse {
        reply: format!(
            "{}\n\n追加結果: stories +{}, tasks +{}, dependencies +{}",
            reply_prefix, added_stories, added_tasks, added_dependencies
        ),
    })
}

async fn build_partial_team_leader_success_response(
    app: &AppHandle,
    project_id: &str,
    before_counts: ProjectBacklogCounts,
    provider_error: &str,
) -> Result<Option<ChatTaskResponse>, String> {
    let Some(after_counts) =
        get_changed_backlog_counts_with_retry(app, project_id, before_counts).await?
    else {
        return Ok(None);
    };

    let _ = app.emit("kanban-updated", ());
    let error_summary = summarize_provider_error(provider_error);

    Ok(build_backlog_counts_reply(
        format!(
            "バックログ更新は反映されましたが、最終のAI応答生成で一時的なエラーが発生しました。内容確認中に再送せず、そのまま追加結果を返します。\n原因: {}",
            error_summary
        ),
        before_counts,
        after_counts,
    ))
}

fn summarize_provider_error(provider_error: &str) -> &str {
    provider_error
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("LLM provider error")
}

fn is_transient_provider_unavailable(provider_error: &str) -> bool {
    let normalized = provider_error.to_ascii_lowercase();
    normalized.contains("503")
        && (normalized.contains("service unavailable")
            || normalized.contains("\"status\": \"unavailable\"")
            || normalized.contains("high demand")
            || normalized.contains("status\": \"unavailable\"")
            || normalized.contains("unavailable"))
}

fn build_team_leader_provider_unavailable_reply(
    provider_error: &str,
    mutation_requested: bool,
) -> ChatTaskResponse {
    let error_summary = summarize_provider_error(provider_error);
    let reply = if mutation_requested {
        format!(
            "AI プロバイダーが一時的に高負荷のため、今回はバックログを作成していません。少し待って再試行するか、CLI もしくは別プロバイダーへ切り替えてください。\n原因: {}",
            error_summary
        )
    } else {
        format!(
            "AI プロバイダーが一時的に高負荷のため、今回は応答を返せませんでした。少し待って再試行するか、CLI もしくは別プロバイダーへ切り替えてください。\n原因: {}",
            error_summary
        )
    };

    ChatTaskResponse { reply }
}

async fn chat_team_leader_with_tools_with_retry(
    app: &AppHandle,
    provider: &crate::rig_provider::AiProvider,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    prior_messages: &[Message],
    project_id: &str,
) -> Result<crate::rig_provider::LlmTextResponse, String> {
    let chat_history = crate::rig_provider::convert_messages(prior_messages);
    crate::rig_provider::chat_team_leader_with_tools(
        app,
        provider,
        api_key,
        model,
        system_prompt,
        user_input,
        chat_history,
        project_id,
    )
    .await
}

fn parse_team_leader_execution_plan(content: &str) -> Result<PoAssistantExecutionPlan, String> {
    parse_json_response::<PoAssistantExecutionPlan>(content)
}

async fn apply_team_leader_execution_plan(
    app: &AppHandle,
    project_id: &str,
    plan: PoAssistantExecutionPlan,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ChatTaskResponse>, String> {
    let PoAssistantExecutionPlan {
        reply,
        operations,
        actions,
    } = plan;

    // ── 新フォーマット: actions 配列のルーティング処理 ──────────────────────
    let mut action_results: Vec<String> = Vec::new();
    for action in actions {
        match action.action.as_str() {
            "create_story" => {
                let args: crate::ai_tools::CreateStoryAndTasksArgs =
                    serde_json::from_value(action.payload).map_err(|e| {
                        format!("create_story payload のパースに失敗しました: {}", e)
                    })?;
                if args.tasks.is_empty() {
                    continue;
                }
                crate::ai_tools::guard_story_creation_against_duplicates(
                    app,
                    project_id,
                    args.target_story_id.as_deref(),
                    args.story_title.as_deref(),
                )
                .await?;
                let story_draft = crate::db::StoryDraftInput {
                    target_story_id: args.target_story_id.clone(),
                    title: args
                        .story_title
                        .clone()
                        .unwrap_or_else(|| "Untitled Story".to_string()),
                    description: args.story_description.clone(),
                    acceptance_criteria: args.acceptance_criteria.clone(),
                    priority: args.story_priority,
                };
                crate::db::insert_story_with_tasks(app, project_id, story_draft, args.tasks)
                    .await?;
                let _ = app.emit("kanban-updated", ());
                action_results.push("PBI・タスクを登録しました。".to_string());
            }
            "add_note" => {
                let args: crate::ai_tools::AddProjectNoteArgs =
                    serde_json::from_value(action.payload)
                        .map_err(|e| format!("add_note payload のパースに失敗しました: {}", e))?;
                crate::db::add_project_note(
                    app.clone(),
                    project_id.to_string(),
                    args.sprint_id,
                    args.title.clone(),
                    args.content,
                    Some("po_assistant".to_string()),
                )
                .await
                .map_err(|e| format!("ふせんの追加に失敗しました: {}", e))?;
                let _ = app.emit("kanban-updated", ());
                action_results.push(format!("ふせん「{}」をボードに追加しました。", args.title));
            }
            "suggest_retro" => {
                let args: crate::ai_tools::SuggestRetroItemArgs =
                    serde_json::from_value(action.payload).map_err(|e| {
                        format!("suggest_retro payload のパースに失敗しました: {}", e)
                    })?;
                let sessions =
                    crate::db::get_retro_sessions(app.clone(), project_id.to_string())
                        .await
                        .map_err(|e| format!("レトロセッションの取得に失敗しました: {}", e))?;
                let active = sessions
                    .iter()
                    .find(|s| s.status == "draft" || s.status == "in_progress");
                match active {
                    Some(session) => {
                        crate::db::add_retro_item(
                            app.clone(),
                            session.id.clone(),
                            args.category.clone(),
                            args.content.clone(),
                            "po".to_string(),
                            None,
                            None,
                        )
                        .await
                        .map_err(|e| format!("レトロアイテムの追加に失敗しました: {}", e))?;
                        let _ = app.emit("kanban-updated", ());
                        let label = match args.category.as_str() {
                            "keep" => "Keep",
                            "problem" => "Problem",
                            "try" => "Try",
                            _ => &args.category,
                        };
                        action_results.push(format!(
                            "レトロの {} に「{}」を追加しました。",
                            label, args.content
                        ));
                    }
                    None => {
                        action_results.push(
                            "アクティブなレトロセッションがないため、レトロアイテムの追加をスキップしました。".to_string(),
                        );
                    }
                }
            }
            unknown => {
                action_results.push(format!(
                    "不明なアクション種別「{}」はスキップしました。",
                    unknown
                ));
            }
        }
    }

    // actions が処理された場合は早期リターン
    if !action_results.is_empty() {
        let summary = if let Some(r) = reply {
            format!("{}\n\n{}", r, action_results.join("\n"))
        } else {
            action_results.join("\n")
        };
        return Ok(Some(ChatTaskResponse { reply: summary }));
    }

    // ── 旧フォーマット: operations 配列の処理（後方互換） ───────────────────
    if operations.is_empty() {
        return Ok(None);
    }

    for operation in operations {
        if operation.tasks.is_empty() {
            continue;
        }

        crate::ai_tools::guard_story_creation_against_duplicates(
            app,
            project_id,
            operation.target_story_id.as_deref(),
            operation.story_title.as_deref(),
        )
        .await?;

        let story_draft = crate::db::StoryDraftInput {
            target_story_id: operation.target_story_id.clone(),
            title: operation
                .story_title
                .clone()
                .unwrap_or_else(|| "Untitled Story".to_string()),
            description: operation.story_description.clone(),
            acceptance_criteria: operation.acceptance_criteria.clone(),
            priority: operation.story_priority,
        };

        crate::db::insert_story_with_tasks(app, project_id, story_draft, operation.tasks).await?;
    }

    let after_counts = get_project_backlog_counts(app, project_id).await?;
    let Some(response) = build_backlog_counts_reply(
        reply.unwrap_or_else(|| "バックログ登録を実行しました。".to_string()),
        before_counts,
        after_counts,
    ) else {
        return Ok(None);
    };

    let _ = app.emit("kanban-updated", ());
    Ok(Some(response))
}

async fn execute_fallback_team_leader_plan(
    app: &AppHandle,
    provider: &crate::rig_provider::AiProvider,
    api_key: &str,
    model: &str,
    project_id: &str,
    context_md: &str,
    user_request: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ChatTaskResponse>, String> {
    let fallback_system_prompt = if looks_like_generic_backlog_creation_request(user_request) {
        build_contextual_backlog_generation_system_prompt(context_md)
    } else {
        format!(
            "あなたはバックログ登録計画を JSON で返すプランナーです。ユーザー依頼に対して、実行すべき `create_story_and_tasks` 相当の操作を JSON のみで返してください。\n\nルール:\n- 既存ストーリーにタスクを追加する場合は、必ず context 内に存在する story ID を `target_story_id` に設定する\n- 新規ストーリーを作る場合のみ `target_story_id` を null にし、`story_title` を必須で入れる\n- `tasks` は必ず1件以上含める\n- 各 task には `title`, `description`, `priority`, `blocked_by_indices` を入れる\n- priority は整数 1〜5\n- 実行不要なら `operations` は空配列にする\n- 出力は必ず JSON オブジェクトのみ\n\n返却形式:\n{{\"reply\":\"ユーザー向け要約\",\"operations\":[{{\"target_story_id\":null,\"story_title\":\"...\",\"story_description\":\"...\",\"acceptance_criteria\":\"...\",\"story_priority\":3,\"tasks\":[{{\"title\":\"...\",\"description\":\"...\",\"priority\":2,\"blocked_by_indices\":[0]}}]}}]}}\n\n【既存バックログ】\n{}",
            context_md
        )
    };

    let raw_plan = crate::rig_provider::chat_with_history(
        provider,
        api_key,
        model,
        &fallback_system_prompt,
        user_request,
        vec![],
    )
    .await?;
    record_provider_usage(app, project_id, "team_leader", &raw_plan).await;

    let plan = match parse_team_leader_execution_plan(&raw_plan.content) {
        Ok(plan) => plan,
        Err(_) => return Ok(None),
    };

    apply_team_leader_execution_plan(app, project_id, plan, before_counts).await
}

async fn execute_contextual_cli_backlog_plan(
    app: &AppHandle,
    project_id: &str,
    cli_type: crate::cli_runner::CliType,
    model: &str,
    cwd: &str,
    context_md: &str,
    user_request: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ChatTaskResponse>, String> {
    let cli_prompt = format!(
        "{}\n\n【今回のユーザー依頼】\n{}",
        build_contextual_backlog_generation_system_prompt(context_md),
        user_request
    );
    let result =
        execute_po_cli_prompt::<PoAssistantExecutionPlan>(&cli_type, model, &cli_prompt, cwd)
            .await?;
    record_cli_usage(app, project_id, "team_leader", &cli_type, &result.metadata).await;

    apply_team_leader_execution_plan(app, project_id, result.value, before_counts).await
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
    let transport = resolve_po_transport(&app, &project_id, Some(provider)).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let prompt = format!(
        "Context: {}\nStory: {}\nDesc: {}\nAC: {}\nJSON Array Output Please.",
        &context_md, &title, &description, &acceptance_criteria
    );

    let system_prompt = r#"You are a task decomposition expert for agile software development.
Given a user story, generate a JSON array of subtasks. Each task object must include:
- "title": string (concise, action-oriented)
- "description": string (implementation details)
- "priority": integer 1-5 (REQUIRED; lower number = higher priority)
- "blocked_by_indices": number[] (zero-based indices of prerequisite tasks in this array; omit or use [] if none)

Priority guidelines (integer 1-5, lower = more urgent):
- 1: Most critical — architecture foundation, blocking everything else
- 2: High priority — core functionality on the critical path
- 3: Medium — important feature work, not blocking others (default)
- 4: Low — supporting tasks, tests, minor improvements
- 5: Lowest — documentation, polish, optional enhancements

Dependency guidelines:
- Use blocked_by_indices to express "this task cannot start until task N is done"
- Example: If task[2] requires the API from task[0], set task[2].blocked_by_indices = [0]
- Keep dependency chains short and avoid circular references

Output ONLY a valid JSON array.
Do not wrap the array in markdown code fences.
Do not include any explanation before or after the JSON."#;

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let response = crate::rig_provider::chat_with_history(
                &provider,
                &api_key,
                &model,
                system_prompt,
                &prompt,
                vec![],
            )
            .await?;
            record_provider_usage(&app, &project_id, "task_generation", &response).await;

            parse_json_response(&response.content)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let cli_prompt = format!(
                r#"{system_prompt}

【プロジェクトコンテキスト】
{context_md}

【対象ストーリー】
- title: {title}
- description: {description}
- acceptance_criteria: {acceptance_criteria}

有効な JSON 配列のみを返してください。
各要素は以下の形式に従ってください。
[
  {{
    "title": "タスク名",
    "description": "実装内容",
    "priority": 3,
    "blocked_by_indices": [0]
  }}
]"#
            );
            let result =
                execute_po_cli_prompt::<Vec<GeneratedTask>>(&cli_type, &model, &cli_prompt, &cwd)
                    .await?;
            record_cli_usage(
                &app,
                &project_id,
                "task_generation",
                &cli_type,
                &result.metadata,
            )
            .await;

            Ok(result.value)
        }
    }
}

#[tauri::command]
pub async fn refine_idea(
    app: AppHandle,
    idea_seed: String,
    previous_context: Option<Vec<Message>>,
    project_id: String,
) -> Result<RefinedIdeaResponse, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let previous_messages = previous_context.unwrap_or_default();
    let system_prompt = "PO Assist";

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let chat_history = crate::rig_provider::convert_messages(&previous_messages);
            let content = crate::rig_provider::chat_with_history(
                &provider,
                &api_key,
                &model,
                system_prompt,
                &idea_seed,
                chat_history,
            )
            .await?;
            record_provider_usage(&app, &project_id, "idea_refine", &content).await;

            parse_json_response(&content.content)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let history_block = if previous_messages.is_empty() {
                "（会話履歴なし）".to_string()
            } else {
                serialize_chat_history(&previous_messages)
            };
            let cli_prompt = format!(
                r#"{system_prompt}

あなたはプロダクトオーナー支援のアシスタントです。ユーザーのアイデアを整理し、実装前のユーザーストーリー草案に落とし込んでください。

【プロジェクトコンテキスト】
{context_md}

【これまでの会話】
{history_block}

【今回のユーザー入力】
{idea_seed}

以下の JSON オブジェクトのみを返してください。
{{
  "reply": "ユーザーへ返す短い整理メッセージ",
  "story_draft": {{
    "title": "ストーリータイトル",
    "description": "背景・価値・範囲が分かる説明",
    "acceptance_criteria": "受け入れ条件"
  }}
}}"#
            );
            let result =
                execute_po_cli_prompt::<RefinedIdeaResponse>(&cli_type, &model, &cli_prompt, &cwd)
                    .await?;
            record_cli_usage(
                &app,
                &project_id,
                "idea_refine",
                &cli_type,
                &result.metadata,
            )
            .await;

            Ok(result.value)
        }
    }
}

// ---------------------------------------------------------------------------
// Inception Deck システムプロンプト構築
// 各フェーズで「何をヒアリングし、どのファイルの差分を生成するか」を定義する
// ---------------------------------------------------------------------------
fn build_inception_system_prompt(phase: u32, context_md: &str) -> String {
    let phase_instruction = match phase {
        1 => {
            r#"## Phase 1: プロダクトの輪郭をつくる

**ヒアリング目標** (2〜4往復で整理する):
- このプロダクトは誰のためのものか
- その人がいま抱えている困りごとや不満は何か
- どんな解決策を提供し、使うと何が良くなるか
- 既存のやり方や競合と比べた違い・選ばれる理由は何か
- 上記を踏まえてエレベーターピッチの材料を揃える

**完了の目安**:
- ターゲット / 課題 / 解決策 / 価値・差別化のうち主要要素が 2〜3 個そろった時点で完了してよい
- AI が要約や候補を提示し、ユーザーが「それで十分」「大丈夫」「それでOK」など同意を示した時点で、追加質問をやめて完了する

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (新規作成)
**出力テンプレート** — Phase 1 の内容だけを、簡潔だが必要十分な粒度でまとめる:
```
# PRODUCT_CONTEXT.md — {プロダクト名}
> 【AIへの指示】本ファイルはプロダクト理解の土台として使う。

## 0. ひとことで言うと
- プロダクト名: {名前}
- 要約: {誰に何を届けるプロダクトか}

## 1. 課題と価値
- ターゲットユーザー: {誰}
- 困っていること: {課題}
- 解決策: {何を提供するか}
- 価値: {使うと何が良くなるか}

## 2. エレベーターピッチ
- ターゲット: {誰のためのものか}
- 課題: {どんな悩みを抱えているか}
- 解決策: {どんな方法で解決するか}
- 主要な価値: {なぜ使う価値があるか}
- 差別化ポイント: {既存手段との違い}

## 3. 役割分担
- 人間(PO): What と優先順位の意思決定
- AI: How の具体化と実行支援
```"#
        }

        2 => {
            r#"## Phase 2: やらないことリスト (Not List)

**ヒアリング目標** (2〜3往復):
- スコープ外にすること / 絶対やってはならないこと
- 【完了の目安】「やらないこと」が 2〜3 個挙がった時点、または提案にユーザーが同意した時点で深掘りをやめて完了する

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (末尾に追記)
**追記テンプレート** — Phase 2 の内容だけを追記する:
```
## 3. 運用ルール
- {スプリント方針を1行}

## 4. やらないこと (Not To Do)
- {項目1}
- {項目2}

## 5. コンテキスト管理
- Layer 1 (本ファイル + Rule.md): 不変のコア原則
- Layer 2 (handoff.md): スプリントごとの揮発性コンテキスト
```"#
        }

        3 => {
            r#"## Phase 3: どう動かしたいか・どんな環境で使いたいか

**ヒアリング目標** (2〜3往復):
- このアプリを主にどこで使いたいか（PCブラウザ / スマホ / タブレット など）
- 最初はローカル中心でよいか、早めにクラウドでも使いたいか
- データの扱いで大事にしたいこと（移行しやすさ / バックアップ / オフラインでも使いたい など）
- 通知や外部サービス連携など、動作上の希望や制約
- 【重要】PRODUCT_CONTEXT.md にすでに記載されている情報（利用者・用途・環境など）は絶対に再度質問しない。差分・詳細・未確認の項目のみを確認すること
- 【重要】ユーザーが技術名を答えられなくても進められるようにする。技術名やフレームワーク名は、ユーザーが自分から希望した場合だけ確認すればよい
- 【完了の目安】利用環境・運用方針・制約が 2〜3 項目まとまった時点、または AI の整理内容にユーザーが同意した時点で完了する

**生成ファイル**: patch_target = "ARCHITECTURE.md" (新規作成)
**出力テンプレート** — Phase 3 の内容だけを簡潔にまとめる。ユーザーが技術名を答えていない場合は、会話内容から妥当な構成を推定して埋めてよい:
```
# ARCHITECTURE.md — {プロダクト名}
> 技術水準と設計方針のまとめ

## 技術スタック
- 言語: {選定}
- FW: {選定}
- DB: {選定}

## アーキテクチャ方針
- {方針1}
- {方針2}

## 設計の制約
- {注意点}
```"#
        }

        4 => {
            r#"## Phase 4: 開発ルール・AIルール (How)

**ヒアリング目標** (1〜2往復):
- このプロダクト固有のコーディング規約 / AIへの特別指示
- 【完了の目安】固有ルールや AI 追加指示が 1〜3 個まとまった時点、またはユーザーが「その方針でよい」と同意した時点で完了する

**生成ファイル**: patch_target = "Rule.md" (末尾に追記)
**追記テンプレート** — 既存内容を再掲せず、Phase 4 の内容だけを追記する:
```
---
## {プロダクト名} 固有ルール

### 技術スタック固有の規約
- {規約1}

### AIへの追加指示
- {追加ルール1}
```"#
        }

        _ => "全フェーズ完了。ユーザーにお祝いの言葉を伝えてください。",
    };

    // 既存ドキュメントは先頭400文字のみを参考情報として渡す（転記禁止）
    let existing_docs = if context_md.is_empty() {
        "（生成済みドキュメントなし）".to_string()
    } else {
        let preview: String = context_md.chars().take(400).collect();
        let suffix = if context_md.chars().count() > 400 {
            "...(省略)"
        } else {
            ""
        };
        format!(
            "【既存ドキュメント概要（参考のみ・このフェーズ以外の内容を再出力しないこと）】\n{}{}",
            preview, suffix
        )
    };

    format!(
        r#"あなたは Vicara の「Scrum Product Partner」です。

## 役割
- ユーザーの曖昧なアイデアを整理し、プロダクトの価値と判断材料を言語化する
- スクラムやインセプションデッキの専門用語を前提にせず、平易な言葉で伴走する
- 情報が足りないときは、答えやすい具体的な質問や短い例を示して会話を前に進める

## 対話ルール
- コード・コマンド・実装手順の提案はしない
- 「どう作るか」よりも「誰のどんな課題をどう解くか」を明らかにする
- 一問一答に固執せず、短いガイド、言い換え、記入例を添えてよい
- ユーザーが迷っていそうなら、答え方の例を 1 つだけ示してよい
- ユーザーが技術者でない場合は、技術名そのものではなく利用シーン・制約・運用上の希望を先に聞く
- 他フェーズで生成済みのドキュメント内容を patch_content に含めない

## 応答方針
- 情報が足りない間は、`reply` に自然な案内と次の質問を書く
- `reply` は 1〜3 文程度でよく、必要なら短い補足や例を含めてよい
- 【重要】ヒアリングのループを防ぐため、目的の情報が規定数（各 Phase の「完了の目安」参照）集まった時点、またはユーザーが「それで十分」「大丈夫」「それでOK」「はい、それで問題ない」など同意を示した時点で、ただちに質問を打ち切り `is_finished: true` を返して完了する
- 同じ論点を言い換えて繰り返し聞かない。迷いがある場合は、新しい質問を増やす前に現在の理解を要約して確認する
- 完了条件を満たした場合は、`patch_target` と `patch_content` を返してドキュメント生成を行う
- `patch_content` は簡潔にまとめるが、必要な判断材料は削らない
- 既存ドキュメントは参考にするが、再出力やコピペはしない

{phase_instruction}

{existing_docs}

## 出力フォーマット（必ず JSON オブジェクトのみを返すこと）

ヒアリング中:
{{"reply": "次に聞きたいことや補足ガイド", "is_finished": false, "patch_target": null, "patch_content": null}}

ドキュメント生成時:
{{"reply": "まとめた内容を短く伝えるメッセージ", "is_finished": true, "patch_target": "ファイル名.md", "patch_content": "このフェーズで保存する Markdown"}}

patch_content にはこのフェーズで追加・更新する部分のみを含め、他フェーズの内容は含めないこと。"#,
        phase_instruction = phase_instruction,
        existing_docs = existing_docs,
    )
}

#[tauri::command]
pub async fn chat_inception(
    app: AppHandle,
    project_id: String,
    phase: u32,
    messages_history: Vec<Message>,
) -> Result<ChatInceptionResponse, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let system_prompt = build_inception_system_prompt(phase, &context_md);

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let chat_history = crate::rig_provider::convert_messages(&messages_history);
            let content = crate::rig_provider::chat_with_history(
                &provider,
                &api_key,
                &model,
                &system_prompt,
                "",
                chat_history,
            )
            .await?;
            record_provider_usage(&app, &project_id, "inception", &content).await;

            let resp: ChatInceptionResponse = match parse_json_response(&content.content) {
                Ok(r) => r,
                Err(_) => ChatInceptionResponse {
                    reply: content.content,
                    is_finished: false,
                    patch_target: None,
                    patch_content: None,
                },
            };

            Ok(resp)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let history_block = if messages_history.is_empty() {
                "（まだ会話履歴はありません）".to_string()
            } else {
                serialize_chat_history(&messages_history)
            };
            let cli_prompt = format!(
                r#"{system_prompt}

## 会話履歴
{history_block}

会話履歴を踏まえ、最後のユーザー発言に応答してください。
出力は必ず JSON オブジェクトのみで返してください。"#
            );
            let result = execute_po_cli_prompt::<ChatInceptionResponse>(
                &cli_type,
                &model,
                &cli_prompt,
                &cwd,
            )
            .await?;
            record_cli_usage(&app, &project_id, "inception", &cli_type, &result.metadata).await;

            Ok(result.value)
        }
    }
}

#[tauri::command]
pub async fn chat_with_team_leader(
    app: AppHandle,
    project_id: String,
    messages_history: Vec<Message>,
) -> Result<ChatTaskResponse, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let before_counts = get_project_backlog_counts(&app, &project_id).await?;
    let latest_user_index = messages_history
        .iter()
        .rposition(|message| message.role == "user");
    let (latest_user_message, prior_messages) = if let Some(index) = latest_user_index {
        let latest = messages_history[index].content.clone();
        let prior = messages_history[..index].to_vec();
        (latest, prior)
    } else {
        (String::new(), messages_history.clone())
    };
    let generic_backlog_request = looks_like_generic_backlog_creation_request(&latest_user_message);
    let has_product_context = has_product_context_document(&context_md);

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let mutation_requested = looks_like_backlog_mutation_request(&latest_user_message);
            let system_prompt = format!(
                "あなたは vicara の Scrum Team に所属する POアシスタントです。あなたの役割は、プロダクトオーナーの意思決定を支援しながら、要求の具体化、バックログの優先順位整理、追加タスクの登録を進めることです。ユーザーから機能要件や追加タスクの要望があった場合、自身が持つツール (`create_story_and_tasks`) を必ず呼び出して、PBI（プロダクトバックログアイテム）とサブタスク群をデータベースに自動登録してください。\n\n【用語ルール】\n- ユーザーへの返答では「ストーリー」ではなく必ず「PBI」と呼ぶこと\n- 例: 「PBI・タスクを登録しました」「既存PBIにタスクを追加しました」\n\n【最重要ルール】\n- `create_story_and_tasks` はユーザーが「PBIに追加して」「バックログに登録して」「タスクを作って」など**バックログ追加を明示的に依頼した場合のみ**使うこと\n- 「次のTRYとして〜」「レトロに追加して」「改善提案として〜」などレトロ・KPT関連の依頼では `create_story_and_tasks` を使わないこと\n- ユーザーが明示的に求めていないのに自己判断でPBIを作ることは禁止\n- 既存PBIにタスクを追加する依頼では、コンテキスト中の story ID を読んで `target_story_id` を必ず指定すること\n- 依頼が「バックログを1つ作って」のように抽象的でも、PRODUCT_CONTEXT.md / ARCHITECTURE.md / Rule.md と既存バックログからプロダクト固有の具体案を1件具体化して登録すること\n- 「新しいバックログ項目」「要求詳細を整理する」などのプレースホルダ名は禁止\n- ツールを呼んでいないのに「追加しました」「登録しました」と断定してはいけない\n- ツールが失敗した場合は、成功を装わずエラー内容を簡潔に伝えること\n\n【現在のプロダクトの状況（既存バックログ等）】\n{}\n\n【優先度と依存関係の設定ルール】\nPBIとタスクを作成する際は、必ず以下のフィールドを設定してください：\n- story_priority: 整数 1〜5（小さいほど優先度が高い）\n- 各タスクの priority: 整数 1〜5（小さいほど優先度が高い）\n- 各タスクの blocked_by_indices: 先行タスクの配列インデックス（0始まり）を指定。依存がなければ省略か空配列\n\n優先度の判断基準（1〜5、数値が小さいほど重要）:\n- 1: 最重要 — アーキテクチャの根幹、他の全タスクをブロックする基盤作業\n- 2: 高優先 — クリティカルパス上のコア機能\n- 3: 中優先 — 重要な機能実装だが他をブロックしない（デフォルト）\n- 4: 低優先 — サポートタスク、テスト、軽微な改善\n- 5: 最低優先 — ドキュメント、UIの微調整、オプション機能\n\n【重要】ツール実行に失敗した場合は、エラー内容を確認して原因をユーザーに報告、または代替策を考えてください。ツールが失敗したからといって、決してユーザーに手動での登録作業を丸投げしないでください。\n\n【レトロスペクティブ連携 — ふせん＆KPT提案】\n- 【最重要】ユーザーが「PBIに追加」「タスクを登録」など明示的にバックログ操作を求めた場合は `add_project_note` を絶対に呼ばないこと。その場合は `create_story_and_tasks` のみを使うこと。\n- `add_project_note`（ふせん）は、ユーザーが明示的に求めていない場面で会話から自然に浮かんだ気づき・懸念・メモを記録するためだけに使うこと。\n- プロセスの改善点、良かった点、問題点に気づいた場合は、`suggest_retro_item` ツールでレトロボードへKPTアイテムを積極的に提案してください。\n- カテゴリの判断基準:\n  - Keep: 継続すべき良い取り組みやプラクティス\n  - Problem: 解決すべき課題や障害\n  - Try: 次回試してみたい改善案\n- ツールの使用は明らかに有用な場合に限り、過剰な呼び出しは避けてください。\n- レトロセッションが存在しない場合にエラーが返ったら、ユーザーにレトロセッションの開始を案内してください。\n\n会話の返答は必ず以下の形式のJSONオブジェクトのみで返してください。\n\n{{\"reply\": \"ツール実行結果やユーザーへのメッセージ内容\"}}",
                context_md
            );

            let raw_text = match chat_team_leader_with_tools_with_retry(
                &app,
                &provider,
                &api_key,
                &model,
                &system_prompt,
                &latest_user_message,
                &prior_messages,
                &project_id,
            )
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    if mutation_requested {
                        if let Some(partial_success_response) =
                            build_partial_team_leader_success_response(
                                &app,
                                &project_id,
                                before_counts,
                                &error,
                            )
                            .await?
                        {
                            return Ok(partial_success_response);
                        }
                    }

                    if is_transient_provider_unavailable(&error) {
                        return Ok(build_team_leader_provider_unavailable_reply(
                            &error,
                            mutation_requested,
                        ));
                    }

                    return Err(error);
                }
            };
            record_provider_usage(&app, &project_id, "team_leader", &raw_text).await;
            let data_changed =
                detect_backlog_change_with_retry(&app, &project_id, before_counts).await?;

            if mutation_requested && !data_changed {
                if generic_backlog_request && !has_product_context {
                    return Ok(ChatTaskResponse {
                        reply: build_missing_product_context_reply(),
                    });
                }

                if let Some(fallback_response) = execute_fallback_team_leader_plan(
                    &app,
                    &provider,
                    &api_key,
                    &model,
                    &project_id,
                    &context_md,
                    &latest_user_message,
                    before_counts,
                )
                .await?
                {
                    return Ok(fallback_response);
                }

                return Ok(ChatTaskResponse {
                    reply: if generic_backlog_request {
                        "PRODUCT_CONTEXT.md を踏まえた具体的なバックログ案を生成できず、実際のバックログ件数変化も確認できませんでした。今回は成功扱いにせず停止します。プロジェクトの Local Path と PRODUCT_CONTEXT.md の内容を確認してから再試行してください。".to_string()
                    } else {
                        "登録・追加系の依頼として解釈しましたが、実際にはバックログの件数変化を確認できませんでした。今回は成功扱いにせず停止します。`create_story_and_tasks` の未実行または失敗が疑われるため、再試行時は対象ストーリーIDを明示して実行してください。".to_string()
                    },
                });
            }

            let resp: ChatTaskResponse = match parse_json_response(&raw_text.content) {
                Ok(r) => r,
                Err(_) => ChatTaskResponse {
                    reply: raw_text.content,
                },
            };

            Ok(resp)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let history_block = if prior_messages.is_empty() {
                "（会話履歴なし）".to_string()
            } else {
                serialize_chat_history(&prior_messages)
            };
            let cli_prompt = format!(
                r#"あなたは vicara の Scrum Team に所属する POアシスタントです。会話内容と既存バックログを踏まえ、必要なアクションを JSON で返してください。CLI ではアプリ側が JSON を解釈して DB 登録・ノート追加・レトロ追加を実行します。

【用語ルール】
- `reply` フィールドでユーザーに返す文章では「ストーリー」ではなく必ず「PBI」と表記すること

【アクション種別】
- `create_story` : バックログにPBI（プロダクトバックログアイテム）＆タスクを登録する
- `add_note`     : 会話中の気づきを「ふせん」としてボードに残す
- `suggest_retro`: レトロボードに KPT アイテムを提案する（keep / problem / try）

【create_story の使用条件 — 最重要】
- ユーザーが「PBIに追加して」「バックログに登録して」「タスクを作って」など、**バックログへの追加を明示的に依頼した場合のみ** `create_story` を使うこと
- 「次のTRYとして〜」「レトロに追加して」「ふせんに残して」「改善提案として〜」などレトロ・KPT・ふせん関連の依頼では `create_story` を絶対に使わないこと
- ユーザーが明示的に求めていないのに「便宜的にタスクも作っておこう」という自己判断での `create_story` 使用は禁止

【その他のルール】
- アクション不要なら `actions` は空配列にする
- `create_story` の場合: 既存PBIにタスクを追加するときは `target_story_id` を必ず指定し、新規なら null にして `story_title` を必須で入れる
- `create_story` の場合: 依頼が抽象的でも、PRODUCT_CONTEXT.md / ARCHITECTURE.md と既存バックログから具体案を1件生成する（プレースホルダ名禁止）
- `create_story` の場合: `tasks` は必ず 1 件以上、各タスクに `title`, `description`, `priority`, `blocked_by_indices` を含める
- `create_story` の場合: story_priority / task.priority は整数 1〜5
- `add_note` の場合: ユーザーが明示的にPBI/タスク作成を求めた場合は使わない。会話から自然に浮かんだ気づき・メモのみに使う。`sprint_id` は省略可
- `suggest_retro` の場合: Keep=継続したい良い点、Problem=課題、Try=改善提案。レトロセッション不在でも記録する（アプリ側でハンドリング）
- ユーザー向け説明は `reply` に簡潔に書く
- 出力は必ず JSON オブジェクトのみ

【既存バックログ】
{context_md}

【これまでの会話】
{history_block}

【今回のユーザー依頼】
{latest_user_message}

返却形式（複数アクションを同時に指定可能）:
{{
  "reply": "ユーザーへ返すメッセージ",
  "actions": [
    {{
      "action": "create_story",
      "payload": {{
        "target_story_id": null,
        "story_title": "PBI名",
        "story_description": "説明",
        "acceptance_criteria": "受け入れ条件",
        "story_priority": 3,
        "tasks": [
          {{
            "title": "タスク名",
            "description": "実装内容",
            "priority": 2,
            "blocked_by_indices": [0]
          }}
        ]
      }}
    }},
    {{
      "action": "add_note",
      "payload": {{
        "title": "ふせんのタイトル",
        "content": "内容（Markdown可）",
        "sprint_id": null
      }}
    }},
    {{
      "action": "suggest_retro",
      "payload": {{
        "category": "try",
        "content": "改善提案の内容"
      }}
    }}
  ]
}}"#
            );
            let result = execute_po_cli_prompt::<PoAssistantExecutionPlan>(
                &cli_type,
                &model,
                &cli_prompt,
                &cwd,
            )
            .await?;
            record_cli_usage(
                &app,
                &project_id,
                "team_leader",
                &cli_type,
                &result.metadata,
            )
            .await;

            let plan = result.value;
            if plan.operations.is_empty() && plan.actions.is_empty() {
                if generic_backlog_request {
                    if !has_product_context {
                        return Ok(ChatTaskResponse {
                            reply: build_missing_product_context_reply(),
                        });
                    }

                    if let Some(applied_response) = execute_contextual_cli_backlog_plan(
                        &app,
                        &project_id,
                        cli_type,
                        &model,
                        &cwd,
                        &context_md,
                        &latest_user_message,
                        before_counts,
                    )
                    .await?
                    {
                        return Ok(applied_response);
                    }
                }

                return Ok(ChatTaskResponse {
                    reply: plan
                        .reply
                        .unwrap_or_else(|| "判断材料を整理しました。".to_string()),
                });
            }

            if let Some(applied_response) =
                apply_team_leader_execution_plan(&app, &project_id, plan, before_counts).await?
            {
                return Ok(applied_response);
            }

            if generic_backlog_request {
                if !has_product_context {
                    return Ok(ChatTaskResponse {
                        reply: build_missing_product_context_reply(),
                    });
                }

                if let Some(applied_response) = execute_contextual_cli_backlog_plan(
                    &app,
                    &project_id,
                    cli_type,
                    &model,
                    &cwd,
                    &context_md,
                    &latest_user_message,
                    before_counts,
                )
                .await?
                {
                    return Ok(applied_response);
                }
            }

            Ok(ChatTaskResponse {
                reply: "登録・追加系の計画を受け取りましたが、実際にはバックログの件数変化を確認できませんでした。今回は成功扱いにせず停止します。対象ストーリーIDや生成タスク内容を見直して再試行してください。".to_string(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Epic 51: SM エージェント KPT 合成
// ---------------------------------------------------------------------------

const RETRO_REVIEW_SOURCE_KIND: &str = "retrospective";
const RETRO_MAX_RUNS_IN_PROMPT: usize = 10;
const RETRO_REASONING_TAIL_CHARS: usize = 1_500;
const RETRO_FINAL_ANSWER_HEAD_CHARS: usize = 2_000;
const RETRO_CHANGED_FILES_CHARS: usize = 1_000;
const RETRO_LOG_SECTION_CAP_CHARS: usize = 20_000;
const RETRO_SUMMARY_FALLBACK_MAX_CHARS: usize = 4_000;

#[derive(Debug, Clone, Deserialize)]
struct RetroReviewItem {
    category: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RetroSynthesisResponse {
    summary_markdown: String,
    #[serde(default)]
    items: Vec<RetroReviewItem>,
}

fn take_head_chars(input: &str, max_chars: usize) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let total = trimmed.chars().count();
    if total <= max_chars {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(max_chars).collect();
    format!("{head}…(先頭 {max_chars} 文字)")
}

fn take_tail_chars(input: &str, max_chars: usize) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let total = trimmed.chars().count();
    if total <= max_chars {
        return trimmed.to_string();
    }
    let tail: String = trimmed
        .chars()
        .skip(total.saturating_sub(max_chars))
        .collect();
    format!("…(末尾 {max_chars} 文字)\n{tail}")
}

fn truncate_to_chars(input: &str, max_chars: usize) -> String {
    let total = input.chars().count();
    if total <= max_chars {
        return input.to_string();
    }
    let head: String = input.chars().take(max_chars).collect();
    format!("{head}\n…(truncated)")
}

fn format_changed_files(raw: &Option<String>) -> String {
    let value = match raw {
        Some(value) => value.trim(),
        None => return "(なし)".to_string(),
    };
    if value.is_empty() {
        return "(なし)".to_string();
    }
    if let Ok(json) = serde_json::from_str::<Vec<String>>(value) {
        if json.is_empty() {
            return "(なし)".to_string();
        }
        let joined = json
            .iter()
            .map(|path| format!("- {path}"))
            .collect::<Vec<_>>()
            .join("\n");
        return take_head_chars(&joined, RETRO_CHANGED_FILES_CHARS);
    }
    take_head_chars(value, RETRO_CHANGED_FILES_CHARS)
}

fn build_retro_review_prompt(
    role: &crate::db::TeamRole,
    tasks: &[crate::db::Task],
    runs: &[crate::db::AgentRetroRun],
    notes: &[crate::db::ProjectNote],
    usage: &crate::db::SprintLlmUsageSummary,
) -> String {
    let system_prompt_head = take_head_chars(&role.system_prompt, 300);

    let task_section = if tasks.is_empty() {
        "(担当タスクなし)".to_string()
    } else {
        tasks
            .iter()
            .map(|task| {
                let description = task
                    .description
                    .as_deref()
                    .map(|d| take_head_chars(d, 200))
                    .unwrap_or_default();
                format!(
                    "- [{status}] {title}{desc}",
                    status = task.status,
                    title = task.title,
                    desc = if description.is_empty() {
                        String::new()
                    } else {
                        format!("\n    {description}")
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let run_count = runs.len();
    let selected_runs: Vec<&crate::db::AgentRetroRun> = runs
        .iter()
        .rev()
        .take(RETRO_MAX_RUNS_IN_PROMPT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let run_section_raw = if selected_runs.is_empty() {
        "(実行ログなし)".to_string()
    } else {
        selected_runs
            .iter()
            .enumerate()
            .map(|(idx, run)| {
                let final_answer_excerpt = run
                    .final_answer
                    .as_deref()
                    .map(|value| take_head_chars(value, RETRO_FINAL_ANSWER_HEAD_CHARS))
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "(final_answer なし)".to_string());
                let reasoning_excerpt = run
                    .reasoning_log
                    .as_deref()
                    .map(|value| take_tail_chars(value, RETRO_REASONING_TAIL_CHARS))
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "(reasoning_log なし)".to_string());
                let changed_files = format_changed_files(&run.changed_files_json);
                format!(
                    "### Run {index} — started_at={started} — success={success} — tool_events={tools}\n- final_answer:\n{final_answer}\n- reasoning_log (末尾):\n{reasoning}\n- changed_files:\n{changed_files}",
                    index = idx + 1,
                    started = run.started_at,
                    success = run.success,
                    tools = run.tool_event_count,
                    final_answer = final_answer_excerpt,
                    reasoning = reasoning_excerpt,
                    changed_files = changed_files,
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let run_section = if run_section_raw.chars().count() > RETRO_LOG_SECTION_CAP_CHARS {
        log::warn!(
            "retro prompt: run section truncated ({} chars -> {})",
            run_section_raw.chars().count(),
            RETRO_LOG_SECTION_CAP_CHARS
        );
        truncate_to_chars(&run_section_raw, RETRO_LOG_SECTION_CAP_CHARS)
    } else {
        run_section_raw
    };

    let notes_section = if notes.is_empty() {
        "(関連ノートなし)".to_string()
    } else {
        notes
            .iter()
            .map(|note| {
                let content_excerpt = take_head_chars(&note.content, 500);
                format!("- **{title}**\n{content}", title = note.title, content = content_excerpt)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        r#"あなたは熟練のスクラムマスター補佐です。以下のスプリント活動ログを読み、担当ロール「{role_name}」の観点で Keep / Problem / Try を抽出してください。

# 担当ロール
- 名前: {role_name}
- 役割: {role_prompt}

# スプリント統計
- LLM呼び出し回数: {events}
- 失敗回数: {failures}
- 入出力トークン: {input_tokens} / {output_tokens}
- 推定コスト(USD): {cost:.4}

# 担当タスク一覧 ({task_count} 件)
{tasks}

# 実行ログ抜粋 (最新 {selected}/{total} 件)
{runs}

# プロジェクトノート
{notes}

# 出力フォーマット（厳守）
JSON 配列のみを返してください。前後に説明や ``` を付けないでください。
3 〜 6 件、Keep / Problem / Try のバランスを意識した日本語で記述してください。
[
  {{"category": "keep|problem|try", "content": "..."}},
  ...
]"#,
        role_name = role.name,
        role_prompt = system_prompt_head,
        events = usage.total_events,
        failures = usage.failure_count,
        input_tokens = usage.total_input_tokens,
        output_tokens = usage.total_output_tokens,
        cost = usage.total_cost_usd,
        task_count = tasks.len(),
        tasks = task_section,
        selected = selected_runs.len(),
        total = run_count,
        runs = run_section,
        notes = notes_section,
    )
}

fn build_retro_kpt_synthesis_prompt(
    items: &[crate::db::RetroItem],
    role_lookup: &std::collections::HashMap<String, String>,
    usage: &crate::db::SprintLlmUsageSummary,
) -> String {
    // sm アイテムは再合成の素材に含めない（再実行時の二重掲載を防ぐ）
    let source_items: Vec<&crate::db::RetroItem> =
        items.iter().filter(|item| item.source != "sm").collect();

    let grouped = |category: &str| -> String {
        let lines: Vec<String> = source_items
            .iter()
            .filter(|item| item.category == category)
            .map(|item| {
                let role_label = item
                    .source_role_id
                    .as_deref()
                    .and_then(|id| role_lookup.get(id))
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| match item.source.as_str() {
                        "po" => "PO",
                        "user" => "ユーザー",
                        _ => "不明",
                    });
                let content_excerpt = take_head_chars(&item.content, 600);
                format!("[{role}] {content}", role = role_label, content = content_excerpt)
            })
            .collect();
        if lines.is_empty() {
            "(該当アイテムなし)".to_string()
        } else {
            lines.join("\n")
        }
    };

    let item_count = source_items.len();

    format!(
        r##"あなたは経験豊富なスクラムマスター (SM) です。
以下に各ロール（開発エージェント・PO・ユーザー）が個別に出した KPT の生データを示します。
あなたの役割はこれらを**そのまま転記することではなく**、チーム全体を俯瞰した上で「ロール横断パターン」「根本原因」「プロセス改善機会」を抽出し、次スプリントに直結する洞察を生成することです。

## SM の視点で必ず行うこと
- 複数ロールに共通して見られる傾向をひとつの上位アイテムにまとめる
- 表面的な現象（例: "テストが遅い"）ではなく、根本原因（例: "テスト環境の共有によるボトルネック"）を示す
- Try は「誰が」「何を」「いつまでに」が想像できる具体的アクションにする
- 既存アイテムを言い換えるだけのアイテムは生成しない
- SM サマリは単なる箇条書きではなく、このスプリントの「物語」を 1〜2 段落で語ること

## スプリント規模
- LLM 呼び出し: {events} 回 / 失敗: {failures} 回
- トークン: 入力 {input_tokens} / 出力 {output_tokens}
- 推定コスト: ${cost:.4}

## 素材アイテム数: {item_count} 件

### Keep（良かったこと）
{keeps}

### Problem（課題・問題）
{problems}

### Try（次に試すこと）
{tries}

## 出力要件
1. 各カテゴリ 2〜5 件、チーム全体視点の**新たな洞察**として書く（入力の言い換え禁止）
2. summary_markdown は 400〜800 字程度のMarkdown。見出し「## ハイライト」「## リスク」「## 次スプリントへ」の構造を使う
3. 日本語で出力する

## 出力フォーマット（厳守）
前後に説明や ``` を付けず、以下の JSON オブジェクトのみを返してください。
{{
  "summary_markdown": "...",
  "items": [
    {{"category": "keep|problem|try", "content": "チーム全体視点の洞察"}},
    ...
  ]
}}"##,
        events = usage.total_events,
        failures = usage.failure_count,
        input_tokens = usage.total_input_tokens,
        output_tokens = usage.total_output_tokens,
        cost = usage.total_cost_usd,
        item_count = item_count,
        keeps = grouped("keep"),
        problems = grouped("problem"),
        tries = grouped("try"),
    )
}

fn normalize_retro_category(raw: &str) -> String {
    let lowered = raw.trim().to_lowercase();
    match lowered.as_str() {
        "keep" | "problem" | "try" => lowered,
        _ => "problem".to_string(),
    }
}

fn parse_retro_review_items(content: &str) -> Vec<RetroReviewItem> {
    match parse_json_response::<Vec<RetroReviewItem>>(content) {
        Ok(items) => items
            .into_iter()
            .filter(|item| !item.content.trim().is_empty())
            .collect(),
        Err(error) => {
            log::warn!("retro review JSON parse failed: {error}");
            let fallback_content =
                take_head_chars(content, RETRO_SUMMARY_FALLBACK_MAX_CHARS);
            vec![RetroReviewItem {
                category: "problem".to_string(),
                content: format!("(自動分類に失敗) {fallback_content}"),
            }]
        }
    }
}

fn parse_retro_synthesis_response(content: &str) -> RetroSynthesisResponse {
    match parse_json_response::<RetroSynthesisResponse>(content) {
        Ok(parsed) => parsed,
        Err(error) => {
            log::warn!("retro synthesis JSON parse failed: {error}");
            RetroSynthesisResponse {
                summary_markdown: take_head_chars(content, RETRO_SUMMARY_FALLBACK_MAX_CHARS),
                items: Vec::new(),
            }
        }
    }
}

async fn call_retro_llm(
    app: &AppHandle,
    project_id: &str,
    sprint_id: &str,
    system_prompt: &str,
    prompt: &str,
    transport: PoTransport,
) -> Result<String, String> {
    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let response = crate::rig_provider::chat_with_history(
                &provider,
                &api_key,
                &model,
                system_prompt,
                prompt,
                vec![],
            )
            .await?;
            if let Err(error) = crate::llm_observability::record_llm_usage(
                app,
                crate::llm_observability::RecordLlmUsageInput {
                    project_id: project_id.to_string(),
                    task_id: None,
                    sprint_id: Some(sprint_id.to_string()),
                    source_kind: RETRO_REVIEW_SOURCE_KIND.to_string(),
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
                log::warn!("Failed to record retro LLM usage (api): {error}");
            }
            Ok(response.content)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let cli_prompt = format!("{system_prompt}\n\n{prompt}");
            let result = execute_po_cli_prompt::<serde_json::Value>(
                &cli_type, &model, &cli_prompt, &cwd,
            )
            .await?;
            if let Err(error) = crate::llm_observability::record_cli_usage(
                app,
                crate::llm_observability::ClaudeCliUsageRecordInput {
                    project_id: Some(project_id.to_string()),
                    task_id: None,
                    sprint_id: Some(sprint_id.to_string()),
                    source_kind: RETRO_REVIEW_SOURCE_KIND.to_string(),
                    cli_type: cli_type.as_str().to_string(),
                    model: result.metadata.model.clone(),
                    request_started_at: result.metadata.request_started_at,
                    request_completed_at: result.metadata.request_completed_at,
                    success: true,
                    error_message: None,
                },
            )
            .await
            {
                log::warn!("Failed to record retro LLM usage (cli): {error}");
            }
            Ok(result.value.to_string())
        }
    }
}

#[tauri::command]
pub async fn generate_agent_retro_review(
    app: AppHandle,
    project_id: String,
    sprint_id: String,
    retro_session_id: String,
    role_id: String,
    skip_inactive: bool,
) -> Result<Vec<crate::db::RetroItem>, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let role = crate::db::get_team_role_by_id(&app, &role_id)
        .await?
        .ok_or_else(|| format!("team_role が見つかりません: {role_id}"))?;
    let tasks = crate::db::get_tasks_by_sprint_and_role(&app, &sprint_id, &role_id).await?;
    let runs =
        crate::db::get_agent_retro_runs_by_sprint_and_role(&app, &sprint_id, &role.name).await?;

    // 稼働実績なしロールのスキップ:
    // skip_inactive=true かつ当スプリントにタスクも実行ログも存在しない場合は
    // 無駄な LLM 呼び出しを行わずに早期リターンする。
    if skip_inactive && tasks.is_empty() && runs.is_empty() {
        log::info!(
            "generate_agent_retro_review: スキップ (未稼働ロール) role={} sprint={}",
            role.name,
            sprint_id
        );
        return Ok(vec![]);
    }

    let notes = crate::db::get_project_notes_by_sprint(&app, &project_id, &sprint_id).await?;
    let usage = crate::db::get_llm_usage_summary_by_sprint(&app, &sprint_id).await?;

    let prompt = build_retro_review_prompt(&role, &tasks, &runs, &notes, &usage);
    let system_prompt = "あなたはスクラムレトロスペクティブを自動化する熟練のスクラムマスターです。必ず JSON のみを返します。";

    let response_text =
        call_retro_llm(&app, &project_id, &sprint_id, system_prompt, &prompt, transport).await?;

    let review_items = parse_retro_review_items(&response_text);

    let mut persisted = Vec::with_capacity(review_items.len());
    let base_sort_order = crate::db::get_retro_items(app.clone(), retro_session_id.clone())
        .await
        .unwrap_or_default()
        .len() as i32;
    for (index, item) in review_items.into_iter().enumerate() {
        let category = normalize_retro_category(&item.category);
        let content = item.content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        let created = crate::db::add_retro_item(
            app.clone(),
            retro_session_id.clone(),
            category,
            content,
            "agent".to_string(),
            Some(role_id.clone()),
            Some(base_sort_order + index as i32),
        )
        .await?;
        persisted.push(created);
    }

    Ok(persisted)
}

#[tauri::command]
pub async fn synthesize_retro_kpt(
    app: AppHandle,
    project_id: String,
    sprint_id: String,
    retro_session_id: String,
) -> Result<String, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let session = crate::db::get_retro_session(app.clone(), retro_session_id.clone())
        .await?
        .ok_or_else(|| format!("retro_session が見つかりません: {retro_session_id}"))?;
    if session.sprint_id != sprint_id {
        return Err("retro_session の sprint_id が一致しません".to_string());
    }

    let items = crate::db::get_retro_items(app.clone(), retro_session_id.clone()).await?;
    let usage = crate::db::get_llm_usage_summary_by_sprint(&app, &sprint_id).await?;

    let mut role_lookup: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    // sm アイテムはロールを持たないのでスキップ
    for item in items.iter().filter(|i| i.source != "sm") {
        if let Some(role_id) = item.source_role_id.as_deref() {
            if !role_lookup.contains_key(role_id) {
                if let Ok(Some(role)) = crate::db::get_team_role_by_id(&app, role_id).await {
                    role_lookup.insert(role_id.to_string(), role.name);
                }
            }
        }
    }

    // 再実行時に前回の SM アイテムが残らないよう、合成前にクリアする
    if let Err(e) = crate::db::delete_retro_items_by_source(
        &app,
        &retro_session_id,
        "sm",
    ).await {
        log::warn!("SM アイテムの事前削除に失敗しました (続行): {e}");
    }

    let prompt = build_retro_kpt_synthesis_prompt(&items, &role_lookup, &usage);
    let system_prompt = "あなたはスクラムマスター (SM) として、チーム全体の KPT を統合し Markdown サマリと統合 KPT を JSON で返します。";

    let response_text =
        call_retro_llm(&app, &project_id, &sprint_id, system_prompt, &prompt, transport).await?;

    let parsed = parse_retro_synthesis_response(&response_text);
    let summary_markdown = parsed.summary_markdown.trim().to_string();

    let base_sort_order = items.len() as i32;
    for (index, item) in parsed.items.into_iter().enumerate() {
        let category = normalize_retro_category(&item.category);
        let content = item.content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        let _ = crate::db::add_retro_item(
            app.clone(),
            retro_session_id.clone(),
            category,
            content,
            "sm".to_string(),
            None,
            Some(base_sort_order + index as i32),
        )
        .await?;
    }

    crate::db::update_retro_session(
        app.clone(),
        retro_session_id.clone(),
        "completed".to_string(),
        Some(summary_markdown.clone()),
    )
    .await?;

    Ok(summary_markdown)
}

#[cfg(test)]
mod tests {
    use super::{
        build_backlog_counts_reply, build_gemini_trust_hint, build_inception_system_prompt,
        build_retro_kpt_synthesis_prompt, build_retro_review_prompt,
        build_team_leader_provider_unavailable_reply, has_product_context_document,
        is_transient_provider_unavailable, looks_like_generic_backlog_creation_request,
        normalize_retro_category, parse_retro_review_items, parse_retro_synthesis_response,
        truncate_output_tail, ProjectBacklogCounts,
    };
    use crate::cli_runner::CliType;
    use crate::db::{
        AgentRetroRun, ProjectNote, RetroItem, SprintLlmUsageSummary, Task, TeamRole,
    };
    use std::collections::HashMap;

    fn make_role(name: &str) -> TeamRole {
        TeamRole {
            id: format!("role-{name}"),
            name: name.to_string(),
            system_prompt: "あなたはテスト用ロールです。".to_string(),
            cli_type: "claude".to_string(),
            model: "claude-haiku-4-5".to_string(),
            avatar_image: None,
            sort_order: 0,
        }
    }

    fn make_run(idx: usize, reasoning: &str) -> AgentRetroRun {
        AgentRetroRun {
            id: format!("run-{idx}"),
            project_id: "project-1".to_string(),
            task_id: Some(format!("task-{idx}")),
            sprint_id: Some("sprint-1".to_string()),
            source_kind: "task_execution".to_string(),
            role_name: "Lead Engineer".to_string(),
            cli_type: "claude".to_string(),
            model: "claude-haiku-4-5".to_string(),
            started_at: 1_000 + idx as i64,
            completed_at: 2_000 + idx as i64,
            duration_ms: 1_000,
            success: true,
            error_message: None,
            reasoning_log: Some(reasoning.to_string()),
            final_answer: Some(format!("final answer {idx}")),
            changed_files_json: Some("[\"src/a.rs\",\"src/b.rs\"]".to_string()),
            tool_event_count: 2,
            created_at: "2026-04-15 00:00:00".to_string(),
        }
    }

    fn empty_usage() -> SprintLlmUsageSummary {
        SprintLlmUsageSummary {
            total_events: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
            failure_count: 0,
        }
    }

    #[test]
    fn retro_review_prompt_includes_role_and_tasks() {
        let role = make_role("Lead Engineer");
        let tasks = vec![Task {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            story_id: "story-1".to_string(),
            sequence_number: 1,
            title: "通知APIの設計".to_string(),
            description: Some("pub/sub の概要".to_string()),
            status: "Done".to_string(),
            sprint_id: Some("sprint-1".to_string()),
            archived: false,
            assignee_type: None,
            assigned_role_id: Some("role-Lead Engineer".to_string()),
            created_at: "2026-04-15 00:00:00".to_string(),
            updated_at: "2026-04-15 00:00:00".to_string(),
            priority: 3,
        }];
        let prompt = build_retro_review_prompt(&role, &tasks, &[], &[], &empty_usage());
        assert!(prompt.contains("Lead Engineer"));
        assert!(prompt.contains("通知APIの設計"));
        assert!(prompt.contains("JSON 配列のみ"));
        assert!(prompt.contains("(実行ログなし)"));
    }

    #[test]
    fn retro_review_prompt_truncates_long_reasoning_log() {
        let role = make_role("Lead Engineer");
        let huge = "あ".repeat(100_000);
        let runs: Vec<AgentRetroRun> = (0..3).map(|i| make_run(i, &huge)).collect();
        let prompt = build_retro_review_prompt(&role, &[], &runs, &[], &empty_usage());
        assert!(prompt.chars().count() < 25_000);
        assert!(prompt.contains("truncated") || prompt.contains("末尾"));
    }

    #[test]
    fn retro_review_prompt_limits_to_last_n_runs() {
        let role = make_role("Lead Engineer");
        let runs: Vec<AgentRetroRun> = (0..30).map(|i| make_run(i, "short")).collect();
        let prompt = build_retro_review_prompt(&role, &[], &runs, &[], &empty_usage());
        assert!(prompt.contains("最新 10/30 件"));
    }

    #[test]
    fn retro_review_prompt_includes_notes_section() {
        let role = make_role("Lead Engineer");
        let notes = vec![ProjectNote {
            id: "note-1".to_string(),
            project_id: "project-1".to_string(),
            sprint_id: Some("sprint-1".to_string()),
            title: "レビュー観点".to_string(),
            content: "境界条件を重点確認する".to_string(),
            source: "user".to_string(),
            created_at: "2026-04-15 00:00:00".to_string(),
            updated_at: "2026-04-15 00:00:00".to_string(),
        }];
        let prompt = build_retro_review_prompt(&role, &[], &[], &notes, &empty_usage());
        assert!(prompt.contains("レビュー観点"));
        assert!(prompt.contains("境界条件"));
    }

    #[test]
    fn parse_retro_review_items_valid_json() {
        let raw = "[{\"category\":\"keep\",\"content\":\"速度が良かった\"},{\"category\":\"problem\",\"content\":\"タスク見積が甘い\"}]";
        let items = parse_retro_review_items(raw);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category, "keep");
        assert!(items[1].content.contains("見積"));
    }

    #[test]
    fn parse_retro_review_items_falls_back_on_garbage() {
        let items = parse_retro_review_items("not json at all");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, "problem");
        assert!(items[0].content.contains("自動分類に失敗"));
    }

    #[test]
    fn parse_retro_review_items_with_surrounding_noise() {
        let raw = "以下の通りです:\n[{\"category\":\"try\",\"content\":\"CI並列化を試す\"}]\n以上です";
        let items = parse_retro_review_items(raw);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, "try");
    }

    #[test]
    fn parse_retro_synthesis_response_valid() {
        let raw = "{\"summary_markdown\":\"# Sprint\\n良かった\",\"items\":[{\"category\":\"keep\",\"content\":\"ペア作業\"}]}";
        let parsed = parse_retro_synthesis_response(raw);
        assert!(parsed.summary_markdown.contains("Sprint"));
        assert_eq!(parsed.items.len(), 1);
    }

    #[test]
    fn parse_retro_synthesis_response_missing_items_field() {
        let raw = "{\"summary_markdown\":\"サマリのみ\"}";
        let parsed = parse_retro_synthesis_response(raw);
        assert_eq!(parsed.summary_markdown, "サマリのみ");
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn parse_retro_synthesis_response_falls_back_on_plain_text() {
        let parsed = parse_retro_synthesis_response("ただのテキスト出力");
        assert!(parsed.summary_markdown.contains("ただのテキスト"));
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn normalize_retro_category_coerces_unknown_to_problem() {
        assert_eq!(normalize_retro_category("Keep"), "keep");
        assert_eq!(normalize_retro_category("PROBLEM"), "problem");
        assert_eq!(normalize_retro_category("try"), "try");
        assert_eq!(normalize_retro_category("insight"), "problem");
        assert_eq!(normalize_retro_category(""), "problem");
    }

    #[test]
    fn retro_kpt_synthesis_prompt_groups_items_by_category() {
        let items = vec![
            RetroItem {
                id: "i1".to_string(),
                retro_session_id: "s1".to_string(),
                category: "keep".to_string(),
                content: "ペア作業が有効".to_string(),
                source: "agent".to_string(),
                source_role_id: Some("role-Lead Engineer".to_string()),
                is_approved: false,
                sort_order: 0,
                created_at: "2026-04-15 00:00:00".to_string(),
            },
            RetroItem {
                id: "i2".to_string(),
                retro_session_id: "s1".to_string(),
                category: "problem".to_string(),
                content: "見積精度".to_string(),
                source: "user".to_string(),
                source_role_id: None,
                is_approved: false,
                sort_order: 1,
                created_at: "2026-04-15 00:00:00".to_string(),
            },
        ];
        let mut role_lookup = HashMap::new();
        role_lookup.insert("role-Lead Engineer".to_string(), "Lead Engineer".to_string());
        let prompt = build_retro_kpt_synthesis_prompt(&items, &role_lookup, &empty_usage());
        assert!(prompt.contains("### Keep（良かったこと）"));
        assert!(prompt.contains("ペア作業が有効"));
        assert!(prompt.contains("### Problem（課題・問題）"));
        assert!(prompt.contains("見積精度"));
        assert!(prompt.contains("summary_markdown"));
        assert!(prompt.contains("Lead Engineer"));
    }

    #[test]
    fn generic_backlog_creation_request_is_detected() {
        assert!(looks_like_generic_backlog_creation_request(
            "バックログを1つ作成してください"
        ));
    }

    #[test]
    fn task_addition_to_existing_story_does_not_use_generic_story_fallback() {
        assert!(!looks_like_generic_backlog_creation_request(
            "既存 story ID: abc にタスクを追加してください"
        ));
    }

    #[test]
    fn product_context_document_is_detected_from_project_context_block() {
        assert!(has_product_context_document(
            "\n【プロジェクト既存ドキュメントコンテキスト】\n--- PRODUCT_CONTEXT.md ---\n# sample"
        ));
        assert!(!has_product_context_document(
            "\n【現在のバックログ】\nstory-1: 既存ストーリー"
        ));
    }

    #[test]
    fn backlog_counts_reply_reports_actual_deltas() {
        let response = build_backlog_counts_reply(
            "部分成功".to_string(),
            ProjectBacklogCounts {
                stories: 2,
                tasks: 5,
                dependencies: 1,
            },
            ProjectBacklogCounts {
                stories: 3,
                tasks: 8,
                dependencies: 4,
            },
        )
        .expect("reply should exist when backlog changes");

        assert!(response.reply.contains("部分成功"));
        assert!(response.reply.contains("stories +1"));
        assert!(response.reply.contains("tasks +3"));
        assert!(response.reply.contains("dependencies +3"));
    }

    #[test]
    fn transient_provider_unavailable_detects_gemini_503() {
        let error = "Gemini error: CompletionError: HttpError: Invalid status code 503 Service Unavailable with message: {\"error\":{\"status\":\"UNAVAILABLE\",\"message\":\"high demand\"}}";
        assert!(is_transient_provider_unavailable(error));
    }

    #[test]
    fn provider_unavailable_reply_mentions_no_creation_for_mutation() {
        let response = build_team_leader_provider_unavailable_reply(
            "Gemini error: 503 Service Unavailable",
            true,
        );

        assert!(response
            .reply
            .contains("今回はバックログを作成していません"));
        assert!(response.reply.contains("503 Service Unavailable"));
    }

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
            build_gemini_trust_hint(&CliType::Claude, "Project is not in a trusted folder.", "",),
            None
        );
    }

    #[test]
    fn inception_prompt_uses_scrum_product_partner_role_and_guidance() {
        let prompt = build_inception_system_prompt(1, "");

        assert!(prompt.contains("Scrum Product Partner"));
        assert!(prompt.contains("一問一答に固執せず"));
        assert!(prompt.contains("答え方の例を 1 つだけ示してよい"));
    }

    #[test]
    fn phase_one_inception_prompt_requests_elevator_pitch_details() {
        let prompt = build_inception_system_prompt(1, "");

        assert!(prompt.contains("## 2. エレベーターピッチ"));
        assert!(prompt.contains("差別化ポイント"));
        assert!(prompt.contains("既存のやり方や競合と比べた違い"));
    }

    #[test]
    fn inception_prompt_includes_loop_prevention_exit_condition() {
        let prompt = build_inception_system_prompt(1, "");

        assert!(prompt.contains("ヒアリングのループを防ぐため"));
        assert!(prompt.contains("それで十分"));
        assert!(prompt.contains("同じ論点を言い換えて繰り返し聞かない"));
    }

    #[test]
    fn each_phase_prompt_describes_completion_criteria() {
        let phase_two_prompt = build_inception_system_prompt(2, "");
        let phase_three_prompt = build_inception_system_prompt(3, "");
        let phase_four_prompt = build_inception_system_prompt(4, "");

        assert!(phase_two_prompt.contains("【完了の目安】"));
        assert!(phase_three_prompt.contains("【完了の目安】"));
        assert!(phase_four_prompt.contains("【完了の目安】"));
    }

    #[test]
    fn phase_three_prompt_asks_for_usage_context_before_technology_names() {
        let prompt = build_inception_system_prompt(3, "");

        assert!(prompt.contains("PCブラウザ / スマホ / タブレット"));
        assert!(prompt.contains("ローカル中心でよいか"));
        assert!(prompt.contains("技術名を答えられなくても進められる"));
    }
}
