use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PoAssistantExecutionPlan {
    pub reply: Option<String>,
    pub operations: Vec<crate::ai_tools::CreateStoryAndTasksArgs>,
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

fn canonicalize_for_matching(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn path_is_within_trusted_root(candidate: &Path, trusted_root: &Path) -> bool {
    let candidate = canonicalize_for_matching(candidate);
    let trusted_root = canonicalize_for_matching(trusted_root);

    candidate == trusted_root || candidate.starts_with(&trusted_root)
}

fn gemini_trusted_folders_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");

    home.map(|home| {
        PathBuf::from(home)
            .join(".gemini")
            .join("trustedFolders.json")
    })
}

fn load_gemini_trusted_folders() -> Vec<PathBuf> {
    let Some(path) = gemini_trusted_folders_path() else {
        return Vec::new();
    };

    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };

    let Ok(entries) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw)
    else {
        return Vec::new();
    };

    entries
        .into_iter()
        .filter_map(|(path, trust_kind)| match trust_kind.as_str() {
            Some("TRUST_FOLDER" | "TRUST_PARENT") => Some(PathBuf::from(path)),
            _ => None,
        })
        .collect()
}

fn select_gemini_execution_cwd(project_cwd: &str, trusted_folders: &[PathBuf]) -> String {
    let project_path = Path::new(project_cwd);

    if trusted_folders
        .iter()
        .any(|trusted_root| path_is_within_trusted_root(project_path, trusted_root))
    {
        return project_cwd.to_string();
    }

    trusted_folders
        .iter()
        .find(|trusted_root| trusted_root.is_dir())
        .map(|trusted_root| trusted_root.to_string_lossy().to_string())
        .unwrap_or_else(|| project_cwd.to_string())
}

