use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

const DEFAULT_PREVIEW_COMMAND: &str = "npm run dev";

/// dev サーバーの出力からアクセス可能な URL が現れるまで待機する最大時間。
/// Vite の初回起動（依存関係の事前バンドル等）に時間がかかる場合を想定して
/// 余裕を持たせている。
const URL_DETECTION_TIMEOUT: Duration = Duration::from_secs(30);

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
    url: String,
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
            url: self.url.clone(),
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

#[cfg(target_os = "windows")]
fn is_process_running(pid: u32) -> Result<bool, String> {
    let script = format!(
        "$p = Get-Process -Id {} -ErrorAction SilentlyContinue; if ($null -eq $p) {{ exit 1 }} else {{ exit 0 }}",
        pid
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(|e| format!("preview プロセス状態確認に失敗しました: {}", e))?;
    Ok(status.success())
}

#[cfg(not(target_os = "windows"))]
fn is_process_running(pid: u32) -> Result<bool, String> {
    let status = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map_err(|e| format!("preview プロセス状態確認に失敗しました: {}", e))?;
    Ok(status.success())
}

#[cfg(target_os = "windows")]
fn stop_process_tree_by_pid(pid: u32) -> Result<bool, String> {
    if !is_process_running(pid)? {
        return Ok(false);
    }

    let output = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .map_err(|e| format!("preview プロセスツリー停止に失敗しました: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "taskkill が失敗しました。".to_string()
        };
        return Err(format!(
            "preview プロセスツリー停止に失敗しました: {}",
            detail
        ));
    }

    Ok(true)
}

#[cfg(not(target_os = "windows"))]
fn stop_process_tree_by_pid(pid: u32) -> Result<bool, String> {
    if !is_process_running(pid)? {
        return Ok(false);
    }

    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map_err(|e| format!("preview プロセス停止に失敗しました: {}", e))?;
    if !status.success() {
        return Err(format!(
            "preview プロセス停止に失敗しました: kill -TERM {}",
            pid
        ));
    }

    Ok(true)
}

fn stop_child_process(child: &mut Child) -> Result<(), String> {
    if stop_process_tree_by_pid(child.id())? {
        let _ = child.wait();
        return Ok(());
    }

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

pub fn stop_server_or_fallback_pid(
    preview_state: &PreviewState,
    task_id: &str,
    fallback_pid: Option<u32>,
) -> Result<bool, String> {
    if preview_state.stop_server(task_id)?.is_some() {
        return Ok(true);
    }

    if let Some(pid) = fallback_pid {
        return stop_process_tree_by_pid(pid);
    }

    Ok(false)
}

pub(crate) fn normalize_preview_command(command: Option<String>) -> String {
    command
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_PREVIEW_COMMAND.to_string())
}

/// stdout から 1 行読み、Vite が出力する `Local:` / `Network:` ラベル付き URL を抽出する。
///
/// Vite は起動時に ANSI カラーコードつきで以下のような行を出力する:
/// ```text
///   ➜  Local:   http://localhost:5173/
///   ➜  Network: http://192.168.10.10:5173/
/// ```
///
/// 対象プロジェクトがバックエンド API と同時起動する構成（concurrently や
/// monorepo）のとき、バックエンドの `Server running at http://localhost:3000`
/// のような素のログ行を誤って拾うと、Vite ではないポートを開いてしまう。
/// そのため Vite 固有のラベル (`Local:` / `Network:`) が前置された URL
/// だけを受け付けるよう正規表現を厳格化している。
///
/// 優先順位:
///   1. `Local:` + `127.0.0.1`
///   2. `Local:` + `localhost`
///   3. `Local:` + その他ホスト
///   4. `Network:` + `127.0.0.1`
///   5. `Network:` + `localhost`
///   6. `Network:` + その他ホスト（=LAN IP、localhost が解決できない環境向け）
fn extract_url_from_line(line: &str, re: &Regex) -> Option<(String, u16)> {
    // ANSI エスケープシーケンスを除去（色コードの間に "Local" が挟まれていても動くようにする）
    let ansi_re = Regex::new(r"\x1B\[[0-9;]*[a-zA-Z]").ok()?;
    let cleaned = ansi_re.replace_all(line, "");

    let mut best: Option<(u8, String, u16)> = None; // (priority, url, port)
    for caps in re.captures_iter(&cleaned) {
        let label = caps.get(1)?.as_str();
        let host = caps.get(2)?.as_str();
        let port: u16 = caps.get(3)?.as_str().parse().ok()?;

        // Local を Network より強く優先（同一起動でどちらも出るため）
        let label_priority: u8 = if label.eq_ignore_ascii_case("Local") {
            0
        } else {
            10
        };
        let host_priority: u8 = match host {
            "127.0.0.1" => 0,
            "localhost" => 1,
            _ => 2,
        };
        let priority = label_priority + host_priority;

        let url = format!("http://{}:{}", host, port);
        if best.as_ref().map_or(true, |(p, _, _)| priority < *p) {
            best = Some((priority, url, port));
        }
    }

    best.map(|(_, url, port)| (url, port))
}

/// Vite が出力する `Local:` / `Network:` ラベル付きの URL 行にのみマッチする正規表現。
///
/// - `(?i)` で大文字小文字を無視（Vite の出力は `Local` だが保険）
/// - ラベルと URL の間は空白（半角スペース/タブ）を 0 個以上許容
/// - ホスト部は英数/ドット/ハイフンのみ（IP / DNS 名）
/// - ポート 2〜5 桁（1桁ポートのような誤マッチを避ける）
///
/// ⚠️ ラベル無しの素の `http://...:port` は意図的にマッチさせない。
/// これによりバックエンド API の起動ログ（`Server running at http://...`）を
/// 誤抽出することを防ぐ。
fn url_regex() -> Regex {
    Regex::new(r"(?i)\b(Local|Network)\s*:\s*https?://([A-Za-z0-9\.\-]+):(\d{2,5})")
        .expect("valid regex")
}

fn spawn_preview_process(worktree_path: &Path, command: &str) -> Result<Child, String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        Command::new("cmd")
            .args(["/C", command])
            .current_dir(worktree_path)
            .env("BROWSER", "none")
            .env("FORCE_COLOR", "0")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("プレビューサーバー起動に失敗しました: {}", e))
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh")
            .args(["-lc", command])
            .current_dir(worktree_path)
            .env("BROWSER", "none")
            .env("FORCE_COLOR", "0")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("プレビューサーバー起動に失敗しました: {}", e))
    }
}

