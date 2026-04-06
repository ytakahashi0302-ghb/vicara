use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// プラットフォーム共通の型定義
// ---------------------------------------------------------------------------

/// コマンド実行結果。フロントエンド向けに JSON シリアライズ可能。
///
/// - Windows: exit_code・stdout・stderr を完全分離取得。
/// - Unix/PTY: PTY が stdout/stderr を統合するため stderr は空文字列（stdout に混在）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// PTY セッションマネージャー。Tauri の State として登録される（Phase 4）。
pub struct PtyManager {
    sessions: Arc<Mutex<HashMap<String, PtySession>>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

// ---------------------------------------------------------------------------
// Windows 実装: ConPTY の PSEUDOCONSOLE_WIN32_INPUT_MODE フラグ（portable-pty
// の組み込み値）が有効な場合、cmd.exe は Win32 INPUT_RECORD 形式の入力を
// 期待し、プレーンテキストのコマンドを受け取らないため、PTY ベースの
// インタラクティブシェルではなく std::process::Command によるプロセス実行を採用。
// CWD はセッション状態として追跡し、cd コマンドで更新する。
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
struct PtySession {
    cwd: PathBuf,
    last_activity: Instant,
}

#[cfg(target_os = "windows")]
impl PtyManager {
    /// 新しいセッションを作成し、セッション ID を返す。
    pub async fn spawn_session(&self, cwd: &str) -> Result<String, String> {
        let session_id = Uuid::new_v4().to_string();
        let session = PtySession {
            cwd: PathBuf::from(cwd),
            last_activity: Instant::now(),
        };
        self.sessions
            .lock()
            .map_err(|e| format!("Mutex poisoned: {}", e))?
            .insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// 指定セッションでコマンドを実行し、ExecutionResult を返す。
    ///
    /// Windows では cmd.exe /C を使ったプロセス実行を採用。
    /// exit_code・stdout・stderr を完全分離して取得する。
    /// `cd <path>` コマンドを検出して CWD を更新する。
    pub async fn execute_command(
        &self,
        session_id: &str,
        command: &str,
    ) -> Result<ExecutionResult, String> {
        let cwd = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| format!("Mutex poisoned: {}", e))?;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;
            session.last_activity = Instant::now();
            session.cwd.clone()
        };

        let cmd_str = command.to_string();
        let cwd_clone = cwd.clone();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || -> Result<ExecutionResult, String> {
                let output = std::process::Command::new("cmd.exe")
                    .args(["/C", &cmd_str])
                    .current_dir(&cwd_clone)
                    .output()
                    .map_err(|e| format!("Failed to execute command: {}", e))?;

                Ok(ExecutionResult {
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                })
            }),
        )
        .await;

        let exec_result = match result {
            Ok(Ok(Ok(r))) => r,
            Ok(Ok(Err(e))) => return Err(e),
            Ok(Err(join_err)) => return Err(format!("Task panicked: {}", join_err)),
            Err(_) => return Err("Command timed out after 30 seconds".into()),
        };

        // cd コマンドによる CWD 変更を追跡
        let trimmed = command.trim();
        if trimmed.starts_with("cd ") || trimmed.starts_with("cd\t") {
            let dir = trimmed[3..].trim();
            let new_cwd = if std::path::Path::new(dir).is_absolute() {
                PathBuf::from(dir)
            } else {
                cwd.join(dir)
            };
            if new_cwd.is_dir() {
                let mut sessions = self
                    .sessions
                    .lock()
                    .map_err(|e| format!("Mutex poisoned: {}", e))?;
                if let Some(session) = sessions.get_mut(session_id) {
                    session.cwd = new_cwd;
                }
            }
        }

        Ok(exec_result)
    }

    /// 指定セッションを終了し、関連リソースを解放する。
    pub async fn kill_session(&self, session_id: &str) -> Result<(), String> {
        self.sessions
            .lock()
            .map_err(|e| format!("Mutex poisoned: {}", e))?
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Unix 実装: PTY ベースのインタラクティブシェルセッション。
// センチネル行に exit code を付加し（"; echo '__DONE__':$?"）、
// parse_sentinel_exit_code() で解析する。
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "windows"))]
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize, SlavePty};

#[cfg(not(target_os = "windows"))]
use std::io::{Read, Write};