fn resolve_gemini_cli_execution_cwd(project_cwd: &str) -> String {
    let trusted_folders = load_gemini_trusted_folders();
    select_gemini_execution_cwd(project_cwd, &trusted_folders)
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
        let project_cwd = resolve_project_cli_cwd(app, project_id).await?;
        let cwd = if cli_type == crate::cli_runner::CliType::Gemini {
            resolve_gemini_cli_execution_cwd(&project_cwd)
        } else {
            project_cwd
        };

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
    let cli_command_path = crate::cli_detection::resolve_cli_command_path(runner.command_name())
        .ok_or_else(|| build_cli_not_found_message(runner.as_ref()))?;
    let resolved_model = runner.resolve_model(model);
    let args = runner.build_args(prompt, &resolved_model, cwd);
    let stdin_payload = runner.stdin_payload(prompt);
    let env_vars = runner.env_vars();
    let timeout_secs = runner.timeout_secs();
    let display_name = runner.display_name().to_string();
    let cli_not_found_message = build_cli_not_found_message(runner.as_ref());
    let cwd = cwd.to_string();

    let request_started_at = current_timestamp_millis()?;
    let output = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        tauri::async_runtime::spawn_blocking(move || {
            let mut command = std::process::Command::new(&cli_command_path);
            command.args(&args).current_dir(&cwd);
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
        format!(
            "{} の実行が {} 秒でタイムアウトしました。",
            display_name, timeout_secs
        )
    })?
    .map_err(|error| format!("{} の実行スレッドが失敗しました: {}", display_name, error))?
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            cli_not_found_message.clone()
        } else {
            format!("{} の実行に失敗しました: {}", display_name, error)
        }
    })?;
    let request_completed_at = current_timestamp_millis().unwrap_or(request_started_at);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        return Err(if detail.is_empty() {
            format!("{} がエラーで終了しました。", display_name)
        } else {
            format!("{} がエラーで終了しました: {}", display_name, detail)
        });
    }

    let value = parse_json_response::<T>(&stdout).map_err(|error| {
        format!(
            "{} の出力から有効な JSON を抽出できませんでした: {}",
            display_name, error
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
    if let Err(error) = crate::llm_observability::record_claude_cli_usage(
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
    let max_attempts = match provider {
        crate::rig_provider::AiProvider::Gemini => 2,
        _ => 1,
    };

    let mut attempt = 0;
    loop {
        attempt += 1;
        let chat_history = crate::rig_provider::convert_messages(prior_messages);
        match crate::rig_provider::chat_team_leader_with_tools(
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
        {
            Ok(response) => return Ok(response),
            Err(error) => {
                if attempt < max_attempts && is_transient_provider_unavailable(&error) {
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    continue;
                }

                return Err(error);
            }
        }
    }
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
    let PoAssistantExecutionPlan { reply, operations } = plan;
    if operations.is_empty() {
        return Ok(None);
    }

    for operation in operations {
        if operation.tasks.is_empty() {
            continue;
        }

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
            r#"## Phase 1: コア価値とターゲット (Why)

**ヒアリング目標** (2〜3往復で引き出す):
- 解決したい課題 / ターゲットユーザー / コアバリュー / プロダクトの目的

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (新規作成)
**出力テンプレート** — 箇条書き・20行以内で厳守:
```
# PRODUCT_CONTEXT.md — {プロダクト名}
> 【AIへの絶対指示】本ファイルはシステムプロンプトとして機能する。

## 0. 課題とコアバリュー
- 課題: {1行}
- 解決策: {1行}

## 1. プロダクト定義
- 対象: {ターゲット}
- 目標: {目標}

## 2. 役割分担
- 人間(PO): What の意思決定のみ
- AI: How の実行（タスク分解・実装・改善）
```"#
        }

        2 => {
            r#"## Phase 2: やらないことリスト (Not List)

**ヒアリング目標** (2〜3往復):
- スコープ外にすること / 絶対やってはならないこと

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (末尾に追記)
**追記テンプレート** — Section 3〜5のみ・15行以内:
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
            r#"## Phase 3: 技術スタック・アーキテクチャ (What)

**ヒアリング目標** (2〜3往復):
- 言語 / FW / DB / アーキテクチャ上の制約

**生成ファイル**: patch_target = "ARCHITECTURE.md" (新規作成)
**出力テンプレート** — 全体20行以内・箇条書きのみ:
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

**生成ファイル**: patch_target = "Rule.md" (末尾に追記)
**追記テンプレート** — 既存内容は絶対に含めない・15行以内:
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
        r#"あなたは「Inception Deckファシリテーター」です。

## 役割
ユーザーのプロダクト構想をヒアリングし、Markdownドキュメントとして策定することが唯一の仕事。

## 絶対禁止
- コード・実装手順の提案（例: Pythonコード、コマンド等）
- 「作り方」を教えること（あなたは企画コンサルであり、エンジニアではない）
- 他フェーズで生成済みのドキュメント内容を patch_content に含めること

## 出力品質規約（厳守）
- **箇条書きのみ** — 長文解説・説明・挨拶は不要
- **1項目1行** — 無駄な装飾を省く
- **patch_content は20行以内** — トークン節約が最優先
- **reply は1文のみ** — 例:「PRODUCT_CONTEXT.md を生成しました」

{phase_instruction}

{existing_docs}

## 出力フォーマット（必ずこの形式のJSONのみを返すこと）

ヒアリング中:
{{"reply": "質問（1文）", "is_finished": false, "patch_target": null, "patch_content": null}}

ドキュメント生成時:
{{"reply": "〜を生成しました。", "is_finished": true, "patch_target": "ファイル名.md", "patch_content": "Markdownの差分（20行以内）"}}

patch_content にはこのフェーズで追加する部分のみを含め、他フェーズの内容は絶対に含めないこと。"#,
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
                "あなたは vicara の Scrum Team に所属する POアシスタントです。あなたの役割は、プロダクトオーナーの意思決定を支援しながら、要求の具体化、バックログの優先順位整理、追加タスクの登録を進めることです。ユーザーから機能要件や追加タスクの要望があった場合、自身が持つツール (`create_story_and_tasks`) を必ず呼び出して、ストーリーとサブタスク群をデータベースに自動登録してください。\n\n【最重要ルール】\n- ユーザーがストーリーやタスクの作成・追加・登録を求めた場合、説明だけで終わらせず `create_story_and_tasks` を使うこと\n- 既存ストーリーにタスクを追加する依頼では、コンテキスト中の story ID を読んで `target_story_id` を必ず指定すること\n- 依頼が「バックログを1つ作って」のように抽象的でも、PRODUCT_CONTEXT.md / ARCHITECTURE.md / Rule.md と既存バックログからプロダクト固有の具体案を1件具体化して登録すること\n- 「新しいバックログ項目」「要求詳細を整理する」などのプレースホルダ名は禁止\n- ツールを呼んでいないのに「追加しました」「登録しました」と断定してはいけない\n- ツールが失敗した場合は、成功を装わずエラー内容を簡潔に伝えること\n\n【現在のプロダクトの状況（既存バックログ等）】\n{}\n\n【優先度と依存関係の設定ルール】\nストーリーとタスクを作成する際は、必ず以下のフィールドを設定してください：\n- story_priority: 整数 1〜5（小さいほど優先度が高い）\n- 各タスクの priority: 整数 1〜5（小さいほど優先度が高い）\n- 各タスクの blocked_by_indices: 先行タスクの配列インデックス（0始まり）を指定。依存がなければ省略か空配列\n\n優先度の判断基準（1〜5、数値が小さいほど重要）:\n- 1: 最重要 — アーキテクチャの根幹、他の全タスクをブロックする基盤作業\n- 2: 高優先 — クリティカルパス上のコア機能\n- 3: 中優先 — 重要な機能実装だが他をブロックしない（デフォルト）\n- 4: 低優先 — サポートタスク、テスト、軽微な改善\n- 5: 最低優先 — ドキュメント、UIの微調整、オプション機能\n\n【重要】ツール実行に失敗した場合は、エラー内容を確認して原因をユーザーに報告、または代替策を考えてください。ツールが失敗したからといって、決してユーザーに手動での登録作業を丸投げしないでください。\n\n会話の返答は必ず以下の形式のJSONオブジェクトのみで返してください。\n\n{{\"reply\": \"ツール実行結果やユーザーへのメッセージ内容\"}}",
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
                r#"あなたは vicara の Scrum Team に所属する POアシスタントです。会話内容と既存バックログを踏まえ、必要ならバックログ更新計画を JSON で返してください。CLI ではアプリ側が JSON 計画を解釈して DB 登録を実行します。

【ルール】
- バックログの追加・登録が不要な相談なら `operations` は空配列にする
- 既存ストーリーにタスクを追加する場合は、必ずコンテキストに存在する story ID を `target_story_id` に入れる
- 新規ストーリーを作る場合のみ `target_story_id` を null にし、`story_title` を必須で入れる
- 依頼が抽象的な新規バックログ作成でも、PRODUCT_CONTEXT.md / ARCHITECTURE.md / Rule.md と既存バックログからプロダクト固有の具体案を1件生成する
- 「新しいバックログ項目」「要求詳細を整理する」などのプレースホルダ名は禁止
- `tasks` は作成時に必ず 1 件以上含める
- 各 task には `title`, `description`, `priority`, `blocked_by_indices` を含める
- story_priority / task.priority は整数 1〜5
- ユーザー向け説明は `reply` に簡潔に書く
- 出力は必ず JSON オブジェクトのみ

【既存バックログ】
{context_md}

【これまでの会話】
{history_block}

【今回のユーザー依頼】
{latest_user_message}

返却形式:
{{
  "reply": "ユーザーへ返すメッセージ",
  "operations": [
    {{
      "target_story_id": null,
      "story_title": "ストーリー名",
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
            if plan.operations.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::{
        build_backlog_counts_reply, build_team_leader_provider_unavailable_reply,
        has_product_context_document, is_transient_provider_unavailable,
        looks_like_generic_backlog_creation_request, select_gemini_execution_cwd,
        ProjectBacklogCounts,
    };
    use std::path::PathBuf;

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
    fn gemini_execution_cwd_keeps_project_path_when_trusted() {
        let project_dir = tempfile::tempdir().expect("tempdir should exist");
        let trusted_roots = vec![project_dir.path().to_path_buf()];

        let resolved = select_gemini_execution_cwd(
            project_dir.path().to_string_lossy().as_ref(),
            &trusted_roots,
        );

        assert_eq!(resolved, project_dir.path().to_string_lossy());
    }

    #[test]
    fn gemini_execution_cwd_falls_back_to_existing_trusted_root() {
        let project_dir = tempfile::tempdir().expect("project tempdir should exist");
        let trusted_dir = tempfile::tempdir().expect("trusted tempdir should exist");
        let trusted_roots = vec![
            PathBuf::from("C:/missing-trusted-root"),
            trusted_dir.path().to_path_buf(),
        ];

        let resolved = select_gemini_execution_cwd(
            project_dir.path().to_string_lossy().as_ref(),
            &trusted_roots,
        );

        assert_eq!(resolved, trusted_dir.path().to_string_lossy());
    }
}