/// ストリーム (stdout/stderr) を行単位で読み、URL を検出したら
/// mpsc 経由で最初の 1 件を通知する。通知後もストリームは読み続けて
/// パイプバッファが埋まって子プロセスがブロックするのを防ぐ。
fn spawn_stream_reader<R>(reader: R, stream_name: &'static str, sender: mpsc::Sender<(String, u16)>)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let re = url_regex();
        let buf = BufReader::new(reader);
        let mut notified = false;
        for line in buf.lines().map_while(Result::ok) {
            // 生行をそのままログ出力（ANSI含む）。プレビュー起動が
            // 期待通りのポートで立ち上がっているかを切り分けるため、
            // デバッグビルド/リリースビルドともに出力する。
            log::info!(target: "vicara::preview", "[{}] {}", stream_name, line);
            eprintln!("[vicara::preview::{}] {}", stream_name, line);

            if !notified {
                if let Some((url, port)) = extract_url_from_line(&line, &re) {
                    log::info!(
                        target: "vicara::preview",
                        "URL 抽出成功: {} (port {}) from [{}]",
                        url,
                        port,
                        stream_name
                    );
                    eprintln!("[vicara::preview] URL extracted: {} (port {})", url, port);
                    if sender.send((url, port)).is_ok() {
                        notified = true;
                    }
                }
            }
            // 通知後も読み続けて drain（BrokenPipe 回避のため）
        }
    });
}

