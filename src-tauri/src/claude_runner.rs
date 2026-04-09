use crate::{
    cli_detection,
    cli_runner::{self, CliRunner, CliType},
    db, llm_observability, worktree,
};
use std::collections::HashMap;
use std::fs;
use std::io::Read as IoRead;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Windows: std::process::Child を保持
/// Unix: portable-pty の PtyChild + Master/Slave を保持
///
/// trait object で統一し、kill / wait のみ公開する。
struct AgentSession {
    info: ActiveAgentSession,
    temp_file_path: PathBuf,
    /// プロセス kill 用ハンドル
    killer: Box<dyn ProcessKiller + Send + Sync>,
}

#[derive(Clone, serde::Serialize)]
pub struct ActiveAgentSession {
    task_id: String,
    task_title: String,
    role_name: String,
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

// --- Windows: std::process::Child ラッパー ---
#[cfg(target_os = "windows")]
struct StdChildKiller {
    child: std::process::Child,
}

#[cfg(target_os = "windows")]
impl ProcessKiller for StdChildKiller {
    fn kill(&mut self) {
        let _ = self.child.kill();
    }
    fn wait_success(&mut self) -> bool {
        self.child.wait().map(|s| s.success()).unwrap_or(false)
    }
}

// --- Unix: portable-pty の PtyChild ラッパー ---
#[cfg(not(target_os = "windows"))]
use portable_pty::{
    native_pty_system, Child as PtyChild, CommandBuilder, MasterPty, PtySize, SlavePty,
};

#[cfg(not(target_os = "windows"))]
struct PtyChildKiller {
    child: Box<dyn PtyChild + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    _slave: Box<dyn SlavePty + Send>,
}

#[cfg(not(target_os = "windows"))]
impl ProcessKiller for PtyChildKiller {
    fn kill(&mut self) {
        let _ = self.child.kill();
    }
    fn wait_success(&mut self) -> bool {
        self.child.wait().map(|s| s.success()).unwrap_or(false)
    }
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

// ---------------------------------------------------------------------------
// イベントペイロード
// ---------------------------------------------------------------------------

#[derive(Clone, serde::Serialize)]
struct ClaudeOutputPayload {
    task_id: String,
    output: String,
}

#[derive(Clone, serde::Serialize)]
struct ClaudeExitPayload {
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn current_timestamp_millis() -> Result<i64, String> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
}

fn cleanup_temp_file(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!(
                "failed to remove temporary agent prompt file {}: {}",
                path.display(),
                error
            );
        }
    }
}

fn sanitize_for_filename(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "task".to_string()
    } else {
        sanitized
    }
}

fn build_task_prompt(
    task: &db::Task,
    role: &db::TeamRole,
    additional_context: Option<&str>,
) -> String {
    let description = task.description.as_deref().unwrap_or("特になし");
    let extra_context = additional_context
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\n# 追加コンテキスト\n{}\n", value))
        .unwrap_or_default();

    format!(
        "あなたは {} です。\n{}\n\n# タスク名\n{}\n\n# 詳細\n{}\n{}# 作業指示\n- タスクのゴールを達成するための実装を行ってください。\n- 必要なファイル変更を加えてください。\n- 作業を終える前に変更内容が意図通りか自己検証してください。\n- 完了したら終了してください。\n",
        role.name.trim(),
        role.system_prompt.trim(),
        task.title.trim(),
        description.trim(),
        extra_context
    )
}

fn create_prompt_file(task_id: &str, prompt: &str) -> Result<PathBuf, String> {
    let timestamp = current_timestamp_millis()?;

    let file_name = format!(
        "vicara-agent-{}-{}.md",
        sanitize_for_filename(task_id),
        timestamp
    );
    let path = std::env::temp_dir().join(file_name);

    fs::write(&path, prompt).map_err(|e| {
        format!(
            "CLI 実行用の一時ファイル作成に失敗しました ({}): {}",
            path.display(),
            e
        )
    })?;

    Ok(path)
}

