use crate::{agent_retro, cli_runner::CliRunner};
use portable_pty::{
    native_pty_system, Child as PtyChild, CommandBuilder, MasterPty, PtySize, SlavePty,
};
use std::collections::HashMap;
use std::io::Read as IoRead;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use super::super::{
    lifecycle::{
        create_stdout_parser, emit_parsed_stdout_chunks, flush_stdout_parser,
        promote_session_to_running,
    },
    prompting::{build_cli_prompt_from_file, prepare_cli_invocation, spawn_stdin_payload_writer},
    session::remove_session_entry,
    ActiveAgentSession, AgentSession, AgentSessionEntry, AgentUsageContext, ProcessKiller,
    AGENT_CLI_EXIT_EVENT,
};
use super::completion::finalize_completed_session;

struct PtyChildKiller {
    child: Box<dyn PtyChild + Send>,
    _master: Box<dyn MasterPty + Send>,
    _slave: Box<dyn SlavePty + Send>,
}

impl ProcessKiller for PtyChildKiller {
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
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let cli_prompt = build_cli_prompt_from_file(&prompt_file_path);
    let prepared = prepare_cli_invocation(
        runner,
        cli_command_path,
        &session_info.task_id,
        &cli_prompt,
        &session_info.model,
        &cwd,
    )?;
    let mut cmd = CommandBuilder::new(prepared.command_path.to_string_lossy().to_string());
    cmd.args(prepared.args.iter().map(String::as_str));
    cmd.cwd(&cwd);
    for (key, val) in std::env::vars() {
        cmd.env(key, val);
    }
    for (key, val) in runner.env_vars() {
        cmd.env(key, val);
    }
    cmd.env("TERM", "xterm-256color");

    let child = pair.slave.spawn_command(cmd).map_err(|e| {
        let msg = format!("プロセス起動失敗 ({}): {}", runner.display_name(), e);
        log::error!("{}", msg);
        msg
    })?;

    if let Some(payload) = prepared.stdin_payload {
        let writer = pair.master.take_writer().map_err(|error| {
            format!(
                "{} の stdin writer を確保できず、prompt を渡せませんでした: {}",
                runner.display_name(),
                error
            )
        })?;
        spawn_stdin_payload_writer(
            writer,
            payload,
            runner.display_name().to_string(),
            session_info.task_id.clone(),
        );
    }

    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
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
        killer: Box::new(PtyChildKiller {
            child,
            _master: pair.master,
            _slave: pair.slave,
        }),
    };
    promote_session_to_running(app_handle, &sessions_arc, &session_info.task_id, session)?;

    let app_clone = app_handle.clone();
    let sessions_wait = sessions_arc.clone();
    let tid_clone = session_info.task_id.clone();
    let retro_capture_clone = retro_capture.clone();
    let stdout_parser = create_stdout_parser(runner);
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 1024];
        let mut stdout_parser = stdout_parser;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    log::info!("PTY reader: EOF for task {}", tid_clone);
                    break;
                }
                Ok(n) => {
                    let output = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Ok(mut capture) = retro_capture_clone.lock() {
                        capture.ingest_chunk(&output);
                    }
                    emit_parsed_stdout_chunks(
                        &app_clone,
                        &tid_clone,
                        stdout_parser.as_mut(),
                        &output,
                    );
                }
                Err(e) => {
                    log::warn!("PTY reader: error for task {}: {}", tid_clone, e);
                    break;
                }
            }
        }
        flush_stdout_parser(&app_clone, &tid_clone, stdout_parser.as_mut());

        std::thread::sleep(std::time::Duration::from_millis(200));

        if let Some(AgentSessionEntry::Running(mut session)) =
            remove_session_entry(&sessions_wait, &tid_clone)
        {
            let success = session.killer.wait_success();
            let exit_payload = tauri::async_runtime::block_on(finalize_completed_session(
                &app_clone, session, &tid_clone, success,
            ));
            let _ = app_clone.emit(AGENT_CLI_EXIT_EVENT, exit_payload);
        }
    });

    Ok(())
}
