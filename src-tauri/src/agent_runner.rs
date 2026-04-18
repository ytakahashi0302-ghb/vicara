use crate::{
    agent_retro,
    cli_runner::{self, CliType},
    db, worktree,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

mod lifecycle;
mod prompting;
mod session;
mod spawn;

pub(crate) use spawn::execute_cli_prompt_task;

type BoxedProcessKiller = Box<dyn ProcessKiller + Send>;

struct AgentSession {
    info: ActiveAgentSession,
    temp_file_path: PathBuf,
    response_capture_path: Option<PathBuf>,
    usage_context: AgentUsageContext,
    retro_capture: Arc<Mutex<agent_retro::AgentRetroCapture>>,
    killer: BoxedProcessKiller,
}

#[derive(Clone, serde::Serialize)]
pub struct ActiveAgentSession {
    task_id: String,
    task_title: String,
    role_name: String,
    cli_type: String,
    model: String,
    started_at: i64,
    status: String,
}

enum AgentSessionEntry {
    Starting(ActiveAgentSession),
    Running(AgentSession),
}

trait ProcessKiller {
    fn kill(&mut self);
    fn wait_success(&mut self) -> bool;
}

#[derive(Clone)]
struct RecentOutputChunk {
    normalized: String,
    emitted_at: Instant,
}

pub struct AgentState {
    sessions: Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
}

impl AgentState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

const AGENT_CLI_STARTED_EVENT: &str = "agent_cli_started";
const AGENT_CLI_OUTPUT_EVENT: &str = "agent_cli_output";
const AGENT_CLI_EXIT_EVENT: &str = "agent_cli_exit";

#[derive(Clone, serde::Serialize)]
struct AgentOutputPayload {
    task_id: String,
    output: String,
}

#[derive(Clone, serde::Serialize)]
struct AgentExitPayload {
    task_id: String,
    success: bool,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_status: Option<String>,
}

#[derive(Clone)]
struct AgentUsageContext {
    source_kind: String,
    project_id: Option<String>,
    sprint_id: Option<String>,
    db_task_id: Option<String>,
}

trait AgentStdoutParser {
    fn consume(&mut self, chunk: &str) -> Vec<String>;

    fn finish(&mut self) -> Vec<String> {
        Vec::new()
    }
}

#[derive(Default)]
struct PassthroughAgentStdoutParser;

impl AgentStdoutParser for PassthroughAgentStdoutParser {
    fn consume(&mut self, chunk: &str) -> Vec<String> {
        vec![chunk.to_string()]
    }
}

#[tauri::command]
pub async fn get_active_agent_sessions(
    state: tauri::State<'_, AgentState>,
) -> Result<Vec<ActiveAgentSession>, String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let mut result: Vec<ActiveAgentSession> = sessions
        .values()
        .map(session::get_session_summary)
        .collect();
    result.sort_by_key(|session| session.started_at);
    Ok(result)
}

