use crate::{db, git, preview};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, State};

pub use crate::git::WorktreeDiff;
pub use crate::preview::{PreviewServerInfo, PreviewState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub task_id: String,
    pub worktree_path: String,
    pub branch_name: String,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    Active,
    Merging,
    Merged,
    Conflict,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MergeResult {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "conflict")]
    Conflict { conflicting_files: Vec<String> },
    #[serde(rename = "error")]
    Error { message: String },
}

pub struct WorktreeState {
    worktrees: Mutex<HashMap<String, WorktreeInfo>>,
    max_worktrees: usize,
}

impl WorktreeState {
    pub fn new() -> Self {
        Self {
            worktrees: Mutex::new(HashMap::new()),
            max_worktrees: 5,
        }
    }
}

const WORKTREE_DIR: &str = ".scrum-ai-worktrees";

fn worktree_path(project_path: &str, task_id: &str) -> PathBuf {
    Path::new(project_path)
        .join(WORKTREE_DIR)
        .join(format!("task-{}", task_id))
}

fn branch_name(task_id: &str) -> String {
    format!("feature/task-{}", task_id)
}

async fn resolve_worktree_path_for_task(
    app_handle: &AppHandle,
    project_path: &str,
    task_id: &str,
) -> Result<PathBuf, String> {
    Ok(db::get_worktree_by_task_id(app_handle, task_id)
        .await?
        .map(|record| PathBuf::from(record.worktree_path))
        .unwrap_or_else(|| worktree_path(project_path, task_id)))
}

fn ensure_gitignore_entry(project_path: &Path) -> Result<(), String> {
    let gitignore = project_path.join(".gitignore");
    let entry = format!("{}/", WORKTREE_DIR);

    if gitignore.exists() {
        let content = std::fs::read_to_string(&gitignore)
            .map_err(|e| format!(".gitignore読み込みエラー: {}", e))?;
        if content.lines().any(|line| line.trim() == entry.trim()) {
            return Ok(());
        }
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore)
        .map_err(|e| format!(".gitignore書き込みエラー: {}", e))?;

    if gitignore.exists() {
        let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
        if !content.is_empty() && !content.ends_with('\n') {
            writeln!(file).map_err(|e| format!(".gitignore書き込みエラー: {}", e))?;
        }
    }
    writeln!(file, "{}", entry).map_err(|e| format!(".gitignore書き込みエラー: {}", e))?;

    Ok(())
}

