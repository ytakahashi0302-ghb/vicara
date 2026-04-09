use serde::{Deserialize, Serialize};

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
    fn create_runner_returns_claude_runner() {
        let runner = create_runner(&CliType::Claude).expect("Claude runner should exist");

        assert_eq!(runner.cli_type(), CliType::Claude);
        assert_eq!(runner.command_name(), "claude");
        assert_eq!(runner.display_name(), "Claude Code CLI");
    }

    #[test]
    fn create_runner_returns_all_supported_runners() {
        let cases = [
            (CliType::Claude, "claude", "claude-sonnet-4-20250514"),
            (CliType::Gemini, "gemini", "gemini-2.5-pro"),
            (CliType::Codex, "codex", "o3"),
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

        assert_eq!(runner.resolve_model(""), "gemini-2.5-pro");
        assert_eq!(runner.resolve_model("  custom-model  "), "custom-model");
    }
}
