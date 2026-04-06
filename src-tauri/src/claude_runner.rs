use std::io::Read as IoRead;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Windows: std::process::Child を保持
/// Unix: portable-pty の PtyChild + Master/Slave を保持
///
/// trait object で統一し、kill / wait のみ公開する。
struct ClaudeSession {
    /// プロセス kill 用ハンドル
    killer: Box<dyn ProcessKiller + Send + Sync>,
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

pub struct ClaudeState {
    pub current_session: Arc<Mutex<Option<ClaudeSession>>>,
}

impl ClaudeState {
    pub fn new() -> Self {
        Self {
            current_session: Arc::new(Mutex::new(None)),
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
}

// ---------------------------------------------------------------------------
// Windows 実装: std::process::Command + piped stdout/stderr
//
// portable-pty の ConPTY は PSEUDOCONSOLE_WIN32_INPUT_MODE フラグにより
// cmd.exe がプレーンテキスト入力を受け付けないため、PTY ではなく
// パイプベースのプロセス実行を採用する（pty_manager.rs と同じ方針）。
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn spawn_claude_process(
    app_handle: &AppHandle,
    session_arc: Arc<Mutex<Option<ClaudeSession>>>,
    task_id: String,
    prompt: String,
    cwd: String,
) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("claude")
        .args([
            "-p", &prompt,
            "--permission-mode", "bypassPermissions",
            "--add-dir", &cwd,
            "--verbose",
        ])
        .current_dir(&cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| {
            let msg = if e.kind() == std::io::ErrorKind::NotFound {
                format!("claude CLI が見つかりません。Claude Code CLI がインストール済みで PATH に通っていることを確認してください。({})", e)
            } else {
                format!("プロセス起動失敗: {}", e)
            };
            log::error!("{}", msg);
            msg
        })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // セッション登録
    {
        let mut guard = session_arc.lock().map_err(|e| e.to_string())?;
        *guard = Some(ClaudeSession {
            killer: Box::new(StdChildKiller { child }),
        });
    }

    // stdout 読み取りスレッド
    let app_out = app_handle.clone();
    let tid_out = task_id.clone();
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

    // stderr 読み取りスレッド
    let app_err = app_handle.clone();
    let tid_err = task_id.clone();
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

    // 終了待機スレッド
    let session_wait = session_arc.clone();
    let app_wait = app_handle.clone();
    let tid_wait = task_id.clone();
    std::thread::spawn(move || {
        // stdout/stderr スレッドが先に EOF を受け取るのを少し待つ
        std::thread::sleep(std::time::Duration::from_millis(300));

        let mut guard = session_wait.lock().unwrap();
        if let Some(mut session) = guard.take() {
            let success = session.killer.wait_success();
            let _ = app_wait.emit(
                "claude_cli_exit",
                ClaudeExitPayload {
                    task_id: tid_wait,
                    success,
                    reason: if success {
                        "Completed successfully".into()
                    } else {
                        "Process exited with error".into()
                    },
                },
            );
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Unix 実装: portable-pty ベース（macOS・Linux PTY）
// TTY 検出を維持し ANSI カラー出力に対応する。
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
fn spawn_claude_process(
    app_handle: &AppHandle,
    session_arc: Arc<Mutex<Option<ClaudeSession>>>,
    task_id: String,
    prompt: String,
    cwd: String,
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

    let mut cmd = CommandBuilder::new("claude");
    cmd.args([
        "-p",
        &prompt,
        "--permission-mode",
        "bypassPermissions",
        "--add-dir",
        &cwd,
        "--verbose",
    ]);
    cmd.cwd(&cwd);
    for (key, val) in std::env::vars() {
        cmd.env(key, val);
    }
    cmd.env("TERM", "xterm-256color");

    let child = pair.slave.spawn_command(cmd).map_err(|e| {
        let msg = format!("プロセス起動失敗: {}", e);
        log::error!("{}", msg);
        msg
    })?;

    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    {
        let mut guard = session_arc.lock().map_err(|e| e.to_string())?;
        *guard = Some(ClaudeSession {
            killer: Box::new(PtyChildKiller {
                child,
                _master: pair.master,
                _slave: pair.slave,
            }),
        });
    }

    // PTY 読み取り + 終了検知スレッド
    let session_wait = session_arc.clone();
    let app_clone = app_handle.clone();
    let tid_clone = task_id.clone();
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

        let mut guard = session_wait.lock().unwrap();
        if let Some(mut session) = guard.take() {
            let success = session.killer.wait_success();
            let _ = app_clone.emit(
                "claude_cli_exit",
                ClaudeExitPayload {
                    task_id: tid_clone.clone(),
                    success,
                    reason: if success {
                        "Completed successfully".into()
                    } else {
                        "Process exited with error".into()
                    },
                },
            );
        }
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri コマンド
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn execute_claude_task(
    app_handle: AppHandle,
    state: tauri::State<'_, ClaudeState>,
    task_id: String,
    prompt: String,
    cwd: String,
) -> Result<(), String> {
    let session_arc = {
        let guard = state.current_session.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("A Claude process is already running.".into());
        }
        state.current_session.clone()
    };

    // ディレクトリ存在チェック
    let cwd_path = std::path::Path::new(&cwd);
    if !cwd_path.exists() || !cwd_path.is_dir() {
        let err_msg = format!(
            "エラー: 指定されたLocal Path ({}) が存在しません。Settingsで正しいパスを設定してください。",
            cwd
        );
        let _ = app_handle.emit(
            "claude_cli_output",
            ClaudeOutputPayload {
                task_id: task_id.clone(),
                output: format!("\x1b[31m{}\x1b[0m\r\n", err_msg),
            },
        );
        return Err(err_msg);
    }

    log::info!(
        "Spawning claude CLI: cwd={}, prompt_len={}",
        cwd,
        prompt.len()
    );

    spawn_claude_process(
        &app_handle,
        session_arc.clone(),
        task_id.clone(),
        prompt,
        cwd.clone(),
    )?;

    // タイムアウト (180秒)
    let session_arc_timeout = session_arc;
    let app_timeout = app_handle.clone();
    let tid_timeout = task_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(180)).await;
        let mut guard = session_arc_timeout.lock().unwrap();
        if let Some(mut session) = guard.take() {
            session.killer.kill();
            let _ = app_timeout.emit(
                "claude_cli_exit",
                ClaudeExitPayload {
                    task_id: tid_timeout,
                    success: false,
                    reason: "Timeout reached (180s). Process forcefully killed.".into(),
                },
            );
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn kill_claude_process(
    app_handle: AppHandle,
    state: tauri::State<'_, ClaudeState>,
    task_id: String,
) -> Result<(), String> {
    let mut guard = state.current_session.lock().map_err(|e| e.to_string())?;
    if let Some(mut session) = guard.take() {
        session.killer.kill();
        let _ = app_handle.emit(
            "claude_cli_exit",
            ClaudeExitPayload {
                task_id,
                success: false,
                reason: "Manually killed by user.".into(),
            },
        );
        Ok(())
    } else {
        Err("No active Claude process to kill.".into())
    }
}
