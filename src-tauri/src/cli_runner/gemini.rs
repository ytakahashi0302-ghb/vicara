use super::{CliRunner, CliType};

pub const DEFAULT_MODEL: &str = "gemini-2.5-pro";
pub const INSTALL_HINT: &str = "npm install -g @google/gemini-cli";

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

    fn build_args(&self, prompt: &str, model: &str, _cwd: &str) -> Vec<String> {
        vec![
            "-p".to_string(),
            prompt.to_string(),
            "--model".to_string(),
            model.to_string(),
            "--sandbox".to_string(),
            "permissive".to_string(),
        ]
    }

    fn env_vars(&self) -> Vec<(String, String)> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::{GeminiRunner, DEFAULT_MODEL, INSTALL_HINT};
    use crate::cli_runner::{CliRunner, CliType};

    #[test]
    fn builds_expected_gemini_arguments() {
        let runner = GeminiRunner;
        let args = runner.build_args("prompt", "gemini-2.5-flash", "C:/repo");

        assert_eq!(runner.cli_type(), CliType::Gemini);
        assert_eq!(
            args,
            vec![
                "-p",
                "prompt",
                "--model",
                "gemini-2.5-flash",
                "--sandbox",
                "permissive",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
        assert_eq!(runner.default_model(), DEFAULT_MODEL);
        assert_eq!(runner.install_hint(), INSTALL_HINT);
        assert!(runner.env_vars().is_empty());
    }
}