fn build_cli_prompt_from_file(prompt_file_path: &Path) -> String {
    format!(
        "以下のファイルに記載された役割とタスク指示を読み込み、それに従って開発を実行してください。ファイルパス: {}",
        prompt_file_path.display()
    )
}

fn get_session_summary(entry: &AgentSessionEntry) -> ActiveAgentSession {
    match entry {
        AgentSessionEntry::Starting(info) => info.clone(),
        AgentSessionEntry::Running(session) => session.info.clone(),
    }
}

fn remove_session_entry(
    sessions_arc: &Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    task_id: &str,
) -> Option<AgentSessionEntry> {
    match sessions_arc.lock() {
        Ok(mut sessions) => sessions.remove(task_id),
        Err(_) => None,
    }
}

fn reserve_session_slot(
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

fn build_generic_session_info(
    task_id: &str,
    runner: &dyn CliRunner,
) -> Result<ActiveAgentSession, String> {
    Ok(ActiveAgentSession {
        task_id: task_id.to_string(),
        task_title: task_id.to_string(),
        role_name: "Scaffold AI".to_string(),
        model: runner.default_model().to_string(),
        started_at: current_timestamp_millis()?,
        status: "Starting".to_string(),
    })
}

fn build_task_session_info(
    task: &db::Task,
    role: &db::TeamRole,
    runner: &dyn CliRunner,
) -> Result<ActiveAgentSession, String> {
    Ok(ActiveAgentSession {
        task_id: task.id.clone(),
        task_title: task.title.clone(),
        role_name: role.name.clone(),
        model: runner.resolve_model(&role.model),
        started_at: current_timestamp_millis()?,
        status: "Starting".to_string(),
    })
}

fn build_cli_not_found_message(runner: &dyn CliRunner) -> String {
    format!(
        "{} ({}) が見つかりません。`{}` でインストールし、PATH に追加してください。",
        runner.display_name(),
        runner.command_name(),
        runner.install_hint()
    )
}

async fn ensure_cli_is_available(runner: &dyn CliRunner) -> Result<(), String> {
    let installed_clis = cli_detection::detect_installed_clis().await?;
    let is_installed = installed_clis
        .into_iter()
        .find(|cli| cli.name == runner.command_name())
        .map(|cli| cli.installed)
        .unwrap_or(false);

    if is_installed {
        Ok(())
    } else {
        Err(build_cli_not_found_message(runner))
    }
}

fn promote_session_to_running(
    app_handle: &AppHandle,
    sessions_arc: &Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    task_id: &str,
    session: AgentSession,
) -> Result<(), String> {
    let started_payload = session.info.clone();

    let mut sessions = sessions_arc.lock().map_err(|e| e.to_string())?;
    sessions.insert(task_id.to_string(), AgentSessionEntry::Running(session));
    drop(sessions);

    if let Err(error) = app_handle.emit("claude_cli_started", started_payload) {
        log::warn!(
            "failed to emit claude_cli_started for {}: {}",
            task_id,
            error
        );
    }

    Ok(())
}

async fn build_exit_payload(
    app_handle: &AppHandle,
    task_id: &str,
    success: bool,
    reason: String,
) -> ClaudeExitPayload {
    if !success {
        return ClaudeExitPayload {
            task_id: task_id.to_string(),
            success,
            reason,
            new_status: None,
        };
    }

    match db::get_task_by_id(app_handle, task_id).await {
        Ok(Some(task)) => {
            if task.status == "Review" {
                ClaudeExitPayload {
                    task_id: task_id.to_string(),
                    success: true,
                    reason,
                    new_status: Some("Review".to_string()),
                }
            } else {
                match db::update_task_status(
                    app_handle.clone(),
                    task_id.to_string(),
                    "Review".to_string(),
                )
                .await
                {
                    Ok(_) => ClaudeExitPayload {
                        task_id: task_id.to_string(),
                        success: true,
                        reason,
                        new_status: Some("Review".to_string()),
                    },
                    Err(error) => ClaudeExitPayload {
                        task_id: task_id.to_string(),
                        success: false,
                        reason: format!(
                            "CLI の処理は完了しましたが、タスクを Review に更新できませんでした: {}",
                            error
                        ),
                        new_status: None,
                    },
                }
            }
        }
        Ok(None) => ClaudeExitPayload {
            task_id: task_id.to_string(),
            success: true,
            reason,
            new_status: None,
        },
        Err(error) => ClaudeExitPayload {
            task_id: task_id.to_string(),
            success: false,
            reason: format!(
                "CLI の処理は完了しましたが、タスク状態の確認に失敗しました: {}",
                error
            ),
            new_status: None,
        },
    }
}

async fn record_claude_cli_usage_event(
    app_handle: &AppHandle,
    session_info: &ActiveAgentSession,
    usage_context: &AgentUsageContext,
    success: bool,
    reason: String,
) {
    let completed_at = current_timestamp_millis().unwrap_or(session_info.started_at);

    if let Err(error) = llm_observability::record_claude_cli_usage(
        app_handle,
        llm_observability::ClaudeCliUsageRecordInput {
            project_id: usage_context.project_id.clone(),
            task_id: usage_context.db_task_id.clone(),
            sprint_id: usage_context.sprint_id.clone(),
            source_kind: usage_context.source_kind.clone(),
            model: session_info.model.clone(),
            request_started_at: session_info.started_at,
            request_completed_at: completed_at,
            success,
            error_message: (!success).then_some(reason),
        },
    )
    .await
    {
        log::warn!(
            "Failed to record Claude CLI usage for session {}: {}",
            session_info.task_id,
            error
        );
    }
}

async fn execute_prompt_request(
    app_handle: AppHandle,
    runner: &dyn CliRunner,
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
        let _ = app_handle.emit(
            "claude_cli_output",
            ClaudeOutputPayload {
                task_id: session_info.task_id.clone(),
                output: format!("\x1b[31m{}\x1b[0m\r\n", err_msg),
            },
        );
        return Err(err_msg);
    }

    let prompt_file_path = match create_prompt_file(&session_info.task_id, &prompt) {
        Ok(path) => path,
        Err(error) => {
            remove_session_entry(&sessions_arc, &session_info.task_id);
            return Err(error);
        }
    };

    if let Err(error) = spawn_agent_process(
        &app_handle,
        runner,
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

    let app_timeout = app_handle.clone();
    let sessions_arc_timeout = sessions_arc.clone();
    let timeout_task_id = session_info.task_id.clone();
    let timeout_session_info = session_info.clone();
    let timeout_usage_context = usage_context.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(180)).await;

        if let Some(entry) = remove_session_entry(&sessions_arc_timeout, &timeout_task_id) {
            match entry {
                AgentSessionEntry::Running(mut session) => {
                    session.killer.kill();
                    cleanup_temp_file(&session.temp_file_path);
                }
                AgentSessionEntry::Starting(_) => {}
            }

            record_claude_cli_usage_event(
                &app_timeout,
                &timeout_session_info,
                &timeout_usage_context,
                false,
                "Timeout reached (180s). Process forcefully killed.".to_string(),
            )
            .await;

            let _ = app_timeout.emit(
                "claude_cli_exit",
                ClaudeExitPayload {
                    task_id: timeout_task_id,
                    success: false,
                    reason: "Timeout reached (180s). Process forcefully killed.".into(),
                    new_status: None,
                },
            );
        }
    });

    Ok(())
}

