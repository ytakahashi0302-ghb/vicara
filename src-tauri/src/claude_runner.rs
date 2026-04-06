use std::io::Read as IoRead;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use portable_pty::{native_pty_system, Child as PtyChild, CommandBuilder, MasterPty, PtySize, SlavePty};

// ---------------------------------------------------------------------------
// State: 全プラットフォーム共通で portable-pty を使用
// ---------------------------------------------------------------------------

struct ClaudeSession {
    child: Box<dyn PtyChild + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    _slave: Box<dyn SlavePty + Send>,
    prompt_file: Option<std::path::PathBuf>,
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
// プロンプトを一時ファイルに書き出す
// ---------------------------------------------------------------------------

fn write_prompt_to_tempfile(prompt: &str) -> Result<std::path::PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let file_name = format!("claude_prompt_{}.md", uuid::Uuid::new_v4());
    let file_path = temp_dir.join(file_name);
    std::fs::write(&file_path, prompt).map_err(|e| format!("Failed to write prompt file: {}", e))?;
    Ok(file_path)
}

// ---------------------------------------------------------------------------
// 共通実装: portable-pty ベース（Windows ConPTY / macOS・Linux PTY）
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn execute_claude_task(
    app_handle: AppHandle,
    state: tauri::State<'_, ClaudeState>,
    task_id: String,
    prompt: String,
    cwd: String,
) -> Result<(), String> {
    let mut session_guard = state.current_session.lock().map_err(|e| e.to_string())?;
    if session_guard.is_some() {
        return Err("A Claude process is already running.".into());
    }

    // ディレクトリ存在チェック
    let cwd_path = std::path::Path::new(&cwd);
    if !cwd_path.exists() || !cwd_path.is_dir() {
        let err_msg = format!(
            "エラー: 指定されたLocal Path ({}) が存在しません。Settingsで正しいパスを設定してください。",
            cwd
        );
        let _ = app_handle.emit("claude_cli_output", ClaudeOutputPayload {
            task_id: task_id.clone(),
            output: format!("\x1b[31m{}\x1b[0m\r\n", err_msg),
        });
        return Err(err_msg);
    }

    // プロンプトを一時ファイルに書き出し（argv エスケープ問題を回避）
    let prompt_file = write_prompt_to_tempfile(&prompt)?;
    let prompt_file_str = prompt_file.to_string_lossy().to_string();

    // PTY を開く
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    // コマンド構築: Windows は cmd.exe /C 経由、Unix は直接 claude
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = CommandBuilder::new("cmd.exe");
        c.args([
            "/C", "claude",
            "--file", &prompt_file_str,
            "--permission-mode", "bypassPermissions",
            "--add-dir", &cwd,
            "--verbose",
        ]);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = CommandBuilder::new("claude");
        c.args([
            "--file", &prompt_file_str,
            "--permission-mode", "bypassPermissions",
            "--add-dir", &cwd,
            "--verbose",
        ]);
        c
    };

    cmd.cwd(&cwd);
    for (key, val) in std::env::vars() {
        cmd.env(key, val);
    }
    cmd.env("TERM", "xterm-256color");

    let child = match pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => {
            // 一時ファイルをクリーンアップ
            let _ = std::fs::remove_file(&prompt_file);
            let err_msg = format!("CRITICAL: spawn_command failed: {}", e);
            let _ = app_handle.emit("claude_cli_output", ClaudeOutputPayload {
                task_id: task_id.clone(),
                output: format!("\x1b[31m{}\x1b[0m\r\n", err_msg),
            });
            return Err(err_msg);
        }
    };

    let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    *session_guard = Some(ClaudeSession {
        child,
        _master: pair.master,
        _slave: pair.slave,
        prompt_file: Some(prompt_file),
    });
    drop(session_guard);

    // PTY 読み取り + 終了検知スレッド
    let session_arc = state.current_session.clone();
    let app_clone = app_handle.clone();
    let tid_clone = task_id.clone();

    std::thread::spawn(move || {
        let mut reader = reader;
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let output = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_clone.emit("claude_cli_output", ClaudeOutputPayload {
                        task_id: tid_clone.clone(),
                        output,
                    });
                }
                Err(_) => break,
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(200));

        let mut guard = session_arc.lock().unwrap();
        if let Some(mut session) = guard.take() {
            let success = match session.child.wait() {
                Ok(status) => status.success(),
                Err(_) => false,
            };
            // 一時ファイルをクリーンアップ
            if let Some(ref path) = session.prompt_file {
                let _ = std::fs::remove_file(path);
            }
            let _ = app_clone.emit("claude_cli_exit", ClaudeExitPayload {
                task_id: tid_clone.clone(),
                success,
                reason: if success {
                    "Completed successfully".into()
                } else {
                    "Process exited with error".into()
                },
            });
        }
    });

    // タイムアウト (180秒)
    let session_arc_timeout = state.current_session.clone();
    let app_timeout = app_handle.clone();
    let tid_timeout = task_id.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(180)).await;
        let mut guard = session_arc_timeout.lock().unwrap();
        if let Some(mut session) = guard.take() {
            let _ = session.child.kill();
            // 一時ファイルをクリーンアップ
            if let Some(ref path) = session.prompt_file {
                let _ = std::fs::remove_file(path);
            }
            let _ = app_timeout.emit("claude_cli_exit", ClaudeExitPayload {
                task_id: tid_timeout,
                success: false,
                reason: "Timeout reached (180s). Process forcefully killed.".into(),
            });
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
        let _ = session.child.kill();
        // 一時ファイルをクリーンアップ
        if let Some(ref path) = session.prompt_file {
            let _ = std::fs::remove_file(path);
        }
        let _ = app_handle.emit("claude_cli_exit", ClaudeExitPayload {
            task_id,
            success: false,
            reason: "Manually killed by user.".into(),
        });
        Ok(())
    } else {
        Err("No active Claude process to kill.".into())
    }
}