/// 単一 PTY セッションの状態を保持する（Unix 専用）。
/// フィールド宣言順 = Drop 順（Rust の仕様）。
#[cfg(not(target_os = "windows"))]
struct PtySession {
    child: Box<dyn Child + Send + Sync>,
    _slave: Box<dyn SlavePty + Send>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    /// セッション作成時に1回だけ取得し、全コマンドで再利用する永続リーダー。
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    cwd: PathBuf,
    last_activity: Instant,
}

#[cfg(not(target_os = "windows"))]
fn build_shell_command(cwd: &str) -> CommandBuilder {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".into());
    let mut cmd = CommandBuilder::new(&shell);
    cmd.cwd(cwd);
    cmd
}

/// センチネルに exit code をコロン区切りで付加する。
/// 出力例: "__DONE_abc123__:0" (成功) / "__DONE_abc123__:1" (失敗)
#[cfg(not(target_os = "windows"))]
fn build_sentinel_command(command: &str, sentinel: &str) -> String {
    format!("{}; echo '{}':$?\n", command, sentinel)
}

/// センチネル行から exit code を解析する。
/// "__DONE_abc__:127" → 127
#[cfg(not(target_os = "windows"))]
fn parse_sentinel_exit_code(raw: &str, sentinel: &str) -> i32 {
    for line in raw.lines() {
        if line.contains(sentinel) {
            if let Some(colon_pos) = line.rfind(':') {
                if let Ok(code) = line[colon_pos + 1..].trim().parse::<i32>() {
                    return code;
                }
            }
        }
    }
    0 // フォールバック
}

#[cfg(not(target_os = "windows"))]
fn read_until_sentinel(
    reader: Arc<Mutex<Box<dyn Read + Send>>>,
    sentinel: &str,
) -> Result<String, String> {
    let mut guard = reader
        .lock()
        .map_err(|e| format!("Reader lock poisoned: {}", e))?;
    let mut buf = [0u8; 4096];
    let mut accumulated = String::new();

    loop {
        match guard.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                accumulated.push_str(&String::from_utf8_lossy(&buf[..n]));
                if accumulated.contains(sentinel) {
                    break;
                }
            }
            Err(e) => {
                if accumulated.contains(sentinel) {
                    break;
                }
                return Err(format!("PTY read error: {}", e));
            }
        }
    }

    Ok(accumulated)
}

#[cfg(not(target_os = "windows"))]
fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1B' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn clean_output(raw: &str, sentinel: &str) -> String {
    let cleaned = if let Some(pos) = raw.find(sentinel) {
        let line_start = raw[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        raw[..line_start].to_string()
    } else {
        raw.to_string()
    };
    strip_ansi_escapes(&cleaned)
}

#[cfg(not(target_os = "windows"))]
impl PtyManager {
    pub async fn spawn_session(&self, cwd: &str) -> Result<String, String> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;

        let cmd = build_shell_command(cwd);
        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

        let session_id = Uuid::new_v4().to_string();
        let session = PtySession {
            child,
            _slave: pair.slave,
            master: pair.master,
            writer,
            reader: Arc::new(Mutex::new(reader)),
            cwd: PathBuf::from(cwd),
            last_activity: Instant::now(),
        };

        self.sessions
            .lock()
            .map_err(|e| format!("Mutex poisoned: {}", e))?
            .insert(session_id.clone(), session);

