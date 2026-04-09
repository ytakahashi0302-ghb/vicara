use super::{CliRunner, CliType};

pub const DEFAULT_MODEL: &str = "o3";
pub const INSTALL_HINT: &str = "npm install -g @openai/codex";

#[derive(Debug, Clone, Copy, Default)]
pub struct CodexRunner;

impl CliRunner for CodexRunner {
    fn cli_type(&self) -> CliType {
        CliType::Codex
    }

    fn command_name(&self) -> &str {
        "codex"
    }

    fn default_model(&self) -> &str {
        DEFAULT_MODEL
    }

    fn install_hint(&self) -> &str {
        INSTALL_HINT
    }

    fn build_args(&self, prompt: &str, model: &str, _cwd: &str) -> Vec<String> {
        vec![
            "--full-auto".to_string(),
            "--model".to_string(),
            model.to_string(),
            prompt.to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{CodexRunner, DEFAULT_MODEL, INSTALL_HINT};
    use crate::cli_runner::{CliRunner, CliType};

    #[test]
    fn builds_expected_codex_arguments() {
        let runner = CodexRunner;
        let args = runner.build_args("prompt", "o3-mini", "C:/repo");

        assert_eq!(runner.cli_type(), CliType::Codex);
        assert_eq!(
            args,
            vec!["--full-auto", "--model", "o3-mini", "prompt"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(runner.default_model(), DEFAULT_MODEL);
        assert_eq!(runner.install_hint(), INSTALL_HINT);
    }
}