#[tauri::command]
pub async fn execute_agent_task(
    app_handle: AppHandle,
    state: tauri::State<'_, AgentState>,
    task_id: String,
    cwd: String,
    additional_context: Option<String>,
) -> Result<(), String> {
    let task = db::get_task_by_id(&app_handle, &task_id)
        .await?
        .ok_or_else(|| format!("task_id={} のタスクが見つかりません", task_id))?;

    if task.status == "In Progress" {
        return Err("このタスクはすでに進行中です。".to_string());
    }
    if task.status == "Done" {
        return Err("完了済みタスクは再実行できません。".to_string());
    }

    let role_id = task.assigned_role_id.clone().ok_or_else(|| {
        "担当ロールが未設定です。タスク詳細で担当ロールを選択してください。".to_string()
    })?;

    let role = db::get_team_role_by_id(&app_handle, &role_id)
        .await?
        .ok_or_else(|| format!("担当ロールが見つかりません: {}", role_id))?;
    let cli_type = CliType::from_str(&role.cli_type);
    let runner = cli_runner::create_runner(&cli_type)?;
    let cli_command_path = lifecycle::resolve_cli_command_path(runner.as_ref())?;

    if role.system_prompt.trim().is_empty() {
        return Err("担当ロールのシステムプロンプトが未設定です。".to_string());
    }

    let max_concurrent_agents = db::get_max_concurrent_agents_value(&app_handle).await?;
    let session_info = session::build_task_session_info(&task, &role, runner.as_ref())?;

    log::info!(
        "Preparing CLI task: task_id={}, role={}, cli_type={}, model={}, configured_limit={}",
        task.id,
        role.name,
        cli_type.as_str(),
        session_info.model,
        max_concurrent_agents
    );

    let existing_worktree_record = db::get_worktree_by_task_id(&app_handle, &task.id).await?;
    if existing_worktree_record
        .as_ref()
        .map(|record| record.status == "conflict")
        .unwrap_or(false)
    {
        worktree::remove_worktree(
            app_handle.clone(),
            app_handle.state::<worktree::PreviewState>(),
            app_handle.state::<worktree::WorktreeState>(),
            cwd.clone(),
            task.id.clone(),
        )
        .await?;
        log::info!(
            "Reset conflicted worktree before rerun: task_id={}, branch={}",
            task.id,
            existing_worktree_record
                .as_ref()
                .map(|record| record.branch_name.as_str())
                .unwrap_or("unknown")
        );
    }

    let existing_worktree = worktree::get_worktree_status(
        app_handle.state::<worktree::WorktreeState>(),
        cwd.clone(),
        task.id.clone(),
    )
    .await?;

    let (worktree_info, created_new_worktree) = match existing_worktree {
        Some(info) => (info, false),
        None => (
            worktree::create_worktree(
                app_handle.clone(),
                app_handle.state::<worktree::WorktreeState>(),
                cwd.clone(),
                task.id.clone(),
            )
            .await?,
            true,
        ),
    };

    if let Err(error) = db::upsert_worktree_record(
        &app_handle,
        db::WorktreeUpsertInput {
            id: db::get_worktree_by_task_id(&app_handle, &task.id)
                .await?
                .map(|record| record.id)
                .unwrap_or_else(|| format!("worktree-{}", task.id)),
            task_id: task.id.clone(),
            project_id: task.project_id.clone(),
            worktree_path: worktree_info.worktree_path.clone(),
            branch_name: worktree_info.branch_name.clone(),
            preview_port: None,
            preview_pid: None,
            status: "active".to_string(),
        },
    )
    .await
    {
        if created_new_worktree {
            let _ = worktree::remove_worktree(
                app_handle.clone(),
                app_handle.state::<worktree::PreviewState>(),
                app_handle.state::<worktree::WorktreeState>(),
                cwd.clone(),
                task.id.clone(),
            )
            .await;
        }
        return Err(error);
    }

    let rules_section = {
        let all_rules = db::get_retro_rules(app_handle.clone(), task.project_id.clone())
            .await
            .unwrap_or_default();
        let active_rules: Vec<_> = all_rules.into_iter().filter(|r| r.is_active).collect();
        if active_rules.is_empty() {
            String::new()
        } else {
            let rules_text = active_rules
                .iter()
                .map(|r| format!("- {}", r.content))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "# 過去のレトロスペクティブからのチームルール\n以下のルールを遵守してください:\n{}",
                rules_text
            )
        }
    };

    let combined_context = match (additional_context.as_deref(), rules_section.is_empty()) {
        (Some(ctx), false) => Some(format!("{}\n\n{}", ctx, rules_section)),
        (Some(ctx), true) => Some(ctx.to_string()),
        (None, false) => Some(rules_section),
        (None, true) => None,
    };

    let prompt = prompting::build_task_prompt(&task, &role, combined_context.as_deref());
    let usage_context = AgentUsageContext {
        source_kind: "task_execution".to_string(),
        project_id: Some(task.project_id.clone()),
        sprint_id: task.sprint_id.clone(),
        db_task_id: Some(task.id.clone()),
    };
    let sessions_arc =
        match session::reserve_session_slot(&state, session_info.clone(), max_concurrent_agents) {
            Ok(sessions_arc) => sessions_arc,
            Err(error) => {
                if created_new_worktree {
                    let _ = worktree::remove_worktree(
                        app_handle.clone(),
                        app_handle.state::<worktree::PreviewState>(),
                        app_handle.state::<worktree::WorktreeState>(),
                        cwd.clone(),
                        task.id.clone(),
                    )
                    .await;
                }
                return Err(error);
            }
        };
    let result = spawn::execute_prompt_request(
        app_handle.clone(),
        runner.as_ref(),
        cli_command_path,
        sessions_arc,
        session_info,
        prompt,
        worktree_info.worktree_path.clone(),
        usage_context,
    )
    .await;

    if result.is_err() && created_new_worktree {
        let _ = worktree::remove_worktree(
            app_handle.clone(),
            app_handle.state::<worktree::PreviewState>(),
            app_handle.state::<worktree::WorktreeState>(),
            cwd,
            task.id.clone(),
        )
        .await;
    }

    result
}

