use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitInstallationStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeDiff {
    pub summary: String,
    pub files_changed: Vec<String>,
    pub diff_text: String,
}

pub fn check_git_available() -> Result<(), String> {
    Command::new("git")
        .arg("--version")
        .output()
        .map_err(|_| "Gitがインストールされていないか、PATHに見つかりません。Git Worktree機能を使用するにはGitが必要です。".to_string())?;
    Ok(())
}

pub fn run_git_output(cwd: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "commit.gpgsign")
        .env("GIT_CONFIG_VALUE_0", "false")
        .output()
        .map_err(|e| format!("Gitコマンドの実行に失敗しました: {}", e))
}

pub fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = run_git_output(cwd, args)?;

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

pub fn run_git_raw(cwd: &Path, args: &[&str]) -> Result<(bool, String, String), String> {
    let output = run_git_output(cwd, args)?;

    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
    ))
}

pub fn resolve_git_internal_path(
    project_path: &Path,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let git_path = run_git(project_path, &["rev-parse", "--git-path", relative_path])?;
    let resolved = PathBuf::from(&git_path);
    if resolved.is_absolute() {
        Ok(resolved)
    } else {
        Ok(project_path.join(resolved))
    }
}

pub fn read_head_file(project_path: &Path, path: &str) -> Result<Option<String>, String> {
    let object = format!("HEAD:{}", path);
    let (exists, _, _) = run_git_raw(project_path, &["cat-file", "-e", &object])?;
    if !exists {
        return Ok(None);
    }

    let output = run_git_output(project_path, &["show", &object])?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Git show が失敗しました: {}", stderr));
    }

    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

fn has_git_dir(project_path: &Path) -> bool {
    project_path.join(".git").exists()
}

fn has_commits(project_path: &Path) -> Result<bool, String> {
    let (success, _, _) = run_git_raw(project_path, &["rev-parse", "--verify", "HEAD"])?;
    Ok(success)
}

fn main_branch_exists(project_path: &Path) -> Result<bool, String> {
    let (success, _, _) = run_git_raw(
        project_path,
        &["show-ref", "--verify", "--quiet", "refs/heads/main"],
    )?;
    Ok(success)
}

fn ensure_local_git_identity(project_path: &Path) -> Result<(), String> {
    let defaults = [
        ("user.name", "vicara"),
        ("user.email", "vicara@example.local"),
    ];

    for (key, value) in defaults {
        let (success, stdout, _) = run_git_raw(project_path, &["config", "--get", key])?;
        if !success || stdout.trim().is_empty() {
            run_git(project_path, &["config", key, value])?;
        }
    }

    Ok(())
}

fn init_repository_on_main(project_path: &Path) -> Result<(), String> {
    let (success, _, _) = run_git_raw(project_path, &["init", "-b", "main"])?;
    if success {
        return Ok(());
    }

    run_git(project_path, &["init"])?;
    run_git(project_path, &["symbolic-ref", "HEAD", "refs/heads/main"])?;
    Ok(())
}

fn ensure_main_branch(project_path: &Path) -> Result<(), String> {
    if main_branch_exists(project_path)? {
        return Ok(());
    }

    if has_commits(project_path)? {
        run_git(project_path, &["branch", "main", "HEAD"])?;
    } else {
        run_git(project_path, &["symbolic-ref", "HEAD", "refs/heads/main"])?;
    }

    Ok(())
}

fn create_initial_commit(project_path: &Path) -> Result<(), String> {
    ensure_local_git_identity(project_path)?;
    run_git(project_path, &["add", "-A"])?;

    let (has_changes, stdout, _) = run_git_raw(project_path, &["status", "--porcelain"])?;
    if !has_changes {
        return Err("Gitステータスの取得に失敗しました。".to_string());
    }

    if stdout.trim().is_empty() {
        run_git(
            project_path,
            &["commit", "--allow-empty", "-m", "Initial commit"],
        )?;
    } else {
        run_git(project_path, &["commit", "-m", "Initial commit"])?;
    }

    Ok(())
}

pub fn ensure_git_repo(project_path: &Path) -> Result<(), String> {
    check_git_available()?;

    if !project_path.exists() || !project_path.is_dir() {
        return Err(
            "指定されたプロジェクトパスが存在しないか、ディレクトリではありません。".to_string(),
        );
    }

    if !has_git_dir(project_path) {
        init_repository_on_main(project_path)?;
        create_initial_commit(project_path)?;
        return Ok(());
    }

    if !has_commits(project_path)? {
        ensure_main_branch(project_path)?;
        ensure_local_git_identity(project_path)?;
        run_git(
            project_path,
            &["commit", "--allow-empty", "-m", "Initial commit"],
        )?;
        return Ok(());
    }

    ensure_main_branch(project_path)?;
    Ok(())
}

