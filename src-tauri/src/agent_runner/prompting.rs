use crate::{cli_runner::CliRunner, db};
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

pub(super) fn cleanup_temp_file(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            log::warn!(
                "failed to remove temporary agent prompt file {}: {}",
                path.display(),
                error
            );
        }
    }

    if let Some(parent) = path.parent() {
        let is_vicara_temp_dir = parent
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == ".vicara-agent")
            .unwrap_or(false);

        if is_vicara_temp_dir {
            let _ = fs::remove_dir(parent);
        }
    }
}

fn sanitize_for_filename(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "task".to_string()
    } else {
        sanitized
    }
}

pub(super) fn build_task_prompt(
    task: &db::Task,
    role: &db::TeamRole,
    additional_context: Option<&str>,
) -> String {
    let description = task.description.as_deref().unwrap_or("特になし");
    let extra_context = additional_context
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("\n# 追加コンテキスト\n{}\n", value))
        .unwrap_or_default();

    format!(
        "あなたは {} です。\n{}\n\n# タスク名\n{}\n\n# 詳細\n{}\n{}# 作業指示\n- タスクのゴール達成に必要な実装・修正を行ってください。\n- 追加コンテキストにレビュー指摘、確認結果、再実行理由が含まれる場合は、それを優先度の高い不足分として扱い、解消を最優先してください。\n- 既存の挙動やUIを不必要に変えず、関係のない変更は加えないでください。\n- 既存の変更やユーザー作業をむやみに巻き戻さないでください。\n\n# 完了条件\n- タスク名・詳細・追加コンテキストに対して必要な変更が作業ツリーに反映されていること。\n- 変更対象外の機能を壊していないこと。\n- 実装だけでなく、必要なテスト・検証観点まで踏まえて完了判断していること。\n\n# 自己検証\n- 変更したファイルと内容がタスクのゴールに直結しているか見直してください。\n- 追加コンテキストにレビュー指摘や期待との差分がある場合は、それが解消されたかを確認してください。\n- 可能な範囲でテスト、ビルド、静的確認を行い、未実施なら理由を最終報告に残してください。\n- 不明点や制約が残る場合は、成功を断定せずに懸念として明記してください。\n\n# 終了時の報告\n- 変更概要、実施した検証、残る懸念を簡潔にまとめてから終了してください。\n",
        role.name.trim(),
        role.system_prompt.trim(),
        task.title.trim(),
        description.trim(),
        extra_context
    )
}

fn create_agent_temp_file_path(
    task_id: &str,
    cwd: &Path,
    prefix: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    let timestamp = super::lifecycle::current_timestamp_millis()?;
    let prompt_dir = cwd.join(".vicara-agent");

    let file_name = format!(
        "vicara-agent-{}-{}-{}.{}",
        prefix,
        sanitize_for_filename(task_id),
        timestamp,
        extension
    );
    fs::create_dir_all(&prompt_dir).map_err(|e| {
        format!(
            "CLI 実行用の一時ディレクトリ作成に失敗しました ({}): {}",
            prompt_dir.display(),
            e
        )
    })?;
    Ok(prompt_dir.join(file_name))
}

pub(super) fn create_prompt_file(
    task_id: &str,
    prompt: &str,
    cwd: &Path,
) -> Result<PathBuf, String> {
    let path = create_agent_temp_file_path(task_id, cwd, "prompt", "md")?;

    fs::write(&path, prompt).map_err(|e| {
        format!(
            "CLI 実行用の一時ファイル作成に失敗しました ({}): {}",
            path.display(),
            e
        )
    })?;

    Ok(path)
}

pub(super) fn build_cli_prompt_from_file(prompt_file_path: &Path) -> String {
    format!(
        "以下のファイルに記載された役割とタスク指示を読み込み、それに従って開発を実行してください。ファイルパス: {}",
        prompt_file_path.display()
    )
}

pub(super) struct PreparedCliInvocation {
    pub(super) command_path: PathBuf,
    pub(super) args: Vec<String>,
    pub(super) stdin_payload: Option<String>,
    pub(super) response_capture_path: Option<PathBuf>,
}

pub(super) fn prepare_cli_invocation(
    runner: &dyn CliRunner,
    cli_command_path: &Path,
    task_id: &str,
    prompt: &str,
    model: &str,
    cwd: &str,
) -> Result<PreparedCliInvocation, String> {
    let mut base_args = runner.build_args(prompt, model, cwd);
    let response_capture_path = if runner.prefers_response_capture_file() {
        let path = create_agent_temp_file_path(task_id, Path::new(cwd), "response", "txt")?;
        runner.prepare_response_capture(&mut base_args, &path)?;
        Some(path)
    } else {
        None
    };
    let (command_path, args) = runner.prepare_invocation(cli_command_path, base_args)?;

    Ok(PreparedCliInvocation {
        command_path,
        args,
        stdin_payload: runner.stdin_payload(prompt),
        response_capture_path,
    })
}

pub(super) fn read_response_capture_file(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let normalized = contents.replace("\r\n", "\n").trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(super) fn spawn_stdin_payload_writer<W>(
    mut writer: W,
    payload: String,
    cli_name: String,
    task_id: String,
) where
    W: IoWrite + Send + 'static,
{
    std::thread::spawn(move || {
        if let Err(error) = writer.write_all(payload.as_bytes()) {
            log::warn!(
                "failed to write stdin payload for {} task {}: {}",
                cli_name,
                task_id,
                error
            );
            return;
        }

        if let Err(error) = writer.flush() {
            log::warn!(
                "failed to flush stdin payload for {} task {}: {}",
                cli_name,
                task_id,
                error
            );
        }
    });
}