/// `BufReader<ChildStdout>` を型引数で使えない環境もあるため、具象型で wrap する。
fn spawn_stdout_reader(stdout: ChildStdout, sender: mpsc::Sender<(String, u16)>) {
    spawn_stream_reader(stdout, "stdout", sender);
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

    let invocation_command = normalize_preview_command(command);
    let mut child = spawn_preview_process(worktree_path, &invocation_command)?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "プレビューサーバーの stdout を取得できませんでした".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "プレビューサーバーの stderr を取得できませんでした".to_string())?;

    let (tx, rx) = mpsc::channel::<(String, u16)>();
    spawn_stdout_reader(stdout, tx.clone());
    spawn_stream_reader(stderr, "stderr", tx);

    log::info!(
        target: "vicara::preview",
        "spawn: task_id={} cwd={} cmd=`{}`",
        task_id,
        worktree_path.to_string_lossy(),
        invocation_command
    );
    eprintln!(
        "[vicara::preview] spawn: task_id={} cwd={} cmd=`{}`",
        task_id,
        worktree_path.to_string_lossy(),
        invocation_command
    );

    // URL が出力されるまでポーリングしつつ、子プロセスの早期終了も検知する。
    let deadline = Instant::now() + URL_DETECTION_TIMEOUT;
    let (url, port) = loop {
        // 子プロセスが早期終了していないか確認
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("プレビューサーバー状態確認に失敗しました: {}", e))?
        {
            return Err(format!(
                "プレビューサーバーがすぐに終了しました (exit code: {:?})。`{}` の内容を確認してください。",
                status.code(),
                invocation_command
            ));
        }

        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(detected) => break detected,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "プレビューサーバーの起動 URL を {}秒以内に検出できませんでした。`{}` が `http://host:port` 形式の URL を出力することを確認してください。",
                        URL_DETECTION_TIMEOUT.as_secs(),
                        invocation_command
                    ));
                }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "プレビューサーバーの出力ストリームが閉じられたため URL を検出できませんでした。"
                        .to_string(),
                );
            }
        }
    };

    let pid = child.id();
    preview_state.insert(PreviewServer {
        task_id: task_id.to_string(),
        port,
        pid,
        worktree_path: worktree_path.to_string_lossy().to_string(),
        command: invocation_command,
        url,
        child,
    })
}

