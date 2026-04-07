use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::State;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeDiff {
    pub summary: String,
    pub files_changed: Vec<String>,
    pub diff_text: String,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct WorktreeState {
    /// task_id -> WorktreeInfo
    worktrees: Mutex<HashMap<String, WorktreeInfo>>,
    /// Maximum number of concurrent worktrees allowed
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

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const WORKTREE_DIR: &str = ".scrum-ai-worktrees";

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check if git is available on the system
fn check_git_available() -> Result<(), String> {
    Command::new("git")
        .arg("--version")
        .output()
        .map_err(|_| "Gitがインストールされていないか、PATHに見つかりません。Git Worktree機能を使用するにはGitが必要です。".to_string())?;
    Ok(())
}

/// Run a git command in the specified directory and return stdout.
/// Automatically sets config to avoid GPG signing issues in automated contexts.
fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "commit.gpgsign")
        .env("GIT_CONFIG_VALUE_0", "false")
        .output()
        .map_err(|e| format!("Gitコマンドの実行に失敗しました: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!(
            "Git {} が失敗しました: {}",
            args.first().unwrap_or(&""),
            stderr
        ))
    }
}

/// Run a git command allowing failure, returning (success, stdout, stderr).
/// Automatically sets config to avoid GPG signing issues in automated contexts.
fn run_git_raw(cwd: &Path, args: &[&str]) -> Result<(bool, String, String), String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "commit.gpgsign")
        .env("GIT_CONFIG_VALUE_0", "false")
        .output()
        .map_err(|e| format!("Gitコマンドの実行に失敗しました: {}", e))?;

    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

/// Build the worktree path for a given task
fn worktree_path(project_path: &str, task_id: &str) -> PathBuf {
    Path::new(project_path)
        .join(WORKTREE_DIR)
        .join(format!("task-{}", task_id))
}

/// Build the branch name for a given task
fn branch_name(task_id: &str) -> String {
    format!("feature/task-{}", task_id)
}

/// Ensure `.scrum-ai-worktrees/` is listed in .gitignore
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

    // Append the entry
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore)
        .map_err(|e| format!(".gitignore書き込みエラー: {}", e))?;

    // Ensure newline before our entry
    if gitignore.exists() {
        let content = std::fs::read_to_string(&gitignore).unwrap_or_default();
        if !content.is_empty() && !content.ends_with('\n') {
            writeln!(file).map_err(|e| format!(".gitignore書き込みエラー: {}", e))?;
        }
    }
    writeln!(file, "{}", entry).map_err(|e| format!(".gitignore書き込みエラー: {}", e))?;

    Ok(())
}

