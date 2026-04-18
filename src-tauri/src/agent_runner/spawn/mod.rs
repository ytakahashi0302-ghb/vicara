use crate::{
    cli_runner::{self, CliRunner, CliType},
    db,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;

mod completion;
mod timeout;

#[cfg(not(target_os = "windows"))]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

use timeout::spawn_timeout_guard;

#[cfg(not(target_os = "windows"))]
use unix::spawn_agent_process;
#[cfg(target_os = "windows")]
use windows::spawn_agent_process;

use super::{
    lifecycle::resolve_cli_command_path,
    prompting::{cleanup_temp_file, create_prompt_file},
    session::{build_generic_session_info, remove_session_entry, reserve_session_slot},
    ActiveAgentSession, AgentSessionEntry, AgentState, AgentUsageContext,
};

pub(super) async fn execute_prompt_request(
    app_handle: AppHandle,
    runner: &dyn CliRunner,
    cli_command_path: PathBuf,
    sessions_arc: Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    session_info: ActiveAgentSession,
    prompt: String,
    cwd: String,
    usage_context: AgentUsageContext,
) -> Result<(), String> {
    let cwd_path = std::path::Path::new(&cwd);
    if !cwd_path.exists() || !cwd_path.is_dir() {
        remove_session_entry(&sessions_arc, &session_info.task_id);
        let err_msg = format!(
            "エラー: 指定されたLocal Path ({}) が存在しません。Settingsで正しいパスを設定してください。",
            cwd
        );
        super::lifecycle::emit_agent_output(
            &app_handle,
            &session_info.task_id,
            format!("\x1b[31m{}\x1b[0m\r\n", err_msg),
        );
        return Err(err_msg);
    }

    let prompt_file_path = match create_prompt_file(&session_info.task_id, &prompt, cwd_path) {
        Ok(path) => path,
        Err(error) => {
            remove_session_entry(&sessions_arc, &session_info.task_id);
            return Err(error);
        }
    };

    if let Err(error) = spawn_agent_process(
        &app_handle,
        runner,
        &cli_command_path,
        sessions_arc.clone(),
        session_info.clone(),
        prompt_file_path.clone(),
        cwd,
        usage_context.clone(),
    ) {
        remove_session_entry(&sessions_arc, &session_info.task_id);
        cleanup_temp_file(&prompt_file_path);
        return Err(error);
    }

    spawn_timeout_guard(
        app_handle,
        sessions_arc,
        session_info.task_id.clone(),
        session_info,
        usage_context,
    );

    Ok(())
}

pub(crate) async fn execute_cli_prompt_task(
    app_handle: AppHandle,
    state: tauri::State<'_, AgentState>,
    task_id: String,
    prompt: String,
    cwd: String,
    cli_type: CliType,
    model: String,
    project_id: Option<String>,
) -> Result<(), String> {
    let max_concurrent_agents = db::get_max_concurrent_agents_value(&app_handle).await?;
    let usage_context = AgentUsageContext {
        source_kind: "scaffold_ai".to_string(),
        project_id,
        sprint_id: None,
        db_task_id: None,
    };
    let runner = cli_runner::create_runner(&cli_type)?;
    let cli_command_path = resolve_cli_command_path(runner.as_ref())?;
    let session_info =
        build_generic_session_info(&task_id, runner.as_ref(), runner.resolve_model(&model))?;
    let sessions_arc = reserve_session_slot(&state, session_info.clone(), max_concurrent_agents)?;

    execute_prompt_request(
        app_handle,
        runner.as_ref(),
        cli_command_path,
        sessions_arc,
        session_info,
        prompt,
        cwd,
        usage_context,
    )
    .await
}
