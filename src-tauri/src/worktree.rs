use crate::{db, git, node_dependencies, preview};
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

const WORKTREE_DIR: &str = ".vicara-worktrees";
const WORKTREE_IGNORE_ENTRY: &str = ".vicara-worktrees/";

fn worktree_path(project_path: &str, task_id: &str) -> PathBuf {
    Path::new(project_path)
        .join(WORKTREE_DIR)
        .join(format!("task-{}", task_id))
}

fn project_root_preview_key(project_id: &str) -> String {
    format!("project-root:{}", project_id)
}

fn branch_name(task_id: &str) -> String {
    format!("feature/task-{}", task_id)
}

fn normalize_path_for_compare(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");

    #[cfg(windows)]
    {
        normalized.to_lowercase()
    }

    #[cfg(not(windows))]
    {
        normalized
    }
}

fn list_registered_worktree_paths(project_path: &Path) -> Result<Vec<String>, String> {
    let output = git::run_git(project_path, &["worktree", "list", "--porcelain"])?;
    Ok(output
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .map(|path| normalize_path_for_compare(&path))
        .collect())
}

fn is_registered_worktree_path(project_path: &Path, worktree_path: &Path) -> Result<bool, String> {
    let target = normalize_path_for_compare(worktree_path);
    let registered = list_registered_worktree_paths(project_path)?;
    Ok(registered.into_iter().any(|path| path == target))
}

fn branch_exists(project_path: &Path, branch_name: &str) -> Result<bool, String> {
    let (success, _, _) = git::run_git_raw(
        project_path,
        &[
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{}", branch_name),
        ],
    )?;
    Ok(success)
}

fn cleanup_stale_worktree_directory(project_path: &Path, worktree_path: &Path) {
    remove_worktree_node_modules_link(worktree_path);

    if worktree_path.exists() {
        let _ = std::fs::remove_dir_all(worktree_path);
    }

    let _ = git::run_git(project_path, &["worktree", "prune"]);
}

fn merge_failed_due_to_conflict(stdout: &str, stderr: &str) -> bool {
    !git::parse_conflict_files(&format!("{}\n{}", stdout, stderr)).is_empty()
        || stdout.contains("CONFLICT")
        || stderr.contains("CONFLICT")
}

fn infer_project_root_from_worktree_path(worktree_path: &Path) -> Option<PathBuf> {
    worktree_path.parent()?.parent().map(Path::to_path_buf)
}

fn preview_pid_from_record(record: Option<&db::WorktreeRecord>) -> Option<u32> {
    record
        .and_then(|item| item.preview_pid)
        .and_then(|pid| u32::try_from(pid).ok())
}

async fn stop_preview_for_task(
    app_handle: &AppHandle,
    preview_state: &PreviewState,
    task_id: &str,
) -> Result<bool, String> {
    let record = db::get_worktree_by_task_id(app_handle, task_id).await?;
    preview::stop_server_or_fallback_pid(
        preview_state,
        task_id,
        preview_pid_from_record(record.as_ref()),
    )
}

fn stop_project_root_preview_for_project(
    preview_state: &PreviewState,
    project_id: &str,
) -> Result<bool, String> {
    Ok(preview_state
        .stop_server(&project_root_preview_key(project_id))?
        .is_some())
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

fn append_unique_ignore_entry(
    file_path: &Path,
    entry: &str,
    read_label: &str,
    write_label: &str,
) -> Result<(), String> {
    let existing_content = if file_path.exists() {
        let content =
            std::fs::read_to_string(file_path).map_err(|e| format!("{}: {}", read_label, e))?;
        if content.lines().any(|line| line.trim() == entry) {
            return Ok(());
        }
        Some(content)
    } else {
        None
    };

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .map_err(|e| format!("{}: {}", write_label, e))?;

    if let Some(content) = existing_content {
        if !content.is_empty() && !content.ends_with('\n') {
            writeln!(file).map_err(|e| format!("{}: {}", write_label, e))?;
        }
    }
    writeln!(file, "{}", entry).map_err(|e| format!("{}: {}", write_label, e))?;

    Ok(())
}

fn normalize_lines_for_compare(content: &str) -> Vec<String> {
    let mut lines: Vec<String> = content
        .replace("\r\n", "\n")
        .split('\n')
        .map(|line| line.to_string())
        .collect();

    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }

    lines
}

