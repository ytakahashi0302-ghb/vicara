use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use super::super::{
    lifecycle::{persist_agent_retro_run_for_session, record_cli_usage_event},
    prompting::cleanup_temp_file,
    session::remove_session_entry,
    ActiveAgentSession, AgentSessionEntry, AgentUsageContext, AGENT_CLI_EXIT_EVENT,
};

pub(super) fn spawn_timeout_guard(
    app_timeout: AppHandle,
    sessions_arc_timeout: Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    timeout_task_id: String,
    timeout_session_info: ActiveAgentSession,
    timeout_usage_context: AgentUsageContext,
) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(180)).await;

        if let Some(entry) = remove_session_entry(&sessions_arc_timeout, &timeout_task_id) {
            match entry {
                AgentSessionEntry::Running(mut session) => {
                    session.killer.kill();
                    cleanup_temp_file(&session.temp_file_path);
                    if let Some(response_capture_path) = session.response_capture_path.as_ref() {
                        persist_agent_retro_run_for_session(
                            &app_timeout,
                            &session.info,
                            &session.usage_context,
                            &session.retro_capture,
                            Some(response_capture_path),
                            false,
                            "Timeout reached (180s). Process forcefully killed.".to_string(),
                        )
                        .await;
                        cleanup_temp_file(response_capture_path);
                    } else {
                        persist_agent_retro_run_for_session(
                            &app_timeout,
                            &session.info,
                            &session.usage_context,
                            &session.retro_capture,
                            None,
                            false,
                            "Timeout reached (180s). Process forcefully killed.".to_string(),
                        )
                        .await;
                    }
                }
                AgentSessionEntry::Starting(_) => {}
            }

            record_cli_usage_event(
                &app_timeout,
                &timeout_session_info,
                &timeout_usage_context,
                false,
                "Timeout reached (180s). Process forcefully killed.".to_string(),
            )
            .await;

            let _ = app_timeout.emit(
                AGENT_CLI_EXIT_EVENT,
                super::super::AgentExitPayload {
                    task_id: timeout_task_id,
                    success: false,
                    reason: "Timeout reached (180s). Process forcefully killed.".into(),
                    new_status: None,
                },
            );
        }
    });
}