#[tauri::command]
pub async fn check_git_installed() -> Result<GitInstallationStatus, String> {
    match Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => Ok(GitInstallationStatus {
            installed: true,
            version: Some(String::from_utf8_lossy(&output.stdout).trim().to_string()),
            message: None,
        }),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Ok(GitInstallationStatus {
                installed: false,
                version: None,
                message: Some(if stderr.is_empty() {
                    "Git コマンドの実行に失敗しました。".to_string()
                } else {
                    format!("Git コマンドの実行に失敗しました: {}", stderr)
                }),
            })
        }
        Err(error) => Ok(GitInstallationStatus {
            installed: false,
            version: None,
            message: Some(format!(
                "vicara の利用には Git のインストールが必要です。詳細: {}",
                error
            )),
        }),
    }
}

pub fn auto_commit_if_needed(wt_path: &Path) -> Result<bool, String> {
    let (success, stdout, _) = run_git_raw(wt_path, &["status", "--porcelain"])?;
    if !success || stdout.is_empty() {
        return Ok(false);
    }

    run_git(wt_path, &["add", "-A"])?;
    run_git(
        wt_path,
        &[
            "commit",
            "-m",
            "[vicara] 自動コミット: エージェント作業完了",
        ],
    )?;

    Ok(true)
}

pub fn parse_conflict_files(merge_output: &str) -> Vec<String> {
    let mut files = Vec::new();
    for line in merge_output.lines() {
        if let Some(pos) = line.find("Merge conflict in ") {
            let file = line[pos + "Merge conflict in ".len()..].trim();
            files.push(file.to_string());
        }
    }
    files
}

pub fn get_worktree_diff(project: &Path, branch_name: &str) -> WorktreeDiff {
    let summary = run_git(
        project,
        &["diff", "--stat", &format!("main...{}", branch_name)],
    )
    .unwrap_or_default();

    let files_output = run_git(
        project,
        &["diff", "--name-only", &format!("main...{}", branch_name)],
    )
    .unwrap_or_default();
    let files_changed = files_output
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect();

    let diff_text =
        run_git(project, &["diff", &format!("main...{}", branch_name)]).unwrap_or_default();

    WorktreeDiff {
        summary,
        files_changed,
        diff_text,
    }
}

pub fn list_changed_files_in_worktree(wt_path: &Path) -> Result<Vec<String>, String> {
    let mut files = BTreeSet::new();

    let mut extend_files = |output: String| {
        for file in output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            files.insert(file.to_string());
        }
    };

    extend_files(run_git(wt_path, &["diff", "--name-only", "main...HEAD"])?);
    extend_files(run_git(wt_path, &["diff", "--name-only"])?);
    extend_files(run_git(wt_path, &["diff", "--name-only", "--cached"])?);
    extend_files(run_git(
        wt_path,
        &["ls-files", "--others", "--exclude-standard"],
    )?);

    Ok(files.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path();

        run_git(path, &["init", "-b", "main"]).expect("git init failed");
        run_git(path, &["config", "user.email", "test@test.com"]).expect("git config failed");
        run_git(path, &["config", "user.name", "Test"]).expect("git config failed");
        run_git(path, &["config", "commit.gpgsign", "false"]).expect("git config failed");

        fs::write(path.join("README.md"), "# Test Project\n").expect("write failed");
        run_git(path, &["add", "."]).expect("git add failed");
        run_git(path, &["commit", "-m", "Initial commit"]).expect("git commit failed");

        dir
    }

    #[test]
    fn list_changed_files_in_worktree_collects_committed_and_untracked_files() {
        let repo = setup_test_repo();
        let wt_path = repo.path().join(".vicara-worktrees").join("task-test");
        fs::create_dir_all(wt_path.parent().expect("parent should exist")).expect("mkdir failed");

        run_git(
            repo.path(),
            &[
                "worktree",
                "add",
                &wt_path.to_string_lossy(),
                "-b",
                "feature/task-test",
                "main",
            ],
        )
        .expect("worktree add failed");

        fs::write(wt_path.join("committed.txt"), "committed\n").expect("write committed failed");
        run_git(&wt_path, &["add", "committed.txt"]).expect("git add failed");
        run_git(&wt_path, &["commit", "-m", "Add committed file"]).expect("git commit failed");

        fs::write(wt_path.join("untracked.txt"), "untracked\n").expect("write untracked failed");

        let changed_files =
            list_changed_files_in_worktree(&wt_path).expect("changed files should be listed");

        assert!(changed_files.contains(&"committed.txt".to_string()));
        assert!(changed_files.contains(&"untracked.txt".to_string()));

        let _ = run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = run_git(repo.path(), &["worktree", "prune"]);
        let _ = run_git(repo.path(), &["branch", "-D", "feature/task-test"]);
    }
}
