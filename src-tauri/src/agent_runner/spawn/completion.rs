use tauri::AppHandle;

use super::super::{
    lifecycle::{build_exit_payload, persist_agent_retro_run_for_session, record_cli_usage_event},
    prompting::cleanup_temp_file,
    AgentSession,
};

fn process_exit_reason(success: bool) -> String {
    if success {
        "Completed successfully".to_string()
    } else {
        "Process exited with error".to_string()
    }
}

pub(super) async fn finalize_completed_session(
    app_handle: &AppHandle,
    session: AgentSession,
    task_id: &str,
    success: bool,
) -> super::super::AgentExitPayload {
    cleanup_temp_file(&session.temp_file_path);
    let reason = process_exit_reason(success);

    if let Some(response_capture_path) = session.response_capture_path.as_ref() {
        persist_agent_retro_run_for_session(
            app_handle,
            &session.info,
            &session.usage_context,
            &session.retro_capture,
            Some(response_capture_path),
            success,
            reason.clone(),
        )
        .await;
        cleanup_temp_file(response_capture_path);
    } else {
        persist_agent_retro_run_for_session(
            app_handle,
            &session.info,
            &session.usage_context,
            &session.retro_capture,
            None,
            success,
            reason.clone(),
        )
        .await;
    }

    record_cli_usage_event(
        app_handle,
        &session.info,
        &session.usage_context,
        success,
        reason.clone(),
    )
    .await;
    build_exit_payload(app_handle, task_id, success, reason).await
}