fn contains_worktree_ignore_entry(content: &str) -> bool {
    content
        .lines()
        .any(|line| line.trim() == WORKTREE_IGNORE_ENTRY)
}

fn lines_without_worktree_ignore_entry(content: &str) -> Vec<String> {
    normalize_lines_for_compare(content)
        .into_iter()
        .filter(|line| line.trim() != WORKTREE_IGNORE_ENTRY)
        .collect()
}

fn ensure_local_exclude_entry(project_path: &Path) -> Result<(), String> {
    let exclude_path = git::resolve_git_internal_path(project_path, "info/exclude")?;
    if let Some(parent) = exclude_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!(".git/info ディレクトリ作成エラー: {}", e))?;
    }

    append_unique_ignore_entry(
        &exclude_path,
        WORKTREE_IGNORE_ENTRY,
        ".git/info/exclude読み込みエラー",
        ".git/info/exclude書き込みエラー",
    )
}

fn migrate_legacy_worktree_gitignore(project_path: &Path) -> Result<bool, String> {
    ensure_local_exclude_entry(project_path)?;

    let gitignore_path = project_path.join(".gitignore");
    if !gitignore_path.exists() {
        return Ok(false);
    }

    let current_content = std::fs::read_to_string(&gitignore_path)
        .map_err(|e| format!(".gitignore読み込みエラー: {}", e))?;
    if !contains_worktree_ignore_entry(&current_content) {
        return Ok(false);
    }

    if let Some(head_content) = git::read_head_file(project_path, ".gitignore")? {
        if contains_worktree_ignore_entry(&head_content) {
            return Ok(false);
        }

        if lines_without_worktree_ignore_entry(&current_content)
            == normalize_lines_for_compare(&head_content)
        {
            std::fs::write(&gitignore_path, head_content)
                .map_err(|e| format!(".gitignore移行書き込みエラー: {}", e))?;
            return Ok(true);
        }

        return Ok(false);
    }

    if lines_without_worktree_ignore_entry(&current_content).is_empty() {
        std::fs::remove_file(&gitignore_path)
            .map_err(|e| format!(".gitignore削除エラー: {}", e))?;
        return Ok(true);
    }

    Ok(false)
}

fn gitignore_has_legacy_worktree_entry(project_path: &Path) -> bool {
    let gitignore_path = project_path.join(".gitignore");
    if !gitignore_path.exists() {
        return false;
    }

    std::fs::read_to_string(gitignore_path)
        .map(|content| contains_worktree_ignore_entry(&content))
        .unwrap_or(false)
}

fn build_dirty_project_root_message(project_path: &Path, status: &str) -> String {
    let mut details = status
        .lines()
        .take(5)
        .map(|line| format!("- {}", line))
        .collect::<Vec<_>>();
    if status.lines().count() > 5 {
        details.push("- ...".to_string());
    }

    let mut message = format!(
        "プロジェクトルートに未コミット変更があるため、マージを開始できません。先に commit / stash / cleanup を行ってください。\n\n現在の変更:\n{}",
        details.join("\n")
    );

    if gitignore_has_legacy_worktree_entry(project_path) {
        message.push_str(
            "\n\n`.gitignore` に旧バージョンが追加した `.vicara-worktrees/` 差分が残っている可能性があります。今回のバージョンでは `.git/info/exclude` を使うため、app 起因の差分だけであれば cleanup 後に再試行してください。",
        );
    }

    message
}

fn ensure_merge_preflight_clean(project_path: &Path) -> Result<(), String> {
    let status = git::run_git(project_path, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        return Ok(());
    }

    Err(build_dirty_project_root_message(project_path, &status))
}