pub async fn execute_claude_prompt_task(
    app_handle: AppHandle,
    state: tauri::State<'_, AgentState>,
    task_id: String,
    prompt: String,
    cwd: String,
    project_id: Option<String>,
) -> Result<(), String> {
    let max_concurrent_agents = db::get_max_concurrent_agents_value(&app_handle).await?;
    let usage_context = AgentUsageContext {
        source_kind: "scaffold_ai".to_string(),
        project_id,
        sprint_id: None,
        db_task_id: None,
    };
    let runner = cli_runner::create_runner(&CliType::Claude)?;
    ensure_cli_is_available(runner.as_ref()).await?;
    let session_info = build_generic_session_info(&task_id, runner.as_ref())?;
    let sessions_arc = reserve_session_slot(&state, session_info.clone(), max_concurrent_agents)?;

    execute_prompt_request(
        app_handle,
        runner.as_ref(),
        sessions_arc,
        session_info,
        prompt,
        cwd,
        usage_context,
    )
    .await
}

// ---------------------------------------------------------------------------
// Windows 実装: std::process::Command + piped stdout/stderr
//
// portable-pty の ConPTY は PSEUDOCONSOLE_WIN32_INPUT_MODE フラグにより
// cmd.exe がプレーンテキスト入力を受け付けないため、PTY ではなく
// パイプベースのプロセス実行を採用する（pty_manager.rs と同じ方針）。
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn spawn_agent_process(
    app_handle: &AppHandle,
    runner: &dyn CliRunner,
    sessions_arc: Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    session_info: ActiveAgentSession,
    prompt_file_path: PathBuf,
    cwd: String,
    usage_context: AgentUsageContext,
) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let cli_prompt = build_cli_prompt_from_file(&prompt_file_path);
    let args = runner.build_args(&cli_prompt, &session_info.model, &cwd);
    let mut command = Command::new(runner.command_name());
    command
        .args(&args)
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
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

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let mut running_info = session_info.clone();
    running_info.status = "Running".to_string();

    let session = AgentSession {
        info: running_info.clone(),
        temp_file_path: prompt_file_path,
        killer: Box::new(StdChildKiller { child }),
    };
    promote_session_to_running(app_handle, &sessions_arc, &session_info.task_id, session)?;

    let app_out = app_handle.clone();
    let tid_out = session_info.task_id.clone();
    if let Some(mut reader) = stdout {
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        log::info!("stdout reader: EOF for task {}", tid_out);
                        break;
                    }
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = app_out.emit(
                            "claude_cli_output",
                            ClaudeOutputPayload {
                                task_id: tid_out.clone(),
                                output,
                            },
                        );
                    }
                    Err(e) => {
                        log::warn!("stdout reader: error for task {}: {}", tid_out, e);
                        break;
                    }
                }
            }
        });
    }

    let app_err = app_handle.clone();
    let tid_err = session_info.task_id.clone();
    if let Some(mut reader) = stderr {
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buf[..n]).to_string();
                        let _ = app_err.emit(
                            "claude_cli_output",
                            ClaudeOutputPayload {
                                task_id: tid_err.clone(),
                                output,
                            },
                        );
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let app_wait = app_handle.clone();
    let sessions_wait = sessions_arc.clone();
    let tid_wait = session_info.task_id.clone();
    let wait_session_info = session_info.clone();
    let wait_usage_context = usage_context.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));

        if let Some(AgentSessionEntry::Running(mut session)) =
            remove_session_entry(&sessions_wait, &tid_wait)
        {
            let success = session.killer.wait_success();
            cleanup_temp_file(&session.temp_file_path);
            let reason = if success {
                "Completed successfully".to_string()
            } else {
                "Process exited with error".to_string()
            };
            tauri::async_runtime::block_on(record_claude_cli_usage_event(
                &app_wait,
                &wait_session_info,
                &wait_usage_context,
                success,
                reason.clone(),
            ));
            let exit_payload = tauri::async_runtime::block_on(build_exit_payload(
                &app_wait, &tid_wait, success, reason,
            ));
            let _ = app_wait.emit("claude_cli_exit", exit_payload);
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Unix 実装: portable-pty ベース（macOS・Linux PTY）
// TTY 検出を維持し ANSI カラー出力に対応する。
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
fn spawn_agent_process(
    app_handle: &AppHandle,
    runner: &dyn CliRunner,
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
    let args = runner.build_args(&cli_prompt, &session_info.model, &cwd);
    let mut cmd = CommandBuilder::new(runner.command_name());
    cmd.args(args.iter().map(String::as_str));
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

    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    let mut running_info = session_info.clone();
    running_info.status = "Running".to_string();

    let session = AgentSession {
        info: running_info.clone(),
        temp_file_path: prompt_file_path,
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
    let wait_session_info = session_info.clone();
    let wait_usage_context = usage_context.clone();
    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    log::info!("PTY reader: EOF for task {}", tid_clone);
                    break;
                }
                Ok(n) => {
                    let output = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_clone.emit(
                        "claude_cli_output",
                        ClaudeOutputPayload {
                            task_id: tid_clone.clone(),
                            output,
                        },
                    );
                }
                Err(e) => {
                    log::warn!("PTY reader: error for task {}: {}", tid_clone, e);
                    break;
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(200));

        if let Some(AgentSessionEntry::Running(mut session)) =
            remove_session_entry(&sessions_wait, &tid_clone)
        {
            let success = session.killer.wait_success();
            cleanup_temp_file(&session.temp_file_path);
            let reason = if success {
                "Completed successfully".to_string()
            } else {
                "Process exited with error".to_string()
            };
            tauri::async_runtime::block_on(record_claude_cli_usage_event(
                &app_clone,
                &wait_session_info,
                &wait_usage_context,
                success,
                reason.clone(),
            ));
            let exit_payload = tauri::async_runtime::block_on(build_exit_payload(
                &app_clone, &tid_clone, success, reason,
            ));
            let _ = app_clone.emit("claude_cli_exit", exit_payload);
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri コマンド
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_active_claude_sessions(
    state: tauri::State<'_, AgentState>,
) -> Result<Vec<ActiveAgentSession>, String> {
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let mut result: Vec<ActiveAgentSession> = sessions.values().map(get_session_summary).collect();
    result.sort_by_key(|session| session.started_at);
    Ok(result)
}

#[tauri::command]
pub async fn execute_claude_task(
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
    ensure_cli_is_available(runner.as_ref()).await?;

    if role.system_prompt.trim().is_empty() {
        return Err("担当ロールのシステムプロンプトが未設定です。".to_string());
    }

    let max_concurrent_agents = db::get_max_concurrent_agents_value(&app_handle).await?;
    let session_info = build_task_session_info(&task, &role, runner.as_ref())?;
    let sessions_arc = reserve_session_slot(&state, session_info.clone(), max_concurrent_agents)?;

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

    db::upsert_worktree_record(
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
    .await?;

    let prompt = build_task_prompt(&task, &role, additional_context.as_deref());
    let usage_context = AgentUsageContext {
        source_kind: "task_execution".to_string(),
        project_id: Some(task.project_id.clone()),
        sprint_id: task.sprint_id.clone(),
        db_task_id: Some(task.id.clone()),
    };
    let result = execute_prompt_request(
        app_handle.clone(),
        runner.as_ref(),
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
pub async fn kill_claude_process(
    app_handle: AppHandle,
    state: tauri::State<'_, AgentState>,
    task_id: String,
) -> Result<(), String> {
    let entry = remove_session_entry(&state.sessions, &task_id)
        .ok_or_else(|| format!("task_id={} に紐づく CLI プロセスは存在しません。", task_id))?;

    match entry {
        AgentSessionEntry::Running(mut session) => {
            session.killer.kill();
            cleanup_temp_file(&session.temp_file_path);
        }
        AgentSessionEntry::Starting(_) => {}
    }

    app_handle
        .emit(
            "claude_cli_exit",
            ClaudeExitPayload {
                task_id,
                success: false,
                reason: "Manually killed by user.".into(),
                new_status: None,
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}