pub fn open_preview_in_browser(app_handle: &AppHandle, url: &str) -> Result<(), String> {
    log::info!(target: "vicara::preview", "open_preview_in_browser: url={}", url);
    eprintln!("[vicara::preview] open_preview_in_browser: url={}", url);
    app_handle
        .opener()
        .open_url(url.to_string(), None::<String>)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_preview_command_uses_default_when_none() {
        assert_eq!(normalize_preview_command(None), DEFAULT_PREVIEW_COMMAND);
    }

    #[test]
    fn normalize_preview_command_trims_input() {
        assert_eq!(
            normalize_preview_command(Some("  pnpm dev  ".to_string())),
            "pnpm dev"
        );
    }

    #[test]
    fn normalize_preview_command_falls_back_on_empty_string() {
        assert_eq!(
            normalize_preview_command(Some("   ".to_string())),
            DEFAULT_PREVIEW_COMMAND
        );
    }

    #[test]
    fn stop_server_or_fallback_pid_returns_false_without_targets() {
        let state = PreviewState::new();
        let stopped =
            stop_server_or_fallback_pid(&state, "missing-task", None).expect("should not fail");
        assert!(!stopped);
    }

    #[test]
    fn extract_url_picks_loopback_when_present() {
        let re = url_regex();
        let line = "  ➜  Local:   http://127.0.0.1:5173/";
        let (url, port) = extract_url_from_line(line, &re).expect("URL should be extracted");
        assert_eq!(url, "http://127.0.0.1:5173");
        assert_eq!(port, 5173);
    }

    #[test]
    fn extract_url_picks_localhost_when_loopback_missing() {
        let re = url_regex();
        let line = "  ➜  Local:   http://localhost:3000/";
        let (url, port) = extract_url_from_line(line, &re).expect("URL should be extracted");
        assert_eq!(url, "http://localhost:3000");
        assert_eq!(port, 3000);
    }

    #[test]
    fn extract_url_prefers_loopback_over_network() {
        let re = url_regex();
        // 同一行に複数 URL（現実にはまれだが保険として）
        let line = "Network: http://192.168.10.10:5173/ Local: http://127.0.0.1:5173/";
        let (url, _) = extract_url_from_line(line, &re).expect("URL should be extracted");
        assert_eq!(url, "http://127.0.0.1:5173");
    }

    #[test]
    fn extract_url_falls_back_to_network_when_only_network_present() {
        let re = url_regex();
        let line = "  ➜  Network: http://192.168.10.10:5173/";
        let (url, port) = extract_url_from_line(line, &re).expect("URL should be extracted");
        assert_eq!(url, "http://192.168.10.10:5173");
        assert_eq!(port, 5173);
    }

    #[test]
    fn extract_url_handles_ansi_color_codes() {
        let re = url_regex();
        // Vite は ANSI 色コードを混ぜて出力する
        let line =
            "  \x1B[32m➜\x1B[39m  \x1B[1mLocal\x1B[22m:   \x1B[36mhttp://127.0.0.1:5173/\x1B[39m";
        let (url, port) = extract_url_from_line(line, &re).expect("URL should be extracted");
        assert_eq!(url, "http://127.0.0.1:5173");
        assert_eq!(port, 5173);
    }

    #[test]
    fn extract_url_returns_none_for_unrelated_line() {
        let re = url_regex();
        let line = "VITE v5.0.0  ready in 342 ms";
        assert!(extract_url_from_line(line, &re).is_none());
    }

    #[test]
    fn extract_url_ignores_too_short_port() {
        let re = url_regex();
        // 1 桁ポートは正規表現で除外
        let line = "Local: http://127.0.0.1:5/";
        assert!(extract_url_from_line(line, &re).is_none());
    }

    #[test]
    fn extract_url_ignores_backend_server_log() {
        let re = url_regex();
        // バックエンド API の起動ログ（Local: / Network: ラベル無し）は無視する
        let line = "Server running at http://localhost:3000";
        assert!(
            extract_url_from_line(line, &re).is_none(),
            "バックエンドの素のURLログを誤抽出してはいけない"
        );
    }

    #[test]
    fn extract_url_ignores_listening_on_log() {
        let re = url_regex();
        let line = "[backend] Listening on http://127.0.0.1:8080/";
        assert!(extract_url_from_line(line, &re).is_none());
    }

    #[test]
    fn extract_url_ignores_url_without_label_prefix() {
        let re = url_regex();
        // URL が素のままで現れる行は全て無視
        let line = "  Please open http://localhost:3000 to continue.";
        assert!(extract_url_from_line(line, &re).is_none());
    }

    #[test]
    fn extract_url_picks_vite_local_when_backend_also_logs() {
        let re = url_regex();
        // 同一ストリームにバックエンドと Vite が混在するケース
        // （本来は行単位で処理するが、念のため同一行検証）
        let line = "Server running at http://localhost:3000   ➜  Local:   http://localhost:5173/";
        let (url, port) = extract_url_from_line(line, &re).expect("Vite の Local URL を抽出");
        assert_eq!(url, "http://localhost:5173");
        assert_eq!(port, 5173);
    }

    #[test]
    fn extract_url_matches_network_label_for_lan_ip() {
        let re = url_regex();
        // Vite Network 行: localhost が解決できない環境用のフォールバック
        let line = "  ➜  Network: http://192.168.10.10:5173/";
        let (url, port) = extract_url_from_line(line, &re).expect("Network URL を抽出");
        assert_eq!(url, "http://192.168.10.10:5173");
        assert_eq!(port, 5173);
    }
}
