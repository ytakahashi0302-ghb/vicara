use crate::{cli_runner::CliRunner, db};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::{ActiveAgentSession, AgentSessionEntry, AgentState};

pub(super) fn get_session_summary(entry: &AgentSessionEntry) -> ActiveAgentSession {
    match entry {
        AgentSessionEntry::Starting(info) => info.clone(),
        AgentSessionEntry::Running(session) => session.info.clone(),
    }
}

pub(super) fn remove_session_entry(
    sessions_arc: &Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    task_id: &str,
) -> Option<AgentSessionEntry> {
    match sessions_arc.lock() {
        Ok(mut sessions) => sessions.remove(task_id),
        Err(_) => None,
    }
}

pub(super) fn reserve_session_slot(
    state: &tauri::State<'_, AgentState>,
    session_info: ActiveAgentSession,
    max_concurrent_agents: i32,
) -> Result<Arc<Mutex<HashMap<String, AgentSessionEntry>>>, String> {
    let max_concurrent_agents = max_concurrent_agents.max(1) as usize;
    let mut sessions = state.sessions.lock().map_err(|e| e.to_string())?;

    if sessions.contains_key(&session_info.task_id) {
        return Err(format!(
            "task_id={} の CLI プロセスはすでに起動中です。",
            session_info.task_id
        ));
    }

    if sessions.len() >= max_concurrent_agents {
        return Err(format!(
            "最大並行稼働数 ({}) に達しているため、新しいタスクは起動できません。",
            max_concurrent_agents
        ));
    }

    sessions.insert(
        session_info.task_id.clone(),
        AgentSessionEntry::Starting(session_info),
    );
    drop(sessions);

    Ok(state.sessions.clone())
}

pub(super) fn build_generic_session_info(
    task_id: &str,
    runner: &dyn CliRunner,
    model: String,
) -> Result<ActiveAgentSession, String> {
    Ok(ActiveAgentSession {
        task_id: task_id.to_string(),
        task_title: task_id.to_string(),
        role_name: "Scaffold AI".to_string(),
        cli_type: runner.cli_type().as_str().to_string(),
        model,
        started_at: super::lifecycle::current_timestamp_millis()?,
        status: "Starting".to_string(),
    })
}

pub(super) fn build_task_session_info(
    task: &db::Task,
    role: &db::TeamRole,
    runner: &dyn CliRunner,
) -> Result<ActiveAgentSession, String> {
    Ok(ActiveAgentSession {
        task_id: task.id.clone(),
        task_title: task.title.clone(),
        role_name: role.name.clone(),
        cli_type: runner.cli_type().as_str().to_string(),
        model: runner.resolve_model(&role.model),
        started_at: super::lifecycle::current_timestamp_millis()?,
        status: "Starting".to_string(),
    })
}
