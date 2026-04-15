use super::{CliRunner, CliType};
use std::path::{Path, PathBuf};

pub const DEFAULT_MODEL: &str = "claude-haiku-4-5";
pub const INSTALL_HINT: &str = "npm install -g @anthropic-ai/claude-code";

#[derive(Debug, Clone, Copy, Default)]
pub struct ClaudeRunner;

impl CliRunner for ClaudeRunner {
    fn cli_type(&self) -> CliType {
        CliType::Claude
    }

    fn command_name(&self) -> &str {
        "claude"
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    fn install_hint(&self) -> &str {
        INSTALL_HINT
    }

    fn build_args(&self, prompt: &str, model: &str, cwd: &str) -> Vec<String> {
        vec![
            "-p".to_string(),
            prompt.to_string(),
            "--model".to_string(),
            model.to_string(),
            "--permission-mode".to_string(),
            "bypassPermissions".to_string(),
            "--add-dir".to_string(),
            cwd.to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--include-partial-messages".to_string(),
            "--verbose".to_string(),
        ]
    }

    fn prepare_invocation(
        &self,
        command_path: &Path,
        args: Vec<String>,
    ) -> Result<(PathBuf, Vec<String>), String> {
        #[cfg(windows)]
        {
            if let Some((node_path, mut prefix_args)) = resolve_windows_npm_shim(command_path)? {
                prefix_args.extend(args);
                return Ok((node_path, prefix_args));
            }
        }

        Ok((command_path.to_path_buf(), args))
    }
}

#[cfg(windows)]
fn resolve_windows_npm_shim(command_path: &Path) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    super::resolve_windows_npm_cli_invocation(
        command_path,
        "claude",
        &["node_modules", "@anthropic-ai", "claude-code", "cli.js"],
        &[],
    )
}

#[cfg(test)]
mod tests {
    use super::{ClaudeRunner, DEFAULT_MODEL, INSTALL_HINT};
    use crate::cli_runner::{CliRunner, CliType};

    #[test]
    fn builds_expected_claude_arguments() {
        let runner = ClaudeRunner;
        let args = runner.build_args("prompt", "claude-3-5-sonnet-20241022", "C:/repo");

        assert_eq!(runner.cli_type(), CliType::Claude);
        assert_eq!(
            args,
            vec![
                "-p",
                "prompt",
                "--model",
                "claude-3-5-sonnet-20241022",
                "--permission-mode",
                "bypassPermissions",
                "--add-dir",
                "C:/repo",
                "--output-format",
                "stream-json",
                "--include-partial-messages",
                "--verbose",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        assert_eq!(runner.default_model(), DEFAULT_MODEL);
        assert_eq!(runner.install_hint(), INSTALL_HINT);
    }

    #[cfg(windows)]
    #[test]
    fn prepare_invocation_rewrites_npm_shim_to_node_bundle() {
        let temp = tempfile::tempdir().expect("tempdir should exist");
        let npm_dir = temp.path();
        let package_dir = npm_dir
            .join("node_modules")
            .join("@anthropic-ai")
            .join("claude-code");
        std::fs::create_dir_all(&package_dir).expect("package dir should exist");
        std::fs::write(package_dir.join("cli.js"), "console.log('ok');")
            .expect("cli entry should exist");

        let command_path = npm_dir.join("claude.cmd");
        std::fs::write(&command_path, "@echo off").expect("cmd shim should exist");

        let runner = ClaudeRunner;
        let (resolved_command, resolved_args) = runner
            .prepare_invocation(&command_path, vec!["-p".into(), "prompt".into()])
            .expect("invocation should be prepared");

        assert!(resolved_command
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.eq_ignore_ascii_case("node") || name.eq_ignore_ascii_case("node.exe"))
            .unwrap_or(false));
        assert!(resolved_args[0].ends_with("cli.js"));
        assert_eq!(resolved_args[1], "-p");
        assert_eq!(resolved_args[2], "prompt");
    }
}