        Ok(session_id)
    }

    pub async fn execute_command(
        &self,
        session_id: &str,
        command: &str,
    ) -> Result<ExecutionResult, String> {
        let sentinel = format!("__DONE_{}__", Uuid::new_v4().to_string().replace('-', ""));
        let full_command = build_sentinel_command(command, &sentinel);

        let reader_arc = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|e| format!("Mutex poisoned: {}", e))?;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Session not found: {}", session_id))?;

            session
                .writer
                .write_all(full_command.as_bytes())
                .map_err(|e| e.to_string())?;
            session.writer.flush().map_err(|e| e.to_string())?;
            session.last_activity = Instant::now();
            Arc::clone(&session.reader)
        };

        let sentinel_clone = sentinel.clone();
        let read_result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tokio::task::spawn_blocking(move || read_until_sentinel(reader_arc, &sentinel_clone)),
        )
        .await;

        let raw_output = match read_result {
            Ok(Ok(Ok(output))) => output,
            Ok(Ok(Err(e))) => return Err(format!("Read error: {}", e)),
            Ok(Err(join_err)) => return Err(format!("Task panicked: {}", join_err)),
            Err(_timeout) => return Err("Command timed out after 30 seconds".into()),
        };

        let exit_code = parse_sentinel_exit_code(&raw_output, &sentinel);
        let stdout = clean_output(&raw_output, &sentinel);

        Ok(ExecutionResult {
            exit_code,
            stdout,
            stderr: String::new(), // PTY は stdout/stderr を統合。stderr は stdout に混在
        })
    }

    pub async fn kill_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|e| format!("Mutex poisoned: {}", e))?;
        let mut session = sessions
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        let _ = session.child.kill();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// プラットフォーム共通メソッド
// ---------------------------------------------------------------------------

impl PtyManager {
    /// アイドル状態（最終アクティビティから idle_threshold 以上経過）のセッションを自動 kill する。
    /// lib.rs の setup ブロックで定期的に呼び出す（例: 5分間隔、30分アイドルで kill）。
    pub async fn cleanup_idle_sessions(&self, idle_threshold: std::time::Duration) {
        let idle_ids: Vec<String> = {
            let sessions = match self.sessions.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            sessions
                .iter()
                .filter(|(_, s)| s.last_activity.elapsed() > idle_threshold)
                .map(|(id, _)| id.clone())
                .collect()
        };
        for id in idle_ids {
            if self.kill_session(&id).await.is_ok() {
                log::info!("PTY: auto-killed idle session {}", id);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ユニットテスト
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// echo コマンドの実行と出力取得の基本テスト
    #[tokio::test]
    async fn test_spawn_and_echo() {
        let mgr = PtyManager::new();
        let id = mgr.spawn_session(".").await.expect("spawn_session failed");
        let result = mgr
            .execute_command(&id, "echo hello_pty_test")
            .await
            .expect("execute_command failed");
        assert!(
            result.stdout.contains("hello_pty_test"),
            "Expected 'hello_pty_test' in stdout, got: {:?}",
            result
        );
        mgr.kill_session(&id).await.expect("kill_session failed");
    }

    /// セッション kill 後に execute_command が失敗することを確認
    #[tokio::test]
    async fn test_kill_then_execute_fails() {
        let mgr = PtyManager::new();
        let id = mgr.spawn_session(".").await.expect("spawn_session failed");
        mgr.kill_session(&id).await.expect("kill_session failed");
        let result = mgr.execute_command(&id, "echo test").await;
        assert!(
            result.is_err(),
            "Expected error for dead session, got: {:?}",
            result
        );
    }

    /// 同一セッションで複数コマンドを順次実行できることを確認
    #[tokio::test]
    async fn test_sequential_commands() {
        let mgr = PtyManager::new();
        let id = mgr.spawn_session(".").await.expect("spawn_session failed");
        let out1 = mgr
            .execute_command(&id, "echo first")
            .await
            .expect("first command failed");
        let out2 = mgr
            .execute_command(&id, "echo second")
            .await
            .expect("second command failed");
        assert!(out1.stdout.contains("first"), "out1: {:?}", out1);
        assert!(out2.stdout.contains("second"), "out2: {:?}", out2);
        mgr.kill_session(&id).await.expect("kill_session failed");
    }

    /// Windows: exit_code が正しく取得できることを確認
    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_exit_code() {
        let mgr = PtyManager::new();
        let id = mgr.spawn_session(".").await.expect("spawn_session failed");

        // 成功コマンド
        let ok = mgr
            .execute_command(&id, "echo ok")
            .await
            .expect("echo failed");
        assert_eq!(ok.exit_code, 0, "echo should exit 0, got: {:?}", ok);

        // 失敗コマンド（存在しない実行ファイル）
        let fail = mgr
            .execute_command(&id, "nonexistent_cmd_xyz_12345")
            .await
            .expect("execute_command itself should not err");
        assert_ne!(
            fail.exit_code, 0,
            "bad cmd should exit non-0, got: {:?}",
            fail
        );

        mgr.kill_session(&id).await.expect("kill_session failed");
    }
}
