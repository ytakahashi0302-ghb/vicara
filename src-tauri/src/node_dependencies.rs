use regex::Regex;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use tauri::{AppHandle, Emitter};

const SCAFFOLD_OUTPUT_EVENT: &str = "scaffold_output";
const AGENT_CLI_OUTPUT_EVENT: &str = "agent_cli_output";
const NODE_MANIFEST_FILES: &[&str] = &[
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NodePackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl NodePackageManager {
    pub(crate) fn command_line(&self) -> &'static str {
        match self {
            Self::Npm => "npm install",
            Self::Pnpm => "pnpm install",
            Self::Yarn => "yarn install",
            Self::Bun => "bun install",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NodeInstallPlan {
    pub(crate) working_dir: PathBuf,
    pub(crate) relative_dir: String,
    package_manager: NodePackageManager,
}

impl NodeInstallPlan {
    pub(crate) fn command_line(&self) -> &'static str {
        self.package_manager.command_line()
    }
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
pub(crate) struct PackageJsonManifest {
    #[serde(rename = "packageManager")]
    package_manager: Option<String>,
    #[serde(default)]
    scripts: HashMap<String, String>,
    #[serde(default)]
    workspaces: Option<serde_json::Value>,
}

#[derive(Clone)]
pub(crate) enum NodeInstallOutputTarget {
    Scaffold,
    AgentTask { task_id: String },
    Silent,
}

#[derive(Clone, serde::Serialize)]
struct TextOutputPayload {
    output: String,
}

#[derive(Clone, serde::Serialize)]
struct AgentOutputPayload {
    task_id: String,
    output: String,
}

fn emit_output(
    app_handle: &AppHandle,
    target: &NodeInstallOutputTarget,
    output: impl Into<String>,
) {
    let output = output.into();
    match target {
        NodeInstallOutputTarget::Scaffold => {
            let _ = app_handle.emit(
                SCAFFOLD_OUTPUT_EVENT,
                TextOutputPayload {
                    output: output.clone(),
                },
            );
        }
        NodeInstallOutputTarget::AgentTask { task_id } => {
            let _ = app_handle.emit(
                AGENT_CLI_OUTPUT_EVENT,
                AgentOutputPayload {
                    task_id: task_id.clone(),
                    output,
                },
            );
        }
        NodeInstallOutputTarget::Silent => {
            log::info!(target: "vicara::node_dependencies", "{}", output);
        }
    }
}

fn normalize_relative_package_dir(relative_path: &str) -> Result<PathBuf, String> {
    let candidate = relative_path.trim();
    if candidate.is_empty() {
        return Err("空の package 参照パスは使用できません。".to_string());
    }

    let candidate = candidate.replace('\\', "/");
    let path = Path::new(&candidate);
    if path.is_absolute() {
        return Err(format!(
            "絶対パスの package 参照は使用できません: {}",
            relative_path
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!(
                    "親ディレクトリ参照を含む package 参照は使用できません: {}",
                    relative_path
                ));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(format!("無効な package 参照パスです: {}", relative_path));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(format!("無効な package 参照パスです: {}", relative_path));
    }

    Ok(normalized)
}

pub(crate) fn read_package_json_manifest(
    package_json_path: &Path,
) -> Result<PackageJsonManifest, String> {
    let content = std::fs::read_to_string(package_json_path).map_err(|error| {
        format!(
            "package.json の読み取りに失敗しました ({}): {}",
            package_json_path.display(),
            error
        )
    })?;
    serde_json::from_str::<PackageJsonManifest>(&content).map_err(|error| {
        format!(
            "package.json を JSON として解釈できませんでした ({}): {}",
            package_json_path.display(),
            error
        )
    })
}

pub(crate) fn detect_node_package_manager(
    package_dir: &Path,
    manifest: &PackageJsonManifest,
) -> NodePackageManager {
    if let Some(package_manager) = manifest.package_manager.as_deref() {
        let normalized = package_manager.trim().to_ascii_lowercase();
        if normalized.starts_with("pnpm@") {
            return NodePackageManager::Pnpm;
        }
        if normalized.starts_with("yarn@") {
            return NodePackageManager::Yarn;
        }
        if normalized.starts_with("bun@") {
            return NodePackageManager::Bun;
        }
        if normalized.starts_with("npm@") {
            return NodePackageManager::Npm;
        }
    }

    if package_dir.join("pnpm-lock.yaml").exists() {
        NodePackageManager::Pnpm
    } else if package_dir.join("yarn.lock").exists() {
        NodePackageManager::Yarn
    } else if package_dir.join("bun.lockb").exists() || package_dir.join("bun.lock").exists() {
        NodePackageManager::Bun
    } else {
        NodePackageManager::Npm
    }
}

pub(crate) fn extract_prefixed_package_dirs(manifest: &PackageJsonManifest) -> Vec<PathBuf> {
    let prefix_pattern =
        Regex::new(r#"--prefix\s+(?:"([^"]+)"|'([^']+)'|([^\s&|;]+))"#).expect("valid regex");
    let mut directories = Vec::new();
    let mut seen = HashSet::new();

    for script in manifest.scripts.values() {
        for captures in prefix_pattern.captures_iter(script) {
            let candidate = captures
                .get(1)
                .or_else(|| captures.get(2))
                .or_else(|| captures.get(3))
                .map(|value| value.as_str().trim())
                .filter(|value| !value.is_empty());
            let Some(candidate) = candidate else {
                continue;
            };
            if seen.insert(candidate.to_string()) {
                directories.push(PathBuf::from(candidate));
            }
        }
    }

    directories.sort();
    directories
}

pub(crate) fn discover_node_install_plans(
    project_root: &Path,
) -> Result<Vec<NodeInstallPlan>, String> {
    let root_package_json = project_root.join("package.json");
    if !root_package_json.exists() {
        return Ok(Vec::new());
    }

    let root_manifest = read_package_json_manifest(&root_package_json)?;
    let mut plans = vec![NodeInstallPlan {
        working_dir: project_root.to_path_buf(),
        relative_dir: ".".to_string(),
        package_manager: detect_node_package_manager(project_root, &root_manifest),
    }];

    if root_manifest.workspaces.is_some() {
        return Ok(plans);
    }

    let mut seen = HashSet::from([".".to_string()]);
    for relative_dir in extract_prefixed_package_dirs(&root_manifest) {
        let normalized = normalize_relative_package_dir(&relative_dir.to_string_lossy())?;
        let package_dir = project_root.join(&normalized);
        let package_json = package_dir.join("package.json");
        if !package_json.exists() {
            continue;
        }

        let relative_display = normalized.to_string_lossy().replace('\\', "/");
        if !seen.insert(relative_display.clone()) {
            continue;
        }

        let manifest = read_package_json_manifest(&package_json)?;
        plans.push(NodeInstallPlan {
            working_dir: package_dir.clone(),
            relative_dir: relative_display,
            package_manager: detect_node_package_manager(&package_dir, &manifest),
        });
    }

    plans.sort_by_key(|plan| {
        if plan.relative_dir == "." {
            0
        } else {
            plan.relative_dir.matches('/').count() + 1
        }
    });

    Ok(plans)
}

#[cfg(target_os = "windows")]
fn spawn_install_shell_process(cwd: &Path, command_line: &str) -> Result<Child, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    Command::new("cmd")
        .args(["/C", command_line])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| {
            format!(
                "依存関係インストールコマンドの起動に失敗しました ({} / `{}`): {}",
                cwd.display(),
                command_line,
                error
            )
        })
}

#[cfg(not(target_os = "windows"))]
fn spawn_install_shell_process(cwd: &Path, command_line: &str) -> Result<Child, String> {
    Command::new("sh")
        .args(["-lc", command_line])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            format!(
                "依存関係インストールコマンドの起動に失敗しました ({} / `{}`): {}",
                cwd.display(),
                command_line,
                error
            )
        })
}

fn spawn_install_output_reader<R>(
    app_handle: AppHandle,
    target: NodeInstallOutputTarget,
    relative_dir: String,
    stream_name: &'static str,
    reader: R,
) -> thread::JoinHandle<()>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let display_dir = if relative_dir == "." {
            "root".to_string()
        } else {
            relative_dir
        };
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            emit_output(
                &app_handle,
                &target,
                format!("[deps:{}:{}] {}", display_dir, stream_name, line),
            );
        }
    })
}