fn create_directory_link(source: &Path, target: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, target)
            .map_err(|e| format!("node_modules symlink作成エラー: {}", e))?;
    }

    #[cfg(windows)]
    {
        let symlink_result = std::os::windows::fs::symlink_dir(source, target);
        if symlink_result.is_err() {
            let output = Command::new("cmd")
                .args([
                    "/C",
                    "mklink",
                    "/J",
                    &target.to_string_lossy(),
                    &source.to_string_lossy(),
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

fn collect_node_modules_relative_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    fn visit(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in std::fs::read_dir(current).map_err(|error| {
            format!(
                "node_modules 探索中にディレクトリ読み取りに失敗しました ({}): {}",
                current.display(),
                error
            )
        })? {
            let entry = entry.map_err(|error| {
                format!(
                    "node_modules 探索中にエントリ読み取りに失敗しました ({}): {}",
                    current.display(),
                    error
                )
            })?;
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "node_modules 探索中に種別判定に失敗しました ({}): {}",
                    entry.path().display(),
                    error
                )
            })?;
            if !file_type.is_dir() && !file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();
            if file_name == ".git" || file_name == WORKTREE_DIR {
                continue;
            }

            if file_name == "node_modules" {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| {
                        format!(
                            "node_modules 相対パス化に失敗しました ({}): {}",
                            path.display(),
                            error
                        )
                    })?
                    .to_path_buf();
                out.push(relative);
                continue;
            }

            if file_type.is_dir() {
                visit(root, &path, out)?;
            }
        }
        Ok(())
    }

    let mut results = Vec::new();
    if root.exists() {
        visit(root, root, &mut results)?;
    }
    results.sort();
    Ok(results)
}

fn link_node_modules(project_path: &Path, wt_path: &Path) -> Result<(), String> {
    for relative_nm in collect_node_modules_relative_paths(project_path)? {
        let main_nm = project_path.join(&relative_nm);
        let wt_nm = wt_path.join(&relative_nm);
        if !main_nm.exists() || wt_nm.exists() {
            continue;
        }

        if let Some(parent) = wt_nm.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("node_modules 親ディレクトリ作成エラー: {}", e))?;
        }

        create_directory_link(&main_nm, &wt_nm)?;
    }

    Ok(())
}

fn project_root_from_worktree_path(wt_path: &Path) -> Result<PathBuf, String> {
    let Some(worktree_root) = wt_path.parent() else {
        return Err(format!(
            "worktree の親ディレクトリを解決できませんでした: {}",
            wt_path.display()
        ));
    };
    let Some(dir_name) = worktree_root.file_name().and_then(|name| name.to_str()) else {
        return Err(format!(
            "worktree ルート名を解決できませんでした: {}",
            worktree_root.display()
        ));
    };
    if dir_name != WORKTREE_DIR {
        return Err(format!(
            "worktree パスが想定外です ({} が `{}` 配下ではありません)",
            wt_path.display(),
            WORKTREE_DIR
        ));
    }

    worktree_root
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "worktree に対応する project root を解決できませんでした: {}",
                wt_path.display()
            )
        })
}

pub(crate) fn ensure_worktree_node_modules_links(wt_path: &Path) -> Result<(), String> {
    let project_root = project_root_from_worktree_path(wt_path)?;
    link_node_modules(&project_root, wt_path)
}

