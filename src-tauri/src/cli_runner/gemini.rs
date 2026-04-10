use super::{CliRunner, CliType};

pub const DEFAULT_MODEL: &str = "gemini-2.5-pro";
pub const INSTALL_HINT: &str = "npm install -g @google/gemini-cli";
const HEADLESS_PROMPT_SUFFIX: &str = "上記の指示に従い、指定形式で回答してください。";

#[derive(Debug, Clone, Copy, Default)]
pub struct GeminiRunner;

impl CliRunner for GeminiRunner {
    fn cli_type(&self) -> CliType {
        CliType::Gemini
    }

    fn command_name(&self) -> &str {
        "gemini"
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    fn install_hint(&self) -> &str {
        INSTALL_HINT
    }

    fn build_args(&self, _prompt: &str, model: &str, _cwd: &str) -> Vec<String> {
        vec![
            "--model".to_string(),
            model.to_string(),
            "--approval-mode".to_string(),
            "yolo".to_string(),
            "--prompt".to_string(),
            HEADLESS_PROMPT_SUFFIX.to_string(),
        ]
    }

    fn stdin_payload(&self, prompt: &str) -> Option<String> {
        Some(prompt.to_string())
    }

    fn timeout_secs(&self) -> u64 {
        180
    }

    fn env_vars(&self) -> Vec<(String, String)> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::{GeminiRunner, DEFAULT_MODEL, HEADLESS_PROMPT_SUFFIX, INSTALL_HINT};
    use crate::cli_runner::{CliRunner, CliType};

    #[test]
    fn builds_expected_gemini_arguments() {
        let runner = GeminiRunner;
        let args = runner.build_args("prompt", "gemini-2.5-flash", "C:/repo");

        assert_eq!(runner.cli_type(), CliType::Gemini);
        assert_eq!(
            args,
            vec![
                "--model",
                "gemini-2.5-flash",
                "--approval-mode",
                "yolo",
                "--prompt",
                HEADLESS_PROMPT_SUFFIX,
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        assert_eq!(runner.default_model(), DEFAULT_MODEL);
        assert_eq!(runner.install_hint(), INSTALL_HINT);
        assert!(runner.env_vars().is_empty());
        assert_eq!(runner.stdin_payload("prompt").as_deref(), Some("prompt"));
        assert_eq!(runner.timeout_secs(), 180);
    }
}