pub(crate) async fn install_node_dependencies(
    app_handle: &AppHandle,
    project_root: &Path,
    target: NodeInstallOutputTarget,
    phase_label: &str,
) -> Result<Vec<String>, String> {
    let plans = discover_node_install_plans(project_root)?;
    if plans.is_empty() {
        emit_output(
            app_handle,
            &target,
            format!(
                "package.json が見つからないため、{}の依存導入はスキップしました。",
                phase_label
            ),
        );
        return Ok(Vec::new());
    }

    let mut completed = Vec::new();
    for plan in plans {
        let install_label = if plan.relative_dir == "." {
            "プロジェクト直下".to_string()
        } else {
            format!("`{}`", plan.relative_dir)
        };
        emit_output(
            app_handle,
            &target,
            format!(
                "{}の依存導入を開始します: {} で `{}`",
                phase_label,
                install_label,
                plan.command_line()
            ),
        );

        let completed_label = plan.relative_dir.clone();
        let app_handle = app_handle.clone();
        let target_for_task = target.clone();
        let phase_label = phase_label.to_string();
        tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
            let mut child = spawn_install_shell_process(&plan.working_dir, plan.command_line())?;
            let stdout = child.stdout.take().ok_or_else(|| {
                "依存関係インストールの stdout を取得できませんでした".to_string()
            })?;
            let stderr = child.stderr.take().ok_or_else(|| {
                "依存関係インストールの stderr を取得できませんでした".to_string()
            })?;

            let stdout_handle = spawn_install_output_reader(
                app_handle.clone(),
                target_for_task.clone(),
                plan.relative_dir.clone(),
                "stdout",
                stdout,
            );
            let stderr_handle = spawn_install_output_reader(
                app_handle.clone(),
                target_for_task.clone(),
                plan.relative_dir.clone(),
                "stderr",
                stderr,
            );

            let status = child.wait().map_err(|error| {
                format!(
                    "依存関係インストールの完了待機に失敗しました ({} / `{}`): {}",
                    plan.working_dir.display(),
                    plan.command_line(),
                    error
                )
            })?;

            let _ = stdout_handle.join();
            let _ = stderr_handle.join();

            if !status.success() {
                return Err(format!(
                    "{}の依存導入に失敗しました ({} / `{}` / exit code: {:?})",
                    phase_label,
                    if plan.relative_dir == "." {
                        "root".to_string()
                    } else {
                        plan.relative_dir.clone()
                    },
                    plan.command_line(),
                    status.code()
                ));
            }

            if !plan.working_dir.join("node_modules").exists() {
                return Err(format!(
                    "依存導入コマンドは成功しましたが node_modules が見つかりませんでした: {}",
                    plan.working_dir.display()
                ));
            }

            Ok(())
        })
        .await
        .map_err(|join_error| format!("依存導入スレッドの実行に失敗しました: {}", join_error))??;

        completed.push(completed_label);
    }

    Ok(completed)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ScriptVisitKey {
    package_dir: PathBuf,
    script_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScriptInvocation {
    package_dir: PathBuf,
    script_name: String,
}

fn package_manager_run_invocations(script: &str) -> Vec<ScriptInvocation> {
    let npm_like = Regex::new(
        r#"(?i)\b(?:npm|pnpm|bun)\s+(?:--prefix\s+(?:"([^"]+)"|'([^']+)'|([^\s&|;]+))\s+)?run\s+([^\s&|;]+)"#,
    )
    .expect("valid regex");
    let yarn = Regex::new(
        r#"(?i)\byarn\s+(?:--cwd\s+(?:"([^"]+)"|'([^']+)'|([^\s&|;]+))\s+)?(?:run\s+)?([^\s&|;]+)"#,
    )
    .expect("valid regex");

    let mut invocations = Vec::new();
    for captures in npm_like.captures_iter(script) {
        let package_dir = captures
            .get(1)
            .or_else(|| captures.get(2))
            .or_else(|| captures.get(3))
            .map(|value| PathBuf::from(value.as_str().trim()))
            .unwrap_or_else(|| PathBuf::from("."));
        let Some(script_name) = captures
            .get(4)
            .map(|value| value.as_str().trim().trim_matches('"').trim_matches('\''))
        else {
            continue;
        };
        if script_name.is_empty() {
            continue;
        }
        invocations.push(ScriptInvocation {
            package_dir,
            script_name: script_name.to_string(),
        });
    }

    for captures in yarn.captures_iter(script) {
        let package_dir = captures
            .get(1)
            .or_else(|| captures.get(2))
            .or_else(|| captures.get(3))
            .map(|value| PathBuf::from(value.as_str().trim()))
            .unwrap_or_else(|| PathBuf::from("."));
        let Some(script_name) = captures
            .get(4)
            .map(|value| value.as_str().trim().trim_matches('"').trim_matches('\''))
        else {
            continue;
        };
        if script_name.is_empty() {
            continue;
        }
        invocations.push(ScriptInvocation {
            package_dir,
            script_name: script_name.to_string(),
        });
    }

    invocations
}

fn inline_script_refs(script: &str) -> Vec<String> {
    let regex =
        Regex::new(r#"(?i)\b(?:npm|pnpm|yarn|bun):([A-Za-z0-9:_\-]+)\b"#).expect("valid regex");
    let mut script_names = BTreeSet::new();
    for captures in regex.captures_iter(script) {
        if let Some(script_name) = captures.get(1).map(|value| value.as_str().trim()) {
            if !script_name.is_empty() {
                script_names.insert(script_name.to_string());
            }
        }
    }
    script_names.into_iter().collect()
}

fn command_tokens_from_script(script: &str) -> Vec<String> {
    let regex = Regex::new(
        r#"(?i)(?:^|&&|\|\||;|\|)\s*(?:[A-Za-z_][A-Za-z0-9_]*=[^\s]+\s+)*([A-Za-z0-9_.:@/\-]+)"#,
    )
    .expect("valid regex");
    let mut commands = Vec::new();
    let mut seen = HashSet::new();
    for captures in regex.captures_iter(script) {
        let Some(command) = captures.get(1).map(|value| value.as_str().trim()) else {
            continue;
        };
        if command.is_empty() || !seen.insert(command.to_string()) {
            continue;
        }
        commands.push(command.to_string());
    }
    commands
}

fn is_shell_builtin(command: &str) -> bool {
    matches!(
        command.to_ascii_lowercase().as_str(),
        "npm"
            | "npx"
            | "pnpm"
            | "pnpx"
            | "yarn"
            | "bun"
            | "node"
            | "python"
            | "python3"
            | "py"
            | "cmd"
            | "powershell"
            | "pwsh"
            | "sh"
            | "bash"
            | "echo"
            | "cd"
            | "set"
            | "export"
            | "start"
    )
}

fn local_binary_exists(package_dir: &Path, command: &str) -> bool {
    let normalized = command.trim_matches('"').trim_matches('\'');
    if normalized.is_empty()
        || normalized.contains('/')
        || normalized.contains('\\')
        || normalized.starts_with('.')
        || is_shell_builtin(normalized)
    {
        return true;
    }

    let bin_dir = package_dir.join("node_modules").join(".bin");
    if !bin_dir.exists() {
        return false;
    }

    let candidates = if cfg!(windows) {
        vec![
            normalized.to_string(),
            format!("{}.cmd", normalized),
            format!("{}.ps1", normalized),
            format!("{}.exe", normalized),
        ]
    } else {
        vec![normalized.to_string()]
    };

    candidates
        .into_iter()
        .any(|candidate| bin_dir.join(candidate).exists())
}

fn resolve_package_dir(base_dir: &Path, relative_dir: &Path) -> PathBuf {
    if relative_dir == Path::new(".") || relative_dir.as_os_str().is_empty() {
        base_dir.to_path_buf()
    } else {
        base_dir.join(relative_dir)
    }
}

fn collect_missing_binaries_for_script(
    package_dir: &Path,
    script_name: &str,
    visited: &mut HashSet<ScriptVisitKey>,
    missing: &mut BTreeSet<String>,
) -> Result<(), String> {
    let visit_key = ScriptVisitKey {
        package_dir: package_dir.to_path_buf(),
        script_name: script_name.to_string(),
    };
    if !visited.insert(visit_key) {
        return Ok(());
    }

    let package_json = package_dir.join("package.json");
    if !package_json.exists() {
        return Ok(());
    }

    let manifest = read_package_json_manifest(&package_json)?;
    let Some(script) = manifest.scripts.get(script_name) else {
        return Ok(());
    };

    for command in command_tokens_from_script(script) {
        if local_binary_exists(package_dir, &command) {
            continue;
        }
        missing.insert(format!(
            "{} の script `{}` で local binary `{}` が見つかりません",
            package_dir.display(),
            script_name,
            command
        ));
    }

    for nested in inline_script_refs(script) {
        collect_missing_binaries_for_script(package_dir, &nested, visited, missing)?;
    }

    for invocation in package_manager_run_invocations(script) {
        let nested_dir = resolve_package_dir(package_dir, &invocation.package_dir);
        collect_missing_binaries_for_script(
            &nested_dir,
            &invocation.script_name,
            visited,
            missing,
        )?;
    }

    Ok(())
}

fn parse_preview_command_invocation(preview_command: &str) -> Option<ScriptInvocation> {
    let invocations = package_manager_run_invocations(preview_command);
    invocations.into_iter().next()
}

pub(crate) fn collect_preview_dependency_issues(
    project_root: &Path,
    preview_command: &str,
) -> Result<Vec<String>, String> {
    let plans = discover_node_install_plans(project_root)?;
    let mut issues = BTreeSet::new();

    for plan in &plans {
        if !plan.working_dir.join("node_modules").exists() {
            issues.insert(format!(
                "{} に node_modules がありません",
                if plan.relative_dir == "." {
                    "root".to_string()
                } else {
                    plan.relative_dir.clone()
                }
            ));
        }
    }

    if let Some(invocation) = parse_preview_command_invocation(preview_command) {
        let package_dir = resolve_package_dir(project_root, &invocation.package_dir);
        let mut visited = HashSet::new();
        collect_missing_binaries_for_script(
            &package_dir,
            &invocation.script_name,
            &mut visited,
            &mut issues,
        )?;
    }

    Ok(issues.into_iter().collect())
}

pub(crate) async fn ensure_preview_dependencies_ready(
    app_handle: &AppHandle,
    task_id: &str,
    worktree_path: &Path,
    preview_command: &str,
) -> Result<Option<Vec<String>>, String> {
    let issues = collect_preview_dependency_issues(worktree_path, preview_command)?;
    if issues.is_empty() {
        return Ok(None);
    }

    log::warn!(
        target: "vicara::preview",
        "Preview dependency self-heal triggered for task {}: {}",
        task_id,
        issues.join(" / ")
    );

    let installed_dirs = install_node_dependencies(
        app_handle,
        worktree_path,
        NodeInstallOutputTarget::Silent,
        "Preview 起動前",
    )
    .await?;

    Ok(Some(installed_dirs))
}

pub(crate) fn changed_node_manifest_paths(changed_files: &[String]) -> Vec<String> {
    let mut manifests = BTreeSet::new();
    for path in changed_files {
        let normalized = path
            .trim()
            .replace('\\', "/")
            .trim_start_matches("./")
            .to_string();
        let Some(file_name) = normalized.split('/').next_back() else {
            continue;
        };
        if NODE_MANIFEST_FILES
            .iter()
            .any(|candidate| file_name.eq_ignore_ascii_case(candidate))
        {
            manifests.insert(normalized);
        }
    }

    manifests.into_iter().collect()
}

pub(crate) fn has_node_manifest_changes(changed_files: &[String]) -> bool {
    !changed_node_manifest_paths(changed_files).is_empty()
}

#[cfg(test)]
mod tests {
    use super::{
        changed_node_manifest_paths, collect_preview_dependency_issues,
        detect_node_package_manager, discover_node_install_plans, extract_prefixed_package_dirs,
        has_node_manifest_changes, NodePackageManager, PackageJsonManifest,
    };
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    #[test]
    fn changed_node_manifest_paths_detects_root_and_nested_manifests() {
        let changed = vec![
            "src/App.tsx".to_string(),
            "./package.json".to_string(),
            "frontend\\package-lock.json".to_string(),
            "README.md".to_string(),
        ];

        let result = changed_node_manifest_paths(&changed);

        assert_eq!(
            result,
            vec![
                "frontend/package-lock.json".to_string(),
                "package.json".to_string()
            ]
        );
        assert!(has_node_manifest_changes(&changed));
    }

    #[test]
    fn extract_prefixed_package_dirs_reads_prefix_from_scripts() {
        let manifest = PackageJsonManifest {
            scripts: HashMap::from([
                (
                    "dev".to_string(),
                    "concurrently \"npm:dev:api\" \"npm --prefix frontend run dev\"".to_string(),
                ),
                (
                    "lint".to_string(),
                    "npm --prefix admin run lint".to_string(),
                ),
            ]),
            ..PackageJsonManifest::default()
        };

        let result = extract_prefixed_package_dirs(&manifest);

        assert_eq!(
            result,
            vec![
                Path::new("admin").to_path_buf(),
                Path::new("frontend").to_path_buf()
            ]
        );
    }

    #[test]
    fn detect_node_package_manager_prefers_manifest_then_lockfile() {
        let tempdir = tempfile::tempdir().expect("tempdir should exist");
        fs::write(
            tempdir.path().join("pnpm-lock.yaml"),
            "lockfileVersion: '9.0'",
        )
        .expect("lockfile should exist");
        let manifest = PackageJsonManifest::default();

        assert_eq!(
            detect_node_package_manager(tempdir.path(), &manifest),
            NodePackageManager::Pnpm
        );

        let manifest = PackageJsonManifest {
            package_manager: Some("bun@1.2.0".to_string()),
            ..PackageJsonManifest::default()
        };
        assert_eq!(
            detect_node_package_manager(tempdir.path(), &manifest),
            NodePackageManager::Bun
        );
    }

    #[test]
    fn discover_node_install_plans_includes_root_and_prefix_packages() {
        let project_dir = tempfile::tempdir().expect("project tempdir should exist");
        fs::write(
            project_dir.path().join("package.json"),
            r#"{
  "scripts": {
    "dev": "concurrently -n api,web \"npm:dev:api\" \"npm --prefix frontend run dev\""
  }
}"#,
        )
        .expect("root package.json should exist");
        fs::create_dir_all(project_dir.path().join("frontend")).expect("frontend dir should exist");
        fs::write(
            project_dir.path().join("frontend").join("package.json"),
            r#"{"packageManager":"npm@10.0.0"}"#,
        )
        .expect("frontend package.json should exist");

        let plans =
            discover_node_install_plans(project_dir.path()).expect("plans should be discovered");

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].relative_dir, ".");
        assert_eq!(plans[1].relative_dir, "frontend");
    }

    #[test]
    fn collect_preview_dependency_issues_detects_missing_root_and_nested_bins() {
        let project_dir = tempfile::tempdir().expect("project tempdir should exist");
        fs::write(
            project_dir.path().join("package.json"),
            r#"{
  "scripts": {
    "dev": "concurrently -n api,web \"npm:dev:api\" \"npm --prefix frontend run dev\"",
    "dev:api": "node server.js"
  }
}"#,
        )
        .expect("root package.json should exist");
        fs::create_dir_all(project_dir.path().join("node_modules").join(".bin"))
            .expect("root bin dir should exist");
        fs::write(
            project_dir
                .path()
                .join("node_modules")
                .join(".bin")
                .join("concurrently.cmd"),
            "@echo off",
        )
        .expect("root binary should exist");

        let frontend_dir = project_dir.path().join("frontend");
        fs::create_dir_all(frontend_dir.join("node_modules").join(".bin"))
            .expect("frontend bin dir should exist");
        fs::write(
            frontend_dir.join("package.json"),
            r#"{
  "scripts": {
    "dev": "vite --host 0.0.0.0"
  }
}"#,
        )
        .expect("frontend package.json should exist");

        let issues =
            collect_preview_dependency_issues(project_dir.path(), "npm run dev").expect("issues");

        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("vite"));
    }

    #[test]
    fn collect_preview_dependency_issues_passes_when_local_bins_exist() {
        let project_dir = tempfile::tempdir().expect("project tempdir should exist");
        fs::write(
            project_dir.path().join("package.json"),
            r#"{
  "scripts": {
    "dev": "concurrently -n api,web \"npm:dev:api\" \"npm --prefix frontend run dev\"",
    "dev:api": "node server.js"
  }
}"#,
        )
        .expect("root package.json should exist");
        fs::create_dir_all(project_dir.path().join("node_modules").join(".bin"))
            .expect("root bin dir should exist");
        fs::write(
            project_dir
                .path()
                .join("node_modules")
                .join(".bin")
                .join("concurrently.cmd"),
            "@echo off",
        )
        .expect("root binary should exist");

        let frontend_dir = project_dir.path().join("frontend");
        fs::create_dir_all(frontend_dir.join("node_modules").join(".bin"))
            .expect("frontend bin dir should exist");
        fs::write(
            frontend_dir.join("package.json"),
            r#"{
  "scripts": {
    "dev": "vite --host 0.0.0.0"
  }
}"#,
        )
        .expect("frontend package.json should exist");
        fs::write(
            frontend_dir
                .join("node_modules")
                .join(".bin")
                .join("vite.cmd"),
            "@echo off",
        )
        .expect("frontend binary should exist");

        let issues =
            collect_preview_dependency_issues(project_dir.path(), "npm run dev").expect("issues");

        assert!(issues.is_empty());
    }
}