fn link_node_modules(project_path: &Path, wt_path: &Path) -> Result<(), String> {
    let main_nm = project_path.join("node_modules");
    let wt_nm = wt_path.join("node_modules");

    if !main_nm.exists() || wt_nm.exists() {
        return Ok(());
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&main_nm, &wt_nm)
            .map_err(|e| format!("node_modules symlink作成エラー: {}", e))?;
    }

    #[cfg(windows)]
    {
        let symlink_result = std::os::windows::fs::symlink_dir(&main_nm, &wt_nm);
        if symlink_result.is_err() {
            let output = Command::new("cmd")
                .args([
                    "/C",
                    "mklink",
                    "/J",
                    &wt_nm.to_string_lossy(),
                    &main_nm.to_string_lossy(),
                ])
                .output()
                .map_err(|e| format!("junction作成エラー: {}", e))?;

            if !output.status.success() {
                return Err(format!(
                    "node_modules junction作成に失敗しました: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }
    }

    Ok(())
}

fn remove_worktree_node_modules_link(wt_path: &Path) {
    let wt_nm = wt_path.join("node_modules");
    if wt_nm.is_symlink() || (cfg!(windows) && wt_nm.exists()) {
        let _ = if wt_nm.is_symlink() {
            std::fs::remove_file(&wt_nm)
        } else {
            std::fs::remove_dir(&wt_nm)
        };
    }
}

async fn upsert_worktree_record(
    app_handle: &AppHandle,
    task_id: &str,
    worktree_path: &Path,
    branch_name: &str,
    preview_port: Option<i32>,
    preview_pid: Option<i64>,
    status: &str,
) -> Result<(), String> {
    let task = db::get_task_by_id(app_handle, task_id)
        .await?
        .ok_or_else(|| format!("task_id={} のタスクが見つかりません", task_id))?;

    let record_id = db::get_worktree_by_task_id(app_handle, task_id)
        .await?
        .map(|record| record.id)
        .unwrap_or_else(|| format!("worktree-{}", task_id));

    db::upsert_worktree_record(
        app_handle,
        db::WorktreeUpsertInput {
            id: record_id,
            task_id: task_id.to_string(),
            project_id: task.project_id,
            worktree_path: worktree_path.to_string_lossy().to_string(),
            branch_name: branch_name.to_string(),
            preview_port,
            preview_pid,
            status: status.to_string(),
        },
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn create_worktree(
    app_handle: AppHandle,
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<WorktreeInfo, String> {
    let project = Path::new(&project_path);
    git::ensure_git_repo(project)?;

    {
        let worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        let active_count = worktrees
            .values()
            .filter(|worktree| worktree.status == WorktreeStatus::Active)
            .count();
        if active_count >= state.max_worktrees {
            return Err(format!(
                "同時ワークツリーの上限（{}）に達しています。既存のワークツリーをマージまたは削除してください。",
                state.max_worktrees
            ));
        }
        if let Some(existing) = worktrees.get(&task_id) {
            if existing.status == WorktreeStatus::Active {
                return Err(format!(
                    "タスク {} のワークツリーは既に存在します: {}",
                    task_id, existing.worktree_path
                ));
            }
        }
    }

    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);

    let parent = wt_path
        .parent()
        .ok_or("ワークツリーの親ディレクトリが不正です")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("ディレクトリ作成エラー: {}", e))?;

    ensure_gitignore_entry(project)?;
    git::run_git(
        project,
        &[
            "worktree",
            "add",
            &wt_path.to_string_lossy(),
            "-b",
            &br_name,
            "main",
        ],
    )?;
    link_node_modules(project, &wt_path)?;

    let info = WorktreeInfo {
        task_id: task_id.clone(),
        worktree_path: wt_path.to_string_lossy().to_string(),
        branch_name: br_name,
        status: WorktreeStatus::Active,
    };

    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        worktrees.insert(task_id, info.clone());
    }

    let _ = upsert_worktree_record(
        &app_handle,
        &info.task_id,
        Path::new(&info.worktree_path),
        &info.branch_name,
        None,
        None,
        "active",
    )
    .await;

    Ok(info)
}

#[tauri::command]
pub async fn remove_worktree(
    app_handle: AppHandle,
    preview_state: State<'_, PreviewState>,
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<(), String> {
    let project = Path::new(&project_path);
    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);

    let _ = preview_state.stop_server(&task_id);
    remove_worktree_node_modules_link(&wt_path);

    let _ = git::run_git(
        project,
        &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
    );

    if wt_path.exists() {
        let _ = std::fs::remove_dir_all(&wt_path);
    }

    let _ = git::run_git(project, &["worktree", "prune"]);

    if git::run_git(project, &["branch", "-d", &br_name]).is_err() {
        let _ = git::run_git(project, &["branch", "-D", &br_name]);
    }

    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        if let Some(info) = worktrees.get_mut(&task_id) {
            info.status = WorktreeStatus::Removed;
        }
        worktrees.remove(&task_id);
    }

    let _ = db::update_worktree_record_state(&app_handle, &task_id, None, None, "removed").await;

    Ok(())
}

#[tauri::command]
pub async fn merge_worktree(
    app_handle: AppHandle,
    preview_state: State<'_, PreviewState>,
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<MergeResult, String> {
    let project = Path::new(&project_path);
    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);

    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        if let Some(info) = worktrees.get_mut(&task_id) {
            info.status = WorktreeStatus::Merging;
        }
    }

    if wt_path.exists() {
        let _ = git::auto_commit_if_needed(&wt_path);
    }

    let current_branch = git::run_git(project, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if current_branch != "main" {
        return Err(format!(
            "プロジェクトルートが main ブランチ上にありません（現在: {}）。マージ前に main へ切り替えてください。",
            current_branch
        ));
    }

    let (success, stdout, stderr) = git::run_git_raw(
        project,
        &[
            "merge",
            "--no-ff",
            "-m",
            &format!("[MicroScrum AI] Merge task-{}", task_id),
            &br_name,
        ],
    )?;

    if !success {
        let _ = git::run_git(project, &["merge", "--abort"]);
        let conflicting_files = git::parse_conflict_files(&format!("{}\n{}", stdout, stderr));

        {
            let mut worktrees = state
                .worktrees
                .lock()
                .map_err(|e| format!("State lock error: {}", e))?;
            if let Some(info) = worktrees.get_mut(&task_id) {
                info.status = WorktreeStatus::Conflict;
            }
        }

        let _ =
            db::update_worktree_record_state(&app_handle, &task_id, None, None, "conflict").await;

        return Ok(MergeResult::Conflict { conflicting_files });
    }

    let _ = preview_state.stop_server(&task_id);
    remove_worktree_node_modules_link(&wt_path);

    let _ = git::run_git(
        project,
        &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
    );
    if wt_path.exists() {
        let _ = std::fs::remove_dir_all(&wt_path);
    }
    let _ = git::run_git(project, &["worktree", "prune"]);
    let _ = git::run_git(project, &["branch", "-d", &br_name]);

    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        worktrees.remove(&task_id);
    }

    let _ = db::update_worktree_record_state(&app_handle, &task_id, None, None, "merged").await;
    let _ = db::update_task_status(app_handle, task_id, "Done".to_string()).await;

    Ok(MergeResult::Success)
}

#[tauri::command]
pub async fn get_worktree_status(
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<Option<WorktreeInfo>, String> {
    {
        let worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        if let Some(info) = worktrees.get(&task_id) {
            return Ok(Some(info.clone()));
        }
    }

    let wt_path = worktree_path(&project_path, &task_id);
    if wt_path.exists() {
        let br_name = branch_name(&task_id);
        return Ok(Some(WorktreeInfo {
            task_id,
            worktree_path: wt_path.to_string_lossy().to_string(),
            branch_name: br_name,
            status: WorktreeStatus::Active,
        }));
    }

    Ok(None)
}

#[tauri::command]
pub async fn get_worktree_diff(
    project_path: String,
    task_id: String,
) -> Result<WorktreeDiff, String> {
    let project = Path::new(&project_path);
    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);

    if wt_path.exists() {
        let _ = git::auto_commit_if_needed(&wt_path);
    }

    Ok(git::get_worktree_diff(project, &br_name))
}

#[tauri::command]
pub async fn start_preview_server(
    app_handle: AppHandle,
    preview_state: State<'_, PreviewState>,
    project_path: String,
    task_id: String,
    command: Option<String>,
) -> Result<PreviewServerInfo, String> {
    let wt_path = resolve_worktree_path_for_task(&app_handle, &project_path, &task_id).await?;

    if !wt_path.exists() {
        return Err(
            "対象のワークツリーが見つかりません。先に Claude 実行で worktree を生成してください。"
                .to_string(),
        );
    }

    let info = preview::start_preview_for_task(&preview_state, &task_id, &wt_path, command)?;
    upsert_worktree_record(
        &app_handle,
        &task_id,
        &wt_path,
        &branch_name(&task_id),
        Some(info.port as i32),
        Some(info.pid as i64),
        "active",
    )
    .await?;

    Ok(info)
}

#[tauri::command]
pub async fn stop_preview_server(
    app_handle: AppHandle,
    preview_state: State<'_, PreviewState>,
    task_id: String,
) -> Result<bool, String> {
    let stopped = preview_state.stop_server(&task_id)?.is_some();
    let _ = db::update_worktree_record_state(&app_handle, &task_id, None, None, "active").await;
    Ok(stopped)
}

#[tauri::command]
pub async fn open_preview_in_browser(app_handle: AppHandle, port: u16) -> Result<(), String> {
    preview::open_preview_in_browser(&app_handle, port)
}

#[tauri::command]
pub async fn open_static_preview(
    app_handle: AppHandle,
    project_path: String,
    task_id: String,
) -> Result<String, String> {
    let wt_path = resolve_worktree_path_for_task(&app_handle, &project_path, &task_id).await?;

    if !wt_path.exists() {
        return Err(
            "対象のワークツリーが見つかりません。先に Claude 実行で worktree を生成してください。"
                .to_string(),
        );
    }

    let index_path = wt_path.join("index.html");
    if !index_path.exists() {
        return Err(
            "ワークツリー内に index.html が見つかりません。静的サイト構成か確認してください。"
                .to_string(),
        );
    }

    preview::open_local_path(&app_handle, &index_path)
        .map_err(|e| format!("index.html を開けませんでした: {}", e))
}

pub fn cleanup_orphaned_worktrees(project_path: &str) -> Vec<String> {
    let project = Path::new(project_path);
    let worktree_base = project.join(WORKTREE_DIR);
    let mut cleaned = Vec::new();

    if !worktree_base.exists() {
        return cleaned;
    }

    let _ = git::run_git(project, &["worktree", "prune"]);

    let entries = match std::fs::read_dir(&worktree_base) {
        Ok(entries) => entries,
        Err(_) => return cleaned,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue,
        };

        let task_id = if let Some(id) = dir_name.strip_prefix("task-") {
            id.to_string()
        } else {
            continue;
        };

        remove_worktree_node_modules_link(&path);
        let _ = git::run_git(
            project,
            &["worktree", "remove", &path.to_string_lossy(), "--force"],
        );

        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }

        let _ = git::run_git(project, &["branch", "-D", &branch_name(&task_id)]);
        cleaned.push(format!("task-{}", task_id));
        log::info!("Orphaned worktree cleaned: task-{}", task_id);
    }

    let _ = git::run_git(project, &["worktree", "prune"]);

    if let Ok(mut entries) = std::fs::read_dir(&worktree_base) {
        if entries.next().is_none() {
            let _ = std::fs::remove_dir(&worktree_base);
        }
    }

    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path();

        git::run_git(path, &["init", "-b", "main"]).expect("git init failed");
        git::run_git(path, &["config", "user.email", "test@test.com"]).expect("git config failed");
        git::run_git(path, &["config", "user.name", "Test"]).expect("git config failed");
        git::run_git(path, &["config", "commit.gpgsign", "false"])
            .expect("git config gpgsign failed");

        fs::write(path.join("README.md"), "# Test Project\n").expect("write failed");
        git::run_git(path, &["add", "."]).expect("git add failed");
        git::run_git(path, &["commit", "-m", "Initial commit"]).expect("git commit failed");

        dir
    }

    #[test]
    fn test_check_git_available() {
        assert!(git::check_git_available().is_ok());
    }

    #[test]
    fn test_worktree_path_construction() {
        let path = worktree_path("/project", "abc123");
        assert_eq!(
            path,
            PathBuf::from("/project/.scrum-ai-worktrees/task-abc123")
        );
    }

    #[test]
    fn test_branch_name_construction() {
        assert_eq!(branch_name("abc123"), "feature/task-abc123");
    }

    #[test]
    fn test_create_and_remove_worktree() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "test-001";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        ensure_gitignore_entry(repo.path()).expect("gitignore failed");

        let gitignore = fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains(".scrum-ai-worktrees/"));

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        git::run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                &br_name,
                "main",
            ],
        )
        .expect("worktree add failed");

        assert!(wt_path.exists());
        assert!(wt_path.join("README.md").exists());

        let branches = git::run_git(repo.path(), &["branch"]).unwrap();
        assert!(branches.contains("feature/task-test-001"));

        let _ = git::run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = git::run_git(repo.path(), &["worktree", "prune"]);
        let _ = git::run_git(repo.path(), &["branch", "-D", &br_name]);

        assert!(!wt_path.exists());
        let branches = git::run_git(repo.path(), &["branch"]).unwrap();
        assert!(!branches.contains("feature/task-test-001"));
    }

    #[test]
    fn test_merge_worktree_success() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "merge-ok";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        git::run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                &br_name,
                "main",
            ],
        )
        .expect("worktree add failed");

        fs::write(wt_path.join("new_file.txt"), "Hello from worktree\n").unwrap();
        git::run_git(&wt_path, &["add", "."]).unwrap();
        git::run_git(&wt_path, &["commit", "-m", "Add new file"]).unwrap();

        let (success, _, stderr) = git::run_git_raw(
            repo.path(),
            &[
                "merge",
                "--no-ff",
                "-m",
                &format!("[MicroScrum AI] Merge task-{}", task_id),
                &br_name,
            ],
        )
        .unwrap();

        assert!(success, "Merge failed: {}", stderr);
        assert!(repo.path().join("new_file.txt").exists());

        let _ = git::run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = git::run_git(repo.path(), &["worktree", "prune"]);
        let _ = git::run_git(repo.path(), &["branch", "-d", &br_name]);
    }

    #[test]
    fn test_merge_worktree_conflict() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "merge-conflict";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        git::run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                &br_name,
                "main",
            ],
        )
        .expect("worktree add failed");

        fs::write(wt_path.join("README.md"), "Changed by worktree\n").unwrap();
        git::run_git(&wt_path, &["add", "."]).unwrap();
        git::run_git(&wt_path, &["commit", "-m", "Worktree change"]).unwrap();

        fs::write(repo.path().join("README.md"), "Changed on main\n").unwrap();
        git::run_git(repo.path(), &["add", "."]).unwrap();
        git::run_git(repo.path(), &["commit", "-m", "Main change"]).unwrap();

        let (success, stdout, stderr) = git::run_git_raw(
            repo.path(),
            &["merge", "--no-ff", "-m", "Merge test", &br_name],
        )
        .unwrap();

        assert!(!success, "Expected merge to fail with conflict");

        let conflict_files = git::parse_conflict_files(&format!("{}\n{}", stdout, stderr));
        assert!(
            conflict_files.contains(&"README.md".to_string()),
            "Expected README.md in conflict files, got: {:?}",
            conflict_files
        );

        let _ = git::run_git(repo.path(), &["merge", "--abort"]);
        let _ = git::run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = git::run_git(repo.path(), &["worktree", "prune"]);
        let _ = git::run_git(repo.path(), &["branch", "-D", &br_name]);
    }

    #[test]
    fn test_auto_commit_if_needed() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "auto-commit";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        git::run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                &br_name,
                "main",
            ],
        )
        .unwrap();

        let committed = git::auto_commit_if_needed(&wt_path).unwrap();
        assert!(!committed);

        fs::write(wt_path.join("auto.txt"), "auto\n").unwrap();
        let committed = git::auto_commit_if_needed(&wt_path).unwrap();
        assert!(committed);

        let log = git::run_git(&wt_path, &["log", "--oneline", "-1"]).unwrap();
        assert!(log.contains("自動コミット"));

        let _ = git::run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = git::run_git(repo.path(), &["worktree", "prune"]);
        let _ = git::run_git(repo.path(), &["branch", "-D", &br_name]);
    }

    #[test]
    fn test_link_node_modules() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "nm-link";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        let nm_dir = repo.path().join("node_modules");
        fs::create_dir_all(nm_dir.join("some-package")).unwrap();
        fs::write(nm_dir.join("some-package/index.js"), "module.exports = {};").unwrap();

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        git::run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                &br_name,
                "main",
            ],
        )
        .unwrap();

        link_node_modules(repo.path(), &wt_path).expect("link_node_modules failed");

        let wt_nm = wt_path.join("node_modules");
        assert!(wt_nm.exists(), "node_modules symlink should exist");
        assert!(
            wt_nm.join("some-package/index.js").exists(),
            "Should be able to access files through symlink"
        );

        let _ = std::fs::remove_file(&wt_nm);
        let _ = git::run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = git::run_git(repo.path(), &["worktree", "prune"]);
        let _ = git::run_git(repo.path(), &["branch", "-D", &br_name]);
    }

    #[test]
    fn test_cleanup_orphaned_worktrees() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "orphan-001";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        git::run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                &br_name,
                "main",
            ],
        )
        .unwrap();

        assert!(wt_path.exists());

        let cleaned = cleanup_orphaned_worktrees(&project_path);

        assert!(
            cleaned.contains(&"task-orphan-001".to_string()),
            "Should have cleaned orphan. Got: {:?}",
            cleaned
        );
        assert!(!wt_path.exists(), "Worktree directory should be removed");
    }

    #[test]
    fn test_ensure_gitignore_idempotent() {
        let repo = setup_test_repo();

        ensure_gitignore_entry(repo.path()).unwrap();
        ensure_gitignore_entry(repo.path()).unwrap();

        let content = fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        let count = content
            .lines()
            .filter(|line| line.trim() == ".scrum-ai-worktrees/")
            .count();
        assert_eq!(count, 1, "Should appear exactly once, got: {}", count);
    }

    #[test]
    fn test_get_diff() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "diff-test";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        git::run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                &br_name,
                "main",
            ],
        )
        .unwrap();

        fs::write(wt_path.join("diff_file.txt"), "diff content\n").unwrap();
        git::run_git(&wt_path, &["add", "."]).unwrap();
        git::run_git(&wt_path, &["commit", "-m", "Add diff file"]).unwrap();

        let diff = git::get_worktree_diff(repo.path(), &br_name);
        assert!(diff.summary.contains("diff_file.txt"));
        assert!(diff.files_changed.contains(&"diff_file.txt".to_string()));

        let _ = git::run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = git::run_git(repo.path(), &["worktree", "prune"]);
        let _ = git::run_git(repo.path(), &["branch", "-D", &br_name]);
    }

    #[test]
    fn test_ensure_git_repo_initializes_repository_and_tracks_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.html"), "<h1>Hello</h1>").unwrap();

        git::ensure_git_repo(dir.path()).unwrap();

        assert!(dir.path().join(".git").exists());
        assert_eq!(
            git::run_git(dir.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap(),
            "main"
        );
        assert!(
            git::run_git(dir.path(), &["ls-files"])
                .unwrap()
                .contains("index.html"),
            "index.html should be included in the initial commit"
        );
    }

    #[test]
    fn test_ensure_git_repo_creates_empty_commit_for_existing_repo_without_commits() {
        let dir = tempfile::tempdir().unwrap();

        git::run_git(dir.path(), &["init"]).unwrap();
        git::ensure_git_repo(dir.path()).unwrap();

        assert_eq!(
            git::run_git(dir.path(), &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap(),
            "main"
        );
        assert!(git::run_git(dir.path(), &["log", "--oneline"])
            .unwrap()
            .contains("Initial commit"));
    }
}