#[tauri::command]
pub async fn kill_agent_process(
    app_handle: AppHandle,
    state: tauri::State<'_, AgentState>,
    task_id: String,
) -> Result<(), String> {
    let entry = session::remove_session_entry(&state.sessions, &task_id)
        .ok_or_else(|| format!("task_id={} に紐づく CLI プロセスは存在しません。", task_id))?;

    match entry {
        AgentSessionEntry::Running(mut session) => {
            session.killer.kill();
            prompting::cleanup_temp_file(&session.temp_file_path);
            if let Some(response_capture_path) = session.response_capture_path.as_ref() {
                lifecycle::persist_agent_retro_run_for_session(
                    &app_handle,
                    &session.info,
                    &session.usage_context,
                    &session.retro_capture,
                    Some(response_capture_path),
                    false,
                    "Manually killed by user.".to_string(),
                )
                .await;
                prompting::cleanup_temp_file(response_capture_path);
            } else {
                lifecycle::persist_agent_retro_run_for_session(
                    &app_handle,
                    &session.info,
                    &session.usage_context,
                    &session.retro_capture,
                    None,
                    false,
                    "Manually killed by user.".to_string(),
                )
                .await;
            }
        }
        AgentSessionEntry::Starting(_) => {}
    }

    app_handle
        .emit(
            AGENT_CLI_EXIT_EVENT,
            AgentExitPayload {
                task_id,
                success: false,
                reason: "Manually killed by user.".into(),
                new_status: None,
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{lifecycle, prompting};
    use crate::cli_runner::{claude::ClaudeRunner, codex::CodexRunner, CliRunner};
    use crate::db::{Task, TeamRole};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn prepare_cli_invocation_keeps_argument_prompt_for_arg_based_runner() {
        let runner = ClaudeRunner;
        let prepared = prompting::prepare_cli_invocation(
            &runner,
            Path::new("claude"),
            "task-1",
            "sample prompt",
            "claude-model",
            "C:/repo",
        )
        .expect("claude invocation should be prepared");

        assert_eq!(prepared.command_path, Path::new("claude"));
        assert_eq!(prepared.stdin_payload, None);
        assert!(prepared.response_capture_path.is_none());
        assert_eq!(
            prepared.args,
            runner.build_args("sample prompt", "claude-model", "C:/repo")
        );
    }

    #[test]
    fn prepare_cli_invocation_collects_stdin_payload_for_codex_runner() {
        let runner = CodexRunner;
        let prepared = prompting::prepare_cli_invocation(
            &runner,
            Path::new("codex"),
            "task-2",
            "sample prompt",
            "gpt-5.3-codex-spark",
            "C:/repo",
        )
        .expect("codex invocation should be prepared");

        assert_eq!(prepared.command_path, Path::new("codex"));
        assert_eq!(prepared.stdin_payload.as_deref(), Some("sample prompt"));
        assert!(prepared
            .response_capture_path
            .as_ref()
            .map(|path| path.to_string_lossy().contains("response-task-2"))
            .unwrap_or(false));
        assert_eq!(prepared.args[0], "exec");
        assert_eq!(prepared.args[1], "--output-last-message");
        assert!(prepared.args[2].contains("response-task-2"));
        assert_eq!(prepared.args[3], "--full-auto");
    }

    #[test]
    fn meta_output_files_are_treated_as_non_substantive_changes() {
        assert!(lifecycle::is_meta_output_file("walkthrough.md"));
        assert!(lifecycle::is_meta_output_file("./handoff.md"));
        assert!(lifecycle::is_meta_output_file("IMPLEMENTATION_PLAN.md"));
        assert!(!lifecycle::is_meta_output_file("docs/API_SPEC.md"));
        assert!(!lifecycle::is_meta_output_file("src/App.tsx"));
    }

    #[test]
    fn duplicate_output_is_suppressed_within_short_window() {
        let now = Instant::now();
        let mut recent_output = None;

        assert!(!lifecycle::should_suppress_duplicate_output_at(
            &mut recent_output,
            "YOLO mode is enabled.\r\n",
            now,
        ));
        assert!(lifecycle::should_suppress_duplicate_output_at(
            &mut recent_output,
            "YOLO mode is enabled.\n",
            now + Duration::from_millis(100),
        ));
        assert!(!lifecycle::should_suppress_duplicate_output_at(
            &mut recent_output,
            "YOLO mode is enabled.\n",
            now + Duration::from_secs(2),
        ));
    }

    #[test]
    fn build_task_prompt_mentions_completion_and_self_verification() {
        let task = Task {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            story_id: "story-1".to_string(),
            sequence_number: 1,
            title: "通知設定を実装する".to_string(),
            description: Some("メール通知のON/OFFを保存する".to_string()),
            status: "To Do".to_string(),
            sprint_id: None,
            archived: false,
            assignee_type: None,
            assigned_role_id: None,
            created_at: "2026-04-17 00:00:00".to_string(),
            updated_at: "2026-04-17 00:00:00".to_string(),
            priority: 3,
        };
        let role = TeamRole {
            id: "role-1".to_string(),
            name: "Lead Engineer".to_string(),
            system_prompt: "あなたは慎重な実装担当です。".to_string(),
            cli_type: "claude".to_string(),
            model: "claude-haiku-4-5".to_string(),
            avatar_image: None,
            sort_order: 0,
        };

        let prompt =
            prompting::build_task_prompt(&task, &role, Some("既存UIのトグル挙動を維持する"));

        assert!(prompt.contains("# 完了条件"));
        assert!(prompt.contains("# 自己検証"));
        assert!(prompt.contains("既存UIのトグル挙動を維持する"));
        assert!(prompt.contains("レビュー指摘"));
        assert!(prompt.contains("期待との差分"));
    }
}