fn remove_worktree_node_modules_link(wt_path: &Path) {
    if let Ok(mut node_modules_paths) = collect_node_modules_relative_paths(wt_path) {
        node_modules_paths.sort_by(|a, b| {
            b.components()
                .count()
                .cmp(&a.components().count())
                .then_with(|| b.cmp(a))
        });

        for relative_nm in node_modules_paths {
            let wt_nm = wt_path.join(relative_nm);
            if wt_nm.is_symlink() || (cfg!(windows) && wt_nm.exists()) {
                let _ = if wt_nm.is_symlink() {
                    std::fs::remove_file(&wt_nm)
                } else {
                    std::fs::remove_dir(&wt_nm)
                };
            }
        }
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

fn cleanup_worktree_artifacts(project: &Path, wt_path: &Path, br_name: &str) -> Result<(), String> {
    let mut cleanup_logs = Vec::new();

    if let Err(error) = git::run_git(
        project,
        &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
    ) {
        cleanup_logs.push(format!("git worktree remove: {}", error));
    }

    if wt_path.exists() {
        std::fs::remove_dir_all(wt_path).map_err(|error| {
            format!(
                "ワークツリーディレクトリの削除に失敗しました ({}): {}",
                wt_path.display(),
                error
            )
        })?;
    }

    if let Err(error) = git::run_git(project, &["worktree", "prune"]) {
        cleanup_logs.push(format!("git worktree prune: {}", error));
    }

    if git::run_git(project, &["branch", "-d", br_name]).is_err() {
        if let Err(error) = git::run_git(project, &["branch", "-D", br_name]) {
            cleanup_logs.push(format!("git branch -D {}: {}", br_name, error));
        }
    }

    let worktree_registered = is_registered_worktree_path(project, wt_path)?;
    let worktree_directory_remaining = wt_path.exists();
    let branch_remaining = branch_exists(project, br_name)?;

    if !worktree_registered && !worktree_directory_remaining && !branch_remaining {
        return Ok(());
    }

    let mut reasons = Vec::new();
    if worktree_registered {
        reasons.push(format!(
            "git が worktree をまだ登録しています: {}",
            wt_path.display()
        ));
    }
    if worktree_directory_remaining {
        reasons.push(format!(
            "ワークツリーディレクトリが残っています: {}",
            wt_path.display()
        ));
    }
    if branch_remaining {
        reasons.push(format!("task branch が残っています: {}", br_name));
    }
    if !cleanup_logs.is_empty() {
        reasons.push(format!("cleanup log: {}", cleanup_logs.join(" / ")));
    }

    Err(format!(
        "ワークツリー削除を完了できませんでした。{}",
        reasons.join(" / ")
    ))
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
    migrate_legacy_worktree_gitignore(project)?;

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

    if wt_path.exists() && !is_registered_worktree_path(project, &wt_path)? {
        log::warn!(
            "Found stale worktree directory before create_worktree. Cleaning it up: {}",
            wt_path.display()
        );
        cleanup_stale_worktree_directory(project, &wt_path);
    }

    let parent = wt_path
        .parent()
        .ok_or("ワークツリーの親ディレクトリが不正です")?;
    std::fs::create_dir_all(parent).map_err(|e| format!("ディレクトリ作成エラー: {}", e))?;

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

    let _ = stop_preview_for_task(&app_handle, preview_state.inner(), &task_id).await;
    remove_worktree_node_modules_link(&wt_path);
    cleanup_worktree_artifacts(project, &wt_path, &br_name)?;

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
    migrate_legacy_worktree_gitignore(project)?;
    let wt_path = worktree_path(&project_path, &task_id);
    let br_name = branch_name(&task_id);
    let worktree_record = db::get_worktree_by_task_id(&app_handle, &task_id).await?;
    let worktree_registered = if wt_path.exists() {
        is_registered_worktree_path(project, &wt_path)?
    } else {
        false
    };

    {
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        if let Some(info) = worktrees.get_mut(&task_id) {
            info.status = WorktreeStatus::Merging;
        }
    }

    if wt_path.exists() && !worktree_registered {
        log::warn!(
            "merge_worktree detected stale unregistered directory. Cleaning it up: task_id={}, path={}",
            task_id,
            wt_path.display()
        );
        cleanup_stale_worktree_directory(project, &wt_path);
    }

    if !branch_exists(project, &br_name)? {
        {
            let mut worktrees = state
                .worktrees
                .lock()
                .map_err(|e| format!("State lock error: {}", e))?;
            worktrees.remove(&task_id);
        }

        let _ =
            db::update_worktree_record_state(&app_handle, &task_id, None, None, "removed").await;

        return Ok(MergeResult::Error {
            message: "マージ対象の task branch が見つかりません。古い競合状態を掃除したため、「AIで再実行する」で最新 main からやり直してください。".to_string(),
        });
    }

    if wt_path.exists() && worktree_registered {
        let _ = git::auto_commit_if_needed(&wt_path);
    }

    let current_branch = git::run_git(project, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    if current_branch != "main" {
        return Err(format!(
            "プロジェクトルートが main ブランチ上にありません（現在: {}）。マージ前に main へ切り替えてください。",
            current_branch
        ));
    }

    if let Err(message) = ensure_merge_preflight_clean(project) {
        {
            let mut worktrees = state
                .worktrees
                .lock()
                .map_err(|e| format!("State lock error: {}", e))?;
            if let Some(info) = worktrees.get_mut(&task_id) {
                info.status = WorktreeStatus::Active;
            }
        }

        return Ok(MergeResult::Error { message });
    }

    let (success, stdout, stderr) = git::run_git_raw(
        project,
        &[
            "merge",
            "--no-ff",
            "-m",
            &format!("[vicara] Merge task-{}", task_id),
            &br_name,
        ],
    )?;

    if !success {
        let _ = git::run_git(project, &["merge", "--abort"]);

        if !merge_failed_due_to_conflict(&stdout, &stderr) {
            {
                let mut worktrees = state
                    .worktrees
                    .lock()
                    .map_err(|e| format!("State lock error: {}", e))?;
                if let Some(info) = worktrees.get_mut(&task_id) {
                    info.status = WorktreeStatus::Active;
                }
            }

            return Ok(MergeResult::Error {
                message: if !stderr.is_empty() {
                    stderr
                } else if !stdout.is_empty() {
                    stdout
                } else {
                    "競合ではない理由でマージに失敗しました。".to_string()
                },
            });
        }

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

    let _ = preview::stop_server_or_fallback_pid(
        preview_state.inner(),
        &task_id,
        preview_pid_from_record(worktree_record.as_ref()),
    );
    if let Some(record) = worktree_record.as_ref() {
        let _ = stop_project_root_preview_for_project(preview_state.inner(), &record.project_id);
    }
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
        let mut worktrees = state
            .worktrees
            .lock()
            .map_err(|e| format!("State lock error: {}", e))?;
        if let Some(info) = worktrees.get(&task_id) {
            let worktree_path = PathBuf::from(&info.worktree_path);
            if is_registered_worktree_path(Path::new(&project_path), &worktree_path)? {
                return Ok(Some(info.clone()));
            }

            worktrees.remove(&task_id);
        }
    }

    let wt_path = worktree_path(&project_path, &task_id);
    if wt_path.exists() && is_registered_worktree_path(Path::new(&project_path), &wt_path)? {
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
pub async fn get_worktree_record(
    app_handle: AppHandle,
    task_id: String,
) -> Result<Option<db::WorktreeRecord>, String> {
    let Some(record) = db::get_worktree_by_task_id(&app_handle, &task_id).await? else {
        return Ok(None);
    };

    let worktree_path = PathBuf::from(&record.worktree_path);
    let Some(project_path) = infer_project_root_from_worktree_path(&worktree_path) else {
        return Ok(Some(record));
    };

    let registered = if worktree_path.exists() {
        is_registered_worktree_path(&project_path, &worktree_path)?
    } else {
        false
    };
    let branch_still_exists = branch_exists(&project_path, &record.branch_name)?;

    if registered && branch_still_exists {
        return Ok(Some(record));
    }

    if worktree_path.exists() && !registered {
        log::warn!(
            "get_worktree_record detected stale worktree directory. Cleaning it up: task_id={}, path={}",
            task_id,
            worktree_path.display()
        );
        cleanup_stale_worktree_directory(&project_path, &worktree_path);
    }

    let _ = db::update_worktree_record_state(&app_handle, &task_id, None, None, "removed").await;

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

    let worktree_record = db::get_worktree_by_task_id(&app_handle, &task_id).await?;
    let stopped_existing = preview::stop_server_or_fallback_pid(
        preview_state.inner(),
        &task_id,
        preview_pid_from_record(worktree_record.as_ref()),
    )?;
    if stopped_existing {
        let _ = db::update_worktree_record_state(&app_handle, &task_id, None, None, "active").await;
    }

    ensure_worktree_node_modules_links(&wt_path)?;
    let normalized_command = preview::normalize_preview_command(command);
    if let Some(installed_dirs) = node_dependencies::ensure_preview_dependencies_ready(
        &app_handle,
        &task_id,
        &wt_path,
        &normalized_command,
    )
    .await?
    {
        log::info!(
            "Preview dependency self-heal completed for task {}: {}",
            task_id,
            if installed_dirs.is_empty() {
                "(no package directories reported)".to_string()
            } else {
                installed_dirs.join(", ")
            }
        );
    }

    let info = preview::start_preview_for_task(
        &preview_state,
        &task_id,
        &wt_path,
        Some(normalized_command),
    )?;
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
pub async fn start_project_root_preview(
    app_handle: AppHandle,
    preview_state: State<'_, PreviewState>,
    project_id: String,
    project_path: String,
    command: Option<String>,
) -> Result<PreviewServerInfo, String> {
    let project_dir = PathBuf::from(&project_path);
    if !project_dir.exists() || !project_dir.is_dir() {
        return Err("プロジェクトルートが存在しないため、動作確認を開始できません。".to_string());
    }

    let normalized_command = preview::normalize_preview_command(command);
    if let Some(installed_dirs) = node_dependencies::ensure_preview_dependencies_ready(
        &app_handle,
        &project_id,
        &project_dir,
        &normalized_command,
    )
    .await?
    {
        log::info!(
            "Project root preview dependency self-heal completed for project {}: {}",
            project_id,
            if installed_dirs.is_empty() {
                "(no package directories reported)".to_string()
            } else {
                installed_dirs.join(", ")
            }
        );
    }

    preview::start_preview_for_task(
        &preview_state,
        &project_root_preview_key(&project_id),
        &project_dir,
        Some(normalized_command),
    )
}

#[tauri::command]
pub async fn get_project_root_preview(
    preview_state: State<'_, PreviewState>,
    project_id: String,
) -> Result<Option<PreviewServerInfo>, String> {
    preview_state.get_info(&project_root_preview_key(&project_id))
}

#[tauri::command]
pub async fn stop_project_root_preview(
    preview_state: State<'_, PreviewState>,
    project_id: String,
) -> Result<bool, String> {
    stop_project_root_preview_for_project(preview_state.inner(), &project_id)
}

#[tauri::command]
pub async fn stop_preview_server(
    app_handle: AppHandle,
    preview_state: State<'_, PreviewState>,
    task_id: String,
) -> Result<bool, String> {
    let stopped = stop_preview_for_task(&app_handle, preview_state.inner(), &task_id).await?;
    let _ = db::update_worktree_record_state(&app_handle, &task_id, None, None, "active").await;
    Ok(stopped)
}

#[tauri::command]
pub async fn open_preview_in_browser(app_handle: AppHandle, url: String) -> Result<(), String> {
    preview::open_preview_in_browser(&app_handle, &url)
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

#[tauri::command]
pub async fn open_project_root_static_preview(
    app_handle: AppHandle,
    project_path: String,
) -> Result<String, String> {
    let project_dir = PathBuf::from(&project_path);
    if !project_dir.exists() || !project_dir.is_dir() {
        return Err("プロジェクトルートが存在しないため、動作確認を開始できません。".to_string());
    }

    let index_path = project_dir.join("index.html");
    if !index_path.exists() {
        return Err("プロジェクトルートに index.html が見つかりません。静的サイト構成か確認してください。".to_string());
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
            PathBuf::from("/project/.vicara-worktrees/task-abc123")
        );
    }

    #[test]
    fn test_branch_name_construction() {
        assert_eq!(branch_name("abc123"), "feature/task-abc123");
    }

    #[test]
    fn test_preview_pid_from_record_converts_positive_i64() {
        let record = db::WorktreeRecord {
            id: "wt-1".to_string(),
            task_id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            worktree_path: "C:/repo/.vicara-worktrees/task-1".to_string(),
            branch_name: "feature/task-1".to_string(),
            preview_port: Some(5173),
            preview_pid: Some(4242),
            status: "active".to_string(),
            created_at: "2026-04-18 00:00:00".to_string(),
            updated_at: "2026-04-18 00:00:00".to_string(),
        };

        assert_eq!(preview_pid_from_record(Some(&record)), Some(4242));
        assert_eq!(preview_pid_from_record(None), None);
    }

    #[test]
    fn test_create_and_remove_worktree() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "test-001";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);
        let exclude_path = git::resolve_git_internal_path(repo.path(), "info/exclude").unwrap();

        ensure_local_exclude_entry(repo.path()).expect("exclude failed");

        let exclude = fs::read_to_string(&exclude_path).unwrap();
        assert!(exclude.contains(".vicara-worktrees/"));
        assert!(
            !repo.path().join(".gitignore").exists(),
            "tracked .gitignore should not be touched"
        );

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

        cleanup_worktree_artifacts(repo.path(), &wt_path, &br_name).unwrap();

        assert!(!wt_path.exists());
        let branches = git::run_git(repo.path(), &["branch"]).unwrap();
        assert!(!branches.contains("feature/task-test-001"));
    }

    #[test]
    fn test_cleanup_worktree_artifacts_handles_stale_directory_and_branch() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "stale-branch";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        fs::create_dir_all(&wt_path).unwrap();
        fs::write(wt_path.join("placeholder.txt"), "stale\n").unwrap();
        git::run_git(repo.path(), &["branch", &br_name, "main"]).unwrap();

        cleanup_worktree_artifacts(repo.path(), &wt_path, &br_name).unwrap();

        assert!(!wt_path.exists(), "stale directory should be removed");
        assert!(
            !branch_exists(repo.path(), &br_name).unwrap(),
            "stale task branch should be removed"
        );
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
                &format!("[vicara] Merge task-{}", task_id),
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
    fn test_plain_directory_is_not_treated_as_registered_worktree() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "stale-dir";
        let wt_path = worktree_path(&project_path, task_id);

        fs::create_dir_all(&wt_path).unwrap();
        fs::write(wt_path.join("placeholder.txt"), "stale\n").unwrap();

        assert!(
            !is_registered_worktree_path(repo.path(), &wt_path).unwrap(),
            "plain nested directory must not be treated as a git worktree"
        );
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
        let frontend_nm_dir = repo.path().join("frontend").join("node_modules");
        fs::create_dir_all(frontend_nm_dir.join("vite")).unwrap();
        fs::write(frontend_nm_dir.join("vite/index.js"), "export default {};").unwrap();

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
        let wt_frontend_nm = wt_path.join("frontend").join("node_modules");
        assert!(
            wt_frontend_nm.exists(),
            "nested node_modules link should exist"
        );
        assert!(
            wt_frontend_nm.join("vite/index.js").exists(),
            "Should be able to access nested files through symlink"
        );

        remove_worktree_node_modules_link(&wt_path);
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
    fn test_ensure_local_exclude_idempotent() {
        let repo = setup_test_repo();
        let exclude_path = git::resolve_git_internal_path(repo.path(), "info/exclude").unwrap();

        ensure_local_exclude_entry(repo.path()).unwrap();
        ensure_local_exclude_entry(repo.path()).unwrap();

        let content = fs::read_to_string(exclude_path).unwrap();
        let count = content
            .lines()
            .filter(|line| line.trim() == ".vicara-worktrees/")
            .count();
        assert_eq!(count, 1, "Should appear exactly once, got: {}", count);
    }

    #[test]
    fn test_migrate_legacy_gitignore_entry_restores_tracked_file() {
        let repo = setup_test_repo();
        let gitignore_path = repo.path().join(".gitignore");
        let exclude_path = git::resolve_git_internal_path(repo.path(), "info/exclude").unwrap();

        fs::write(&gitignore_path, "node_modules/\n").unwrap();
        git::run_git(repo.path(), &["add", ".gitignore"]).unwrap();
        git::run_git(repo.path(), &["commit", "-m", "Add gitignore"]).unwrap();

        fs::write(&gitignore_path, "node_modules/\n.vicara-worktrees/\n").unwrap();

        assert!(migrate_legacy_worktree_gitignore(repo.path()).unwrap());
        assert_eq!(
            fs::read_to_string(&gitignore_path).unwrap(),
            "node_modules/\n"
        );
        assert!(fs::read_to_string(exclude_path)
            .unwrap()
            .contains(".vicara-worktrees/"));
        assert_eq!(
            git::run_git(repo.path(), &["status", "--porcelain"]).unwrap(),
            ""
        );
    }

    #[test]
    fn test_migrate_legacy_gitignore_entry_removes_untracked_app_file() {
        let repo = setup_test_repo();
        let gitignore_path = repo.path().join(".gitignore");
        let exclude_path = git::resolve_git_internal_path(repo.path(), "info/exclude").unwrap();

        fs::write(&gitignore_path, ".vicara-worktrees/\n").unwrap();

        assert!(migrate_legacy_worktree_gitignore(repo.path()).unwrap());
        assert!(
            !gitignore_path.exists(),
            "legacy app-created .gitignore should be removed"
        );
        assert!(fs::read_to_string(exclude_path)
            .unwrap()
            .contains(".vicara-worktrees/"));
        assert_eq!(
            git::run_git(repo.path(), &["status", "--porcelain"]).unwrap(),
            ""
        );
    }

    #[test]
    fn test_migrate_legacy_gitignore_entry_keeps_other_local_changes() {
        let repo = setup_test_repo();
        let gitignore_path = repo.path().join(".gitignore");

        fs::write(&gitignore_path, "node_modules/\n").unwrap();
        git::run_git(repo.path(), &["add", ".gitignore"]).unwrap();
        git::run_git(repo.path(), &["commit", "-m", "Add gitignore"]).unwrap();

        fs::write(
            &gitignore_path,
            "node_modules/\ncustom/\n.vicara-worktrees/\n",
        )
        .unwrap();

        assert!(!migrate_legacy_worktree_gitignore(repo.path()).unwrap());
        assert_eq!(
            fs::read_to_string(&gitignore_path).unwrap(),
            "node_modules/\ncustom/\n.vicara-worktrees/\n"
        );
    }

    #[test]
    fn test_merge_preflight_blocks_dirty_project_root() {
        let repo = setup_test_repo();

        fs::write(repo.path().join(".gitignore"), "node_modules/\n").unwrap();
        git::run_git(repo.path(), &["add", ".gitignore"]).unwrap();
        git::run_git(repo.path(), &["commit", "-m", "Add gitignore"]).unwrap();

        fs::write(
            repo.path().join(".gitignore"),
            "node_modules/\ncustom/\n.vicara-worktrees/\n",
        )
        .unwrap();

        assert!(!migrate_legacy_worktree_gitignore(repo.path()).unwrap());

        let error = ensure_merge_preflight_clean(repo.path()).unwrap_err();
        assert!(error.contains("commit / stash / cleanup"));
        assert!(error.contains(".gitignore"));
        assert!(error.contains(".vicara-worktrees/"));
    }

    #[test]
    fn test_merge_regression_when_task_branch_changes_gitignore() {
        let repo = setup_test_repo();
        let project_path = repo.path().to_string_lossy().to_string();
        let task_id = "gitignore-merge";
        let wt_path = worktree_path(&project_path, task_id);
        let br_name = branch_name(task_id);

        ensure_local_exclude_entry(repo.path()).unwrap();
        assert_eq!(
            git::run_git(repo.path(), &["status", "--porcelain"]).unwrap(),
            ""
        );

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

        fs::write(wt_path.join(".gitignore"), "dist/\n").unwrap();
        git::run_git(&wt_path, &["add", ".gitignore"]).unwrap();
        git::run_git(&wt_path, &["commit", "-m", "Add worktree gitignore"]).unwrap();

        ensure_merge_preflight_clean(repo.path()).unwrap();

        let (success, _, stderr) = git::run_git_raw(
            repo.path(),
            &[
                "merge",
                "--no-ff",
                "-m",
                &format!("[vicara] Merge task-{}", task_id),
                &br_name,
            ],
        )
        .unwrap();

        assert!(success, "Merge failed: {}", stderr);
        let merged_gitignore = fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        assert_eq!(
            normalize_lines_for_compare(&merged_gitignore),
            vec!["dist/".to_string()]
        );

        let _ = git::run_git(
            repo.path(),
            &["worktree", "remove", &wt_path.to_string_lossy(), "--force"],
        );
        let _ = git::run_git(repo.path(), &["worktree", "prune"]);
        let _ = git::run_git(repo.path(), &["branch", "-d", &br_name]);
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
