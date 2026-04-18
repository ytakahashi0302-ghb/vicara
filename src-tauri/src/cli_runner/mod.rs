use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod claude;
pub mod codex;
pub mod gemini;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliType {
    Claude,
    Gemini,
    Codex,
}

impl CliType {
    pub fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "gemini" => Self::Gemini,
            "codex" => Self::Codex,
            _ => Self::Claude,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::Codex => "codex",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Claude => "Claude Code CLI",
            Self::Gemini => "Gemini CLI",
            Self::Codex => "Codex CLI",
        }
    }
}

pub trait CliRunner: Send + Sync {
    fn cli_type(&self) -> CliType;

    fn command_name(&self) -> &str;

    fn default_model(&self) -> &str;

    fn install_hint(&self) -> &str;

    fn display_name(&self) -> &str {
        self.cli_type().display_name()
    }

    fn build_args(&self, prompt: &str, model: &str, cwd: &str) -> Vec<String>;

    fn prepare_response_capture(
        &self,
        _args: &mut Vec<String>,
        _capture_path: &Path,
    ) -> Result<(), String> {
        Ok(())
    }

    fn prefers_response_capture_file(&self) -> bool {
        false
    }

    fn prepare_invocation(
        &self,
        command_path: &Path,
        args: Vec<String>,
    ) -> Result<(PathBuf, Vec<String>), String> {
        Ok((command_path.to_path_buf(), args))
    }

    fn stdin_payload(&self, _prompt: &str) -> Option<String> {
        None
    }

    fn timeout_secs(&self) -> u64 {
        60
    }

    fn resolve_model(&self, configured_model: &str) -> String {
        let trimmed = configured_model.trim();

        if trimmed.is_empty() {
            self.default_model().to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn env_vars(&self) -> Vec<(String, String)> {
        vec![]
    }

    #[allow(dead_code)]
    fn parse_version(&self, stdout: &[u8], stderr: &[u8]) -> Option<String> {
        [stdout, stderr]
            .into_iter()
            .map(|bytes| String::from_utf8_lossy(bytes).trim().to_string())
            .find_map(|text| {
                text.lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .map(str::to_string)
            })
    }
}

#[cfg(windows)]
pub(crate) fn resolve_windows_npm_cli_invocation(
    command_path: &Path,
    file_name_prefix: &str,
    script_relative_segments: &[&str],
    prefix_args: &[&str],
) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    let Some(file_name) = command_path.file_name().and_then(|name| name.to_str()) else {
        return Ok(None);
    };
    if !file_name.to_ascii_lowercase().starts_with(file_name_prefix) {
        return Ok(None);
    }

    let Some(base_dir) = command_path.parent() else {
        return Ok(None);
    };

    let script_path = script_relative_segments
        .iter()
        .fold(base_dir.to_path_buf(), |path, segment| path.join(segment));
    if !script_path.is_file() {
        return Ok(None);
    }

    let local_node = base_dir.join("node.exe");
    let node_path = if local_node.is_file() {
        local_node
    } else {
        crate::cli_detection::resolve_cli_command_path("node")
            .unwrap_or_else(|| PathBuf::from("node"))
    };

    let mut args = prefix_args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    args.push(script_path.to_string_lossy().to_string());

    Ok(Some((node_path, args)))
}

pub fn create_runner(cli_type: &CliType) -> Result<Box<dyn CliRunner>, String> {
    match cli_type {
        CliType::Claude => Ok(Box::new(claude::ClaudeRunner)),
        CliType::Gemini => Ok(Box::new(gemini::GeminiRunner)),
        CliType::Codex => Ok(Box::new(codex::CodexRunner)),
    }
}

#[cfg(test)]
mod tests {
    use super::{create_runner, CliType};

    #[test]
    fn cli_type_defaults_to_claude_for_unknown_values() {
        assert_eq!(CliType::from_str("unknown"), CliType::Claude);
    }

    #[test]
    fn create_runner_returns_expected_runner_for_claude() {
        let runner = create_runner(&CliType::Claude).expect("Claude runner should exist");

        assert_eq!(runner.cli_type(), CliType::Claude);
        assert_eq!(runner.command_name(), "claude");
        assert_eq!(runner.display_name(), "Claude Code CLI");
    }

    #[test]
    fn create_runner_returns_all_supported_runners() {
        let cases = [
            (CliType::Claude, "claude", "claude-haiku-4-5"),
            (CliType::Gemini, "gemini", "gemini-3-flash-preview"),
            (CliType::Codex, "codex", "gpt-5.4-mini"),
        ];

        for (cli_type, expected_command, expected_model) in cases {
            let runner = create_runner(&cli_type).expect("Runner should exist");

            assert_eq!(runner.cli_type(), cli_type);
            assert_eq!(runner.command_name(), expected_command);
            assert_eq!(runner.default_model(), expected_model);
        }
    }

    #[test]
    fn resolve_model_falls_back_to_runner_default() {
        let runner = create_runner(&CliType::Gemini).expect("Gemini runner should exist");

        assert_eq!(runner.resolve_model(""), "gemini-3-flash-preview");
        assert_eq!(runner.resolve_model("  custom-model  "), "custom-model");
    }
}