/// Create a symlink for node_modules from the main project to the worktree.
/// On Windows, falls back to junction if symlink creation fails.
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
        // Try symlink first, fall back to junction
        let symlink_result = std::os::windows::fs::symlink_dir(&main_nm, &wt_nm);
        if symlink_result.is_err() {
            // Fall back to junction (does not require admin privileges)
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

/// Check if the worktree has uncommitted changes and auto-commit them
fn auto_commit_if_needed(wt_path: &Path) -> Result<bool, String> {
    // Check for uncommitted changes
    let (success, stdout, _) = run_git_raw(wt_path, &["status", "--porcelain"])?;
    if !success || stdout.is_empty() {
        return Ok(false);
    }

    // Stage all changes
    run_git(wt_path, &["add", "-A"])?;

    // Commit
    run_git(
        wt_path,
        &["commit", "-m", "[MicroScrum AI] 自動コミット: エージェント作業完了"],
    )?;

    Ok(true)
}

/// Parse conflicting file paths from git merge stderr/stdout
fn parse_conflict_files(merge_output: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in merge_output.lines() {
        // Pattern: "CONFLICT (content): Merge conflict in <file>"
        if let Some(pos) = line.find("Merge conflict in ") {
            let file = line[pos + "Merge conflict in ".len()..].trim();
            files.push(file.to_string());
        }
        // Pattern: "CONFLICT (add/add): Merge conflict in <file>"
        // Already covered above
    }
    files
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Create a new git worktree for a task
#[tauri::command]
pub async fn create_worktree(
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<WorktreeInfo, String> {
    check_git_available()?;

    let project = Path::new(&project_path);
    if !project.join(".git").exists() && !project.join(".git").is_file() {
        // Also check if it's a file (submodule/worktree case)
        return Err("指定されたプロジェクトパスはGitリポジトリではありません。".to_string());
    }

    // Check concurrency limit
    {
        let worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        let active_count = worktrees
            .values()
            .filter(|w| w.status == WorktreeStatus::Active)
            .count();
        if active_count >= state.max_worktrees {
            return Err(format!(
                "同時ワークツリーの上限（{}）に達しています。既存のワークツリーをマージまたは削除してください。",
                state.max_worktrees
            ));
        }
        // Check for duplicate
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

    // Ensure parent directory exists
    let parent = wt_path
        .parent()
        .ok_or("ワークツリーの親ディレクトリが不正です")?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("ディレクトリ作成エラー: {}", e))?;

    // Ensure .gitignore entry
    ensure_gitignore_entry(project)?;

    // Create worktree: git worktree add <path> -b <branch> main
    run_git(
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

    // Link node_modules from main project
    link_node_modules(project, &wt_path)?;

    let info = WorktreeInfo {
        task_id: task_id.clone(),
        worktree_path: wt_path.to_string_lossy().to_string(),
        branch_name: br_name,
        status: WorktreeStatus::Active,
    };

    // Register in state
    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        worktrees.insert(task_id, info.clone());
    }

    Ok(info)
}

/// Remove a worktree and its associated branch
#[tauri::command]
pub async fn remove_worktree(
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<(), String> {
    let project = Path::new(&project_path);
    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);

    // Remove node_modules symlink first to avoid git worktree remove issues
    let wt_nm = wt_path.join("node_modules");
    if wt_nm.is_symlink() || (cfg!(windows) && wt_nm.exists()) {
        // On Windows, junction is detected as dir, remove with remove_dir
        let _ = if wt_nm.is_symlink() {
            std::fs::remove_file(&wt_nm)
        } else {
            std::fs::remove_dir(&wt_nm)
        };
    }

    // Remove worktree (force to handle unclean state)
    let _ = run_git(project, &["worktree", "remove", &wt_path.to_string_lossy(), "--force"]);

    // If directory still exists, clean up manually
    if wt_path.exists() {
        let _ = std::fs::remove_dir_all(&wt_path);
    }

    // Prune worktree references
    let _ = run_git(project, &["worktree", "prune"]);

    // Delete the branch (safe delete first, force if needed)
    let delete_result = run_git(project, &["branch", "-d", &br_name]);
    if delete_result.is_err() {
        let _ = run_git(project, &["branch", "-D", &br_name]);
    }

    // Update state
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

    Ok(())
}

/// Merge a task's worktree branch into main and clean up
#[tauri::command]
pub async fn merge_worktree(
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<MergeResult, String> {
    let project = Path::new(&project_path);
    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);

    // Update state to merging
    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        if let Some(info) = worktrees.get_mut(&task_id) {
            info.status = WorktreeStatus::Merging;
        }
    }

    // Auto-commit any uncommitted changes in the worktree
    if wt_path.exists() {
        let _ = auto_commit_if_needed(&wt_path);
    }

    // Attempt merge on main
    // First, ensure we are on main in the project root
    let current_branch = run_git(project, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if current_branch != "main" {
        return Err(format!(
            "プロジェクトルートが main ブランチ上にありません（現在: {}）。マージ前に main へ切り替えてください。",
            current_branch
        ));
    }

    let (success, stdout, stderr) = run_git_raw(
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
        // Merge conflict: abort and report
        let _ = run_git(project, &["merge", "--abort"]);

        let conflicting_files = parse_conflict_files(&format!("{}\n{}", stdout, stderr));

        // Update state to conflict
        {
            let mut worktrees = state
                .worktrees
                .lock()
                .map_err(|e| format!("State lock error: {}", e))?;
            if let Some(info) = worktrees.get_mut(&task_id) {
                info.status = WorktreeStatus::Conflict;
            }
        }

        return Ok(MergeResult::Conflict { conflicting_files });
    }

    // Merge succeeded — clean up worktree and branch
    // Remove node_modules symlink first
    let wt_nm = wt_path.join("node_modules");
    if wt_nm.is_symlink() || (cfg!(windows) && wt_nm.exists()) {
        let _ = if wt_nm.is_symlink() {
            std::fs::remove_file(&wt_nm)
        } else {
            std::fs::remove_dir(&wt_nm)
        };
    }

    let _ = run_git(project, &["worktree", "remove", &wt_path.to_string_lossy(), "--force"]);
    if wt_path.exists() {
        let _ = std::fs::remove_dir_all(&wt_path);
    }
    let _ = run_git(project, &["worktree", "prune"]);
    let _ = run_git(project, &["branch", "-d", &br_name]);

    // Update state
    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        worktrees.remove(&task_id);
    }

    Ok(MergeResult::Success)
}

/// Get the status of a worktree for a task
#[tauri::command]
pub async fn get_worktree_status(
    state: State<'_, WorktreeState>,
    project_path: String,
    task_id: String,
) -> Result<Option<WorktreeInfo>, String> {
    // Check in-memory state first
    {
        let worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        if let Some(info) = worktrees.get(&task_id) {
            return Ok(Some(info.clone()));
        }
    }

    // Check filesystem as fallback
    let wt_path = worktree_path(&project_path, &task_id);
    if wt_path.exists() {
        let br_name = branch_name(&task_id);
        let info = WorktreeInfo {
            task_id,
            worktree_path: wt_path.to_string_lossy().to_string(),
            branch_name: br_name,
            status: WorktreeStatus::Active,
        };
        return Ok(Some(info));
    }

    Ok(None)
}

/// Get diff between the worktree's branch and main
#[tauri::command]
pub async fn get_worktree_diff(
    project_path: String,
    task_id: String,
) -> Result<WorktreeDiff, String> {
    let project = Path::new(&project_path);
    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);

    // Auto-commit pending changes so they show in diff
    if wt_path.exists() {
        let _ = auto_commit_if_needed(&wt_path);
    }

    // Get diff stat (summary)
    let summary = run_git(project, &["diff", "--stat", &format!("main...{}", br_name)])
        .unwrap_or_default();

    // Get changed file names
    let files_output = run_git(
        project,
        &["diff", "--name-only", &format!("main...{}", br_name)],
    )
    .unwrap_or_default();
    let files_changed: Vec<String> = files_output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect();

    // Get full diff
    let diff_text = run_git(project, &["diff", &format!("main...{}", br_name)])
        .unwrap_or_default();

    Ok(WorktreeDiff {
        summary,
        files_changed,
        diff_text,
    })
}

/// Detect and clean up orphaned worktrees at startup.
/// Call this during app initialization.
pub fn cleanup_orphaned_worktrees(project_path: &str) -> Vec<String> {
    let project = Path::new(project_path);
    let worktree_base = project.join(WORKTREE_DIR);
    let mut cleaned = Vec::new();

    if !worktree_base.exists() {
        return cleaned;
    }

    // List actual worktrees via git
    let _ = run_git(project, &["worktree", "prune"]);

    // Scan the directory for any leftover task folders
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

        // Extract task_id from dir name (task-<id>)
        let task_id = if let Some(id) = dir_name.strip_prefix("task-") {
            id.to_string()
        } else {
            continue;
        };

        // Remove node_modules link first
        let nm = path.join("node_modules");
        if nm.is_symlink() || (cfg!(windows) && nm.exists()) {
            let _ = if nm.is_symlink() {
                std::fs::remove_file(&nm)
            } else {
                std::fs::remove_dir(&nm)
            };
        }

        // Try to remove via git worktree remove
        let _ = run_git(
            project,
            &["worktree", "remove", &path.to_string_lossy(), "--force"],
        );

        // If still there, force remove
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }

        // Remove branch if it exists
        let br = branch_name(&task_id);
        let _ = run_git(project, &["branch", "-D", &br]);

        cleaned.push(format!("task-{}", task_id));
        log::info!("Orphaned worktree cleaned: task-{}", task_id);
    }

    // Final prune
    let _ = run_git(project, &["worktree", "prune"]);

    // Remove the base dir if empty
    if let Ok(mut entries) = std::fs::read_dir(&worktree_base) {
        if entries.next().is_none() {
            let _ = std::fs::remove_dir(&worktree_base);
        }
    }

    cleaned
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// Create a temporary git repo for testing
    fn setup_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path();

        // git init with main branch
        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(path)
            .output()
            .expect("git init failed");

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .expect("git config failed");

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .expect("git config failed");

        Command::new("git")
            .args(["config", "commit.gpgsign", "false"])
            .current_dir(path)
            .output()
            .expect("git config gpgsign failed");

        // Create initial commit
        fs::write(path.join("README.md"), "# Test Project\n").expect("write failed");
        Command::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .expect("git add failed");
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(path)
            .output()
            .expect("git commit failed");

        dir
    }

    #[test]
    fn test_check_git_available() {
        assert!(check_git_available().is_ok());
    }

    #[test]
    fn test_worktree_path_construction() {
        let p = worktree_path("/project", "abc123");
        assert_eq!(
            p,
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

        // Create worktree
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        ensure_gitignore_entry(repo.path()).expect("gitignore failed");

        // Verify .gitignore contains entry
        let gitignore = fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        assert!(gitignore.contains(".scrum-ai-worktrees/"));

        // Create worktree via git directly (testing the helper functions)
        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        run_git(
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

        // Verify branch was created
        let branches = run_git(repo.path(), &["branch"]).unwrap();
        assert!(branches.contains("feature/task-test-001"));

        // Remove worktree
        let _ = run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = run_git(repo.path(), &["worktree", "prune"]);
        let _ = run_git(repo.path(), &["branch", "-D", &br_name]);

        assert!(!wt_path.exists());
        let branches = run_git(repo.path(), &["branch"]).unwrap();
        assert!(!branches.contains("feature/task-test-001"));
    }

    #[test]
    fn test_merge_worktree_success() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "merge-ok";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        // Create worktree
        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        run_git(
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

        // Make a change in the worktree
        fs::write(wt_path.join("new_file.txt"), "Hello from worktree\n").unwrap();
        run_git(&wt_path, &["add", "."]).unwrap();
        run_git(&wt_path, &["commit", "-m", "Add new file"]).unwrap();

        // Merge into main
        let (success, _, stderr) = run_git_raw(
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

        // Verify file exists on main
        assert!(repo.path().join("new_file.txt").exists());

        // Clean up
        let _ = run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = run_git(repo.path(), &["worktree", "prune"]);
        let _ = run_git(repo.path(), &["branch", "-d", &br_name]);
    }

    #[test]
    fn test_merge_worktree_conflict() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "merge-conflict";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        // Create worktree
        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        run_git(
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

        // Make a change in the worktree
        fs::write(wt_path.join("README.md"), "Changed by worktree\n").unwrap();
        run_git(&wt_path, &["add", "."]).unwrap();
        run_git(&wt_path, &["commit", "-m", "Worktree change"]).unwrap();

        // Make a conflicting change on main
        fs::write(repo.path().join("README.md"), "Changed on main\n").unwrap();
        run_git(repo.path(), &["add", "."]).unwrap();
        run_git(repo.path(), &["commit", "-m", "Main change"]).unwrap();

        // Attempt merge — should conflict
        let (success, stdout, stderr) = run_git_raw(
            repo.path(),
            &["merge", "--no-ff", "-m", "Merge test", &br_name],
        )
        .unwrap();

        assert!(!success, "Expected merge to fail with conflict");

        let conflict_files = parse_conflict_files(&format!("{}\n{}", stdout, stderr));
        assert!(
            conflict_files.contains(&"README.md".to_string()),
            "Expected README.md in conflict files, got: {:?}",
            conflict_files
        );

        // Abort merge
        let _ = run_git(repo.path(), &["merge", "--abort"]);

        // Clean up
        let _ = run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = run_git(repo.path(), &["worktree", "prune"]);
        let _ = run_git(repo.path(), &["branch", "-D", &br_name]);
    }

    #[test]
    fn test_auto_commit_if_needed() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "auto-commit";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        run_git(
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

        // No changes — should return false
        let committed = auto_commit_if_needed(&wt_path).unwrap();
        assert!(!committed);

        // Add a file — should auto-commit
        fs::write(wt_path.join("auto.txt"), "auto\n").unwrap();
        let committed = auto_commit_if_needed(&wt_path).unwrap();
        assert!(committed);

        // Verify the commit was made
        let log = run_git(&wt_path, &["log", "--oneline", "-1"]).unwrap();
        assert!(log.contains("自動コミット"));

        // Clean up
        let _ = run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = run_git(repo.path(), &["worktree", "prune"]);
        let _ = run_git(repo.path(), &["branch", "-D", &br_name]);
    }

    #[test]
    fn test_link_node_modules() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "nm-link";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        // Create a fake node_modules in project root
        let nm_dir = repo.path().join("node_modules");
        fs::create_dir_all(nm_dir.join("some-package")).unwrap();
        fs::write(nm_dir.join("some-package/index.js"), "module.exports = {};").unwrap();

        // Create worktree
        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        run_git(
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

        // Link node_modules
        link_node_modules(repo.path(), &wt_path).expect("link_node_modules failed");

        // Verify symlink exists and points to correct location
        let wt_nm = wt_path.join("node_modules");
        assert!(wt_nm.exists(), "node_modules symlink should exist");
        assert!(
            wt_nm.join("some-package/index.js").exists(),
            "Should be able to access files through symlink"
        );

        // Clean up symlink first
        let _ = std::fs::remove_file(&wt_nm);
        let _ = run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = run_git(repo.path(), &["worktree", "prune"]);
        let _ = run_git(repo.path(), &["branch", "-D", &br_name]);
    }

    #[test]
    fn test_cleanup_orphaned_worktrees() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();

        // Create a worktree then simulate orphaning by deleting only the git reference
        let task_id = "orphan-001";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(wt_path.parent().unwrap()).unwrap();
        run_git(
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

        // Run cleanup
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

        // Call twice — should not duplicate entry
        ensure_gitignore_entry(repo.path()).unwrap();
        ensure_gitignore_entry(repo.path()).unwrap();

        let content = fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        let count = content
            .lines()
            .filter(|l| l.trim() == ".scrum-ai-worktrees/")
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
        run_git(
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

        // Make a change
        fs::write(wt_path.join("diff_file.txt"), "diff content\n").unwrap();
        run_git(&wt_path, &["add", "."]).unwrap();
        run_git(&wt_path, &["commit", "-m", "Add diff file"]).unwrap();

        // Get diff summary
        let summary =
            run_git(repo.path(), &["diff", "--stat", &format!("main...{}", br_name)]).unwrap();
        assert!(
            summary.contains("diff_file.txt"),
            "Diff summary should contain diff_file.txt"
        );

        let files =
            run_git(repo.path(), &["diff", "--name-only", &format!("main...{}", br_name)])
                .unwrap();
        assert!(files.contains("diff_file.txt"));

        // Clean up
        let _ = run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = run_git(repo.path(), &["worktree", "prune"]);
        let _ = run_git(repo.path(), &["branch", "-D", &br_name]);
    }
}
