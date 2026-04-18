use crate::{agent_retro, cli_runner::CliRunner};
use std::collections::HashMap;
use std::io::Read as IoRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use super::super::{
    lifecycle::{
        build_cli_not_found_message, create_stdout_parser, emit_agent_output,
        emit_parsed_stdout_chunks, flush_stdout_parser, preview_output_chunk_for_log,
        promote_session_to_running, should_suppress_duplicate_output,
    },
    prompting::{build_cli_prompt_from_file, prepare_cli_invocation, spawn_stdin_payload_writer},
    session::remove_session_entry,
    ActiveAgentSession, AgentSession, AgentSessionEntry, AgentUsageContext, ProcessKiller,
    RecentOutputChunk, AGENT_CLI_EXIT_EVENT,
};
use super::completion::finalize_completed_session;

struct StdChildKiller {
    child: std::process::Child,
}

impl ProcessKiller for StdChildKiller {
    fn kill(&mut self) {
        let _ = self.child.kill();
    }

    fn wait_success(&mut self) -> bool {
        self.child.wait().map(|s| s.success()).unwrap_or(false)
    }
}

pub(super) fn spawn_agent_process(
    app_handle: &AppHandle,
    runner: &dyn CliRunner,
    cli_command_path: &Path,
    sessions_arc: Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    session_info: ActiveAgentSession,
    prompt_file_path: PathBuf,
    cwd: String,
    usage_context: AgentUsageContext,
) -> Result<(), String> {
    let cli_prompt = build_cli_prompt_from_file(&prompt_file_path);
    let prepared = prepare_cli_invocation(
        runner,
        cli_command_path,
        &session_info.task_id,
        &cli_prompt,
        &session_info.model,
        &cwd,
    )?;
    let mut command = Command::new(&prepared.command_path);
    command
        .args(&prepared.args)
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if prepared.stdin_payload.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    for (key, value) in runner.env_vars() {
        command.env(key, value);
    }
    let mut child = command.spawn().map_err(|e| {
        let msg = if e.kind() == std::io::ErrorKind::NotFound {
            format!("{} ({})", build_cli_not_found_message(runner), e)
        } else {
            format!("プロセス起動失敗 ({}): {}", runner.display_name(), e)
        };
        log::error!("{}", msg);
        msg
    })?;

    if let Some(payload) = prepared.stdin_payload {
        let stdin = child.stdin.take().ok_or_else(|| {
            format!(
                "{} の stdin を確保できず、prompt を渡せませんでした。",
                runner.display_name()
            )
        })?;
        spawn_stdin_payload_writer(
            stdin,
            payload,
            runner.display_name().to_string(),
            session_info.task_id.clone(),
        );
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let recent_output = Arc::new(Mutex::new(None::<RecentOutputChunk>));
    let retro_capture = Arc::new(Mutex::new(agent_retro::AgentRetroCapture::new(
        runner.cli_type(),
    )));

    let mut running_info = session_info.clone();
    running_info.status = "Running".to_string();

    let session = AgentSession {
        info: running_info.clone(),
        temp_file_path: prompt_file_path,
        response_capture_path: prepared.response_capture_path.clone(),
        usage_context: usage_context.clone(),
        retro_capture: retro_capture.clone(),
        killer: Box::new(StdChildKiller { child }),
    };
    promote_session_to_running(app_handle, &sessions_arc, &session_info.task_id, session)?;

    let app_out = app_handle.clone();
    let tid_out = session_info.task_id.clone();
    let recent_output_out = recent_output.clone();
    let retro_capture_out = retro_capture.clone();
    let stdout_parser = create_stdout_parser(runner);
    if let Some(mut reader) = stdout {
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            let mut stdout_parser = stdout_parser;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        log::info!("stdout reader: EOF for task {}", tid_out);
                        break;
                    }
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buf[..n]).to_string();
                        if let Ok(mut capture) = retro_capture_out.lock() {
                            capture.ingest_chunk(&output);
                        }
                        let preview = preview_output_chunk_for_log(&output);
                        log::debug!(
                            "[STREAM][stdout] task={} received {} bytes preview={:?}",
                            tid_out,
                            n,
                            preview
                        );
                        if should_suppress_duplicate_output(&recent_output_out, &output) {
                            log::debug!(
                                "[STREAM][stdout] task={} suppressed duplicate chunk preview={:?}",
                                tid_out,
                                preview
                            );
                            continue;
                        }
                        log::debug!(
                            "[STREAM][stdout] task={} emitting chunk preview={:?}",
                            tid_out,
                            preview
                        );
                        emit_parsed_stdout_chunks(
                            &app_out,
                            &tid_out,
                            stdout_parser.as_mut(),
                            &output,
                        );
                    }
                    Err(e) => {
                        log::warn!("stdout reader: error for task {}: {}", tid_out, e);
                        break;
                    }
                }
            }
            flush_stdout_parser(&app_out, &tid_out, stdout_parser.as_mut());
        });
    }

    let app_err = app_handle.clone();
    let tid_err = session_info.task_id.clone();
    let recent_output_err = recent_output;
    let retro_capture_err = retro_capture;
    if let Some(mut reader) = stderr {
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buf[..n]).to_string();
                        if let Ok(mut capture) = retro_capture_err.lock() {
                            capture.ingest_chunk(&output);
                        }
                        let preview = preview_output_chunk_for_log(&output);
                        log::debug!(
                            "[STREAM][stderr] task={} received {} bytes preview={:?}",
                            tid_err,
                            n,
                            preview
                        );
                        if should_suppress_duplicate_output(&recent_output_err, &output) {
                            log::debug!(
                                "[STREAM][stderr] task={} suppressed duplicate chunk preview={:?}",
                                tid_err,
                                preview
                            );
                            continue;
                        }
                        log::debug!(
                            "[STREAM][stderr] task={} emitting chunk preview={:?}",
                            tid_err,
                            preview
                        );
                        emit_agent_output(&app_err, &tid_err, output);
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let app_wait = app_handle.clone();
    let sessions_wait = sessions_arc.clone();
    let tid_wait = session_info.task_id.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));

        if let Some(AgentSessionEntry::Running(mut session)) =
            remove_session_entry(&sessions_wait, &tid_wait)
        {
            let success = session.killer.wait_success();
            let exit_payload = tauri::async_runtime::block_on(finalize_completed_session(
                &app_wait, session, &tid_wait, success,
            ));
            let _ = app_wait.emit(AGENT_CLI_EXIT_EVENT, exit_payload);
        }
    });

    Ok(())
}
