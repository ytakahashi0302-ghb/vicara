use crate::pty_manager::{ExecutionResult, PtyManager};
use tauri::State;

/// 新しいシェルセッションを起動し、セッション ID を返す。
///
/// # 引数
/// * `cwd` — 作業ディレクトリのパス（絶対パス推奨）
///
/// # 戻り値
/// セッション ID（UUID 文字列）
#[tauri::command]
pub async fn pty_spawn(
    state: State<'_, PtyManager>,
    cwd: String,
) -> Result<String, String> {
    state.spawn_session(&cwd).await
}

/// 指定セッションでコマンドを実行し、結果を返す。
///
/// # 引数
/// * `session_id` — `pty_spawn` で取得したセッション ID
/// * `command` — 実行するシェルコマンド文字列
///
/// # 戻り値
/// `ExecutionResult { exit_code, stdout, stderr }`
#[tauri::command]
pub async fn pty_execute(
    state: State<'_, PtyManager>,
    session_id: String,
    command: String,
) -> Result<ExecutionResult, String> {
    state.execute_command(&session_id, &command).await
}

/// 指定セッションを終了し、関連プロセスとリソースを解放する。
///
/// # 引数
/// * `session_id` — `pty_spawn` で取得したセッション ID
#[tauri::command]
pub async fn pty_kill(
    state: State<'_, PtyManager>,
    session_id: String,
) -> Result<(), String> {
    state.kill_session(&session_id).await
}
