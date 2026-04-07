use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

const DEFAULT_PREVIEW_COMMAND: &str = "npm run dev";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewServerInfo {
    pub task_id: String,
    pub port: u16,
    pub pid: u32,
    pub worktree_path: String,
    pub command: String,
    pub url: String,
}

struct PreviewServer {
    task_id: String,
    port: u16,
    pid: u32,
    worktree_path: String,
    command: String,
    child: Child,
}

impl PreviewServer {
    fn info(&self) -> PreviewServerInfo {
        PreviewServerInfo {
            task_id: self.task_id.clone(),
            port: self.port,
            pid: self.pid,
            worktree_path: self.worktree_path.clone(),
            command: self.command.clone(),
            url: format!("http://127.0.0.1:{}", self.port),
        }
    }
}

pub struct PreviewState {
    servers: Mutex<HashMap<String, PreviewServer>>,
}

impl PreviewState {
    pub fn new() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_info(&self, task_id: &str) -> Result<Option<PreviewServerInfo>, String> {
        let servers = self
            .servers
            .lock()
            .map_err(|e| format!("PreviewState lock error: {}", e))?;
        Ok(servers.get(task_id).map(|server| server.info()))
    }

    pub fn stop_server(&self, task_id: &str) -> Result<Option<PreviewServerInfo>, String> {
        let mut servers = self
            .servers
            .lock()
            .map_err(|e| format!("PreviewState lock error: {}", e))?;

        if let Some(mut server) = servers.remove(task_id) {
            let info = server.info();
            stop_child_process(&mut server.child)?;
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    fn insert(&self, server: PreviewServer) -> Result<PreviewServerInfo, String> {
        let info = server.info();
        let mut servers = self
            .servers
            .lock()
            .map_err(|e| format!("PreviewState lock error: {}", e))?;
        servers.insert(server.task_id.clone(), server);
        Ok(info)
    }

    fn stop_all(&self) {
        if let Ok(mut servers) = self.servers.lock() {
            for (_, mut server) in servers.drain() {
                let _ = stop_child_process(&mut server.child);
            }
        }
    }
}

impl Drop for PreviewState {
    fn drop(&mut self) {
        self.stop_all();
    }
}

fn stop_child_process(child: &mut Child) -> Result<(), String> {
    match child.try_wait().map_err(|e| e.to_string())? {
        Some(_) => Ok(()),
        None => {
            child
                .kill()
                .map_err(|e| format!("プレビューサーバー停止に失敗しました: {}", e))?;
            let _ = child.wait();
            Ok(())
        }
    }
}

fn find_available_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("空きポート取得に失敗しました: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("空きポート取得に失敗しました: {}", e))?
        .port();
    drop(listener);
    Ok(port)
}

fn normalize_preview_command(command: Option<String>) -> String {
    command
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_PREVIEW_COMMAND.to_string())
}

fn spawn_preview_process(worktree_path: &Path, command: &str, port: u16) -> Result<Child, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        Command::new("cmd")
            .args(["/C", command])
            .current_dir(worktree_path)
            .env("PORT", port.to_string())
            .env("HOST", "127.0.0.1")
            .env("BROWSER", "none")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("プレビューサーバー起動に失敗しました: {}", e))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .args(["-lc", command])
            .current_dir(worktree_path)
            .env("PORT", port.to_string())
            .env("HOST", "127.0.0.1")
            .env("BROWSER", "none")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("プレビューサーバー起動に失敗しました: {}", e))
    }
}

pub fn start_preview_for_task(
    preview_state: &PreviewState,
    task_id: &str,
    worktree_path: &Path,
    command: Option<String>,
) -> Result<PreviewServerInfo, String> {
    if let Some(existing) = preview_state.get_info(task_id)? {
        return Ok(existing);
    }

    let preview_command = normalize_preview_command(command);
    let port = find_available_port()?;
    let mut child = spawn_preview_process(worktree_path, &preview_command, port)?;

    std::thread::sleep(Duration::from_millis(1200));
    if let Some(status) = child
        .try_wait()
        .map_err(|e| format!("プレビューサーバー状態確認に失敗しました: {}", e))?
    {
        return Err(format!(
            "プレビューサーバーがすぐに終了しました (exit code: {:?})。コマンド `{}` を確認してください。",
            status.code(),
            preview_command
        ));
    }

    let pid = child.id();
    preview_state.insert(PreviewServer {
        task_id: task_id.to_string(),
        port,
        pid,
        worktree_path: worktree_path.to_string_lossy().to_string(),
        command: preview_command,
        child,
    })
}

pub fn open_preview_in_browser(app_handle: &AppHandle, port: u16) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}", port);
    app_handle
        .opener()
        .open_url(url, None::<String>)
        .map_err(|e| format!("ブラウザでプレビューを開けませんでした: {}", e))?;
    Ok(())
}

pub fn open_local_path(app_handle: &AppHandle, path: &Path) -> Result<String, String> {
    let resolved_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    app_handle
        .opener()
        .open_path(resolved_path.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| format!("ローカルファイルを開けませんでした: {}", e))?;

    Ok(resolved_path.to_string_lossy().to_string())
}
