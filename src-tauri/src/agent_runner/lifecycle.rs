use crate::{
    agent_retro, cli_detection, cli_runner::CliRunner, db, git, llm_observability,
    node_dependencies, worktree,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

use super::{
    ActiveAgentSession, AgentExitPayload, AgentOutputPayload, AgentSession, AgentSessionEntry,
    AgentStdoutParser, AgentUsageContext, PassthroughAgentStdoutParser, RecentOutputChunk,
    AGENT_CLI_OUTPUT_EVENT, AGENT_CLI_STARTED_EVENT,
};

pub(super) fn current_timestamp_millis() -> Result<i64, String> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis() as i64)
}

pub(super) fn create_stdout_parser(_runner: &dyn CliRunner) -> Box<dyn AgentStdoutParser + Send> {
    Box::new(PassthroughAgentStdoutParser)
}

pub(super) fn emit_agent_output(app_handle: &AppHandle, task_id: &str, output: String) {
    if output.is_empty() {
        return;
    }

    let _ = app_handle.emit(
        AGENT_CLI_OUTPUT_EVENT,
        AgentOutputPayload {
            task_id: task_id.to_string(),
            output,
        },
    );
}

pub(super) fn emit_parsed_stdout_chunks(
    app_handle: &AppHandle,
    task_id: &str,
    parser: &mut dyn AgentStdoutParser,
    chunk: &str,
) {
    for output in parser.consume(chunk) {
        emit_agent_output(app_handle, task_id, output);
    }
}

pub(super) fn flush_stdout_parser(
    app_handle: &AppHandle,
    task_id: &str,
    parser: &mut dyn AgentStdoutParser,
) {
    for output in parser.finish() {
        emit_agent_output(app_handle, task_id, output);
    }
}

pub(super) fn build_cli_not_found_message(runner: &dyn CliRunner) -> String {
    format!(
        "{} が見つかりません。インストール後に Settings から再確認してください。",
        runner.display_name()
    )
}

fn normalize_output_chunk_for_dedup(output: &str) -> Option<String> {
    let normalized = output.replace("\r\n", "\n").trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(super) fn preview_output_chunk_for_log(output: &str) -> String {
    let normalized = output.replace('\r', "\\r").replace('\n', "\\n");
    let preview: String = normalized.chars().take(160).collect();
    if normalized.chars().count() > 160 {
        format!("{preview}…")
    } else {
        preview
    }
}

pub(super) fn should_suppress_duplicate_output_at(
    recent_output: &mut Option<RecentOutputChunk>,
    output: &str,
    now: Instant,
) -> bool {
    let Some(normalized) = normalize_output_chunk_for_dedup(output) else {
        return false;
    };

    if let Some(previous) = recent_output.as_ref() {
        if previous.normalized == normalized
            && now.duration_since(previous.emitted_at) <= std::time::Duration::from_millis(750)
        {
            return true;
        }
    }

    *recent_output = Some(RecentOutputChunk {
        normalized,
        emitted_at: now,
    });
    false
}

pub(super) fn should_suppress_duplicate_output(
    recent_output: &Arc<Mutex<Option<RecentOutputChunk>>>,
    output: &str,
) -> bool {
    let Ok(mut guard) = recent_output.lock() else {
        return false;
    };

    should_suppress_duplicate_output_at(&mut guard, output, Instant::now())
}

pub(super) fn resolve_cli_command_path(runner: &dyn CliRunner) -> Result<PathBuf, String> {
    cli_detection::resolve_cli_command_path(runner.command_name())
        .ok_or_else(|| build_cli_not_found_message(runner))
}

pub(super) fn promote_session_to_running(
    app_handle: &AppHandle,
    sessions_arc: &Arc<Mutex<HashMap<String, AgentSessionEntry>>>,
    task_id: &str,
    session: AgentSession,
) -> Result<(), String> {
    let started_payload = session.info.clone();

    let mut sessions = sessions_arc.lock().map_err(|e| e.to_string())?;
    sessions.insert(task_id.to_string(), AgentSessionEntry::Running(session));
    drop(sessions);

    if let Err(error) = app_handle.emit(AGENT_CLI_STARTED_EVENT, started_payload) {
        log::warn!(
            "failed to emit {} for {}: {}",
            AGENT_CLI_STARTED_EVENT,
            task_id,
            error
        );
    }

    Ok(())
}

pub(super) fn is_meta_output_file(path: &str) -> bool {
    let normalized = path
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase();

    matches!(
        normalized.as_str(),
        "walkthrough.md" | "handoff.md" | "task.md" | "implementation_plan.md"
    )
}

struct WorktreeChangeSet {
    worktree_path: PathBuf,
    substantive_files: Vec<String>,
}

async fn load_worktree_change_set(
    app_handle: &AppHandle,
    task_id: &str,
) -> Result<Option<WorktreeChangeSet>, String> {
    let Some(record) = db::get_worktree_by_task_id(app_handle, task_id).await? else {
        return Ok(None);
    };

    let worktree_path = PathBuf::from(record.worktree_path);
    if !worktree_path.exists() {
        return Ok(None);
    }

    let changed_files = git::list_changed_files_in_worktree(&worktree_path)?;
    let substantive_files = changed_files
        .into_iter()
        .filter(|path| !is_meta_output_file(path))
        .collect::<Vec<_>>();

    Ok(Some(WorktreeChangeSet {
        worktree_path,
        substantive_files,
    }))
}

async fn list_substantive_worktree_changes(
    app_handle: &AppHandle,
    task_id: &str,
) -> Result<Option<Vec<String>>, String> {
    Ok(load_worktree_change_set(app_handle, task_id)
        .await?
        .map(|change_set| change_set.substantive_files))
}

async fn sync_dependencies_after_manifest_changes(
    app_handle: &AppHandle,
    task_id: &str,
    worktree_path: &Path,
    substantive_files: &[String],
) -> Result<Option<Vec<String>>, String> {
    if !node_dependencies::has_node_manifest_changes(substantive_files) {
        return Ok(None);
    }
    let changed_manifests = node_dependencies::changed_node_manifest_paths(substantive_files);

    emit_agent_output(
        app_handle,
        task_id,
        format!(
            "\x1b[36mpackage manifest 変更を検知したため、依存関係を再同期します: {}\x1b[0m\r\n",
            changed_manifests.join(", ")
        ),
    );

    worktree::ensure_worktree_node_modules_links(worktree_path).map_err(|error| {
        format!(
            "共有 node_modules の再接続に失敗しました ({}): {}",
            worktree_path.display(),
            error
        )
    })?;

    let installed_dirs = node_dependencies::install_node_dependencies(
        app_handle,
        worktree_path,
        node_dependencies::NodeInstallOutputTarget::AgentTask {
            task_id: task_id.to_string(),
        },
        "Dev Agent 完了後",
    )
    .await?;

    if installed_dirs.is_empty() {
        emit_agent_output(
            app_handle,
            task_id,
            "\x1b[33mpackage manifest は変更されましたが、再同期対象の package.json を検出できませんでした。\x1b[0m\r\n"
                .to_string(),
        );
    } else {
        emit_agent_output(
            app_handle,
            task_id,
            format!(
                "\x1b[32m依存関係の再同期が完了しました: {}\x1b[0m\r\n",
                installed_dirs.join(", ")
            ),
        );
    }

    Ok(Some(installed_dirs))
}

pub(super) async fn build_exit_payload(
    app_handle: &AppHandle,
    task_id: &str,
    success: bool,
    reason: String,
) -> AgentExitPayload {
    if !success {
        return AgentExitPayload {
            task_id: task_id.to_string(),
            success,
            reason,
            new_status: None,
        };
    }

    match db::get_task_by_id(app_handle, task_id).await {
        Ok(Some(task)) => {
            let substantive_changes = match load_worktree_change_set(app_handle, task_id).await {
                Ok(changes) => changes,
                Err(error) => {
                    return AgentExitPayload {
                        task_id: task_id.to_string(),
                        success: false,
                        reason: format!(
                            "CLI の処理は完了しましたが、worktree 差分の確認に失敗したためタスクを Review に更新していません: {}",
                            error
                        ),
                        new_status: None,
                    };
                }
            };

            if let Some(change_set) = substantive_changes.as_ref() {
                if change_set.substantive_files.is_empty() {
                    return AgentExitPayload {
                        task_id: task_id.to_string(),
                        success: false,
                        reason: "CLI は完走しましたが、実装対象の差分を確認できませんでした。`walkthrough.md` / `handoff.md` などの補助ファイルのみが更新された可能性があるため、タスクは Review に移動していません。".to_string(),
                        new_status: None,
                    };
                }
            }

            let reason = match substantive_changes.as_ref() {
                Some(change_set) => match sync_dependencies_after_manifest_changes(
                    app_handle,
                    task_id,
                    &change_set.worktree_path,
                    &change_set.substantive_files,
                )
                .await
                {
                    Ok(Some(installed_dirs)) if !installed_dirs.is_empty() => {
                        format!("{} / 依存再同期: {}", reason, installed_dirs.join(", "))
                    }
                    Ok(_) => reason,
                    Err(error) => {
                        return AgentExitPayload {
                            task_id: task_id.to_string(),
                            success: false,
                            reason: format!(
                                "CLI は完走しましたが、package.json / lockfile 変更後の依存再同期に失敗したためタスクを Review に更新していません: {}",
                                error
                            ),
                            new_status: None,
                        };
                    }
                },
                None => reason,
            };

            if task.status == "Review" {
                AgentExitPayload {
                    task_id: task_id.to_string(),
                    success: true,
                    reason: reason.clone(),
                    new_status: Some("Review".to_string()),
                }
            } else {
                match db::update_task_status(
                    app_handle.clone(),
                    task_id.to_string(),
                    "Review".to_string(),
                )
                .await
                {
                    Ok(_) => AgentExitPayload {
                        task_id: task_id.to_string(),
                        success: true,
                        reason,
                        new_status: Some("Review".to_string()),
                    },
                    Err(error) => AgentExitPayload {
                        task_id: task_id.to_string(),
                        success: false,
                        reason: format!(
                            "CLI の処理は完了しましたが、タスクを Review に更新できませんでした: {}",
                            error
                        ),
                        new_status: None,
                    },
                }
            }
        }
        Ok(None) => AgentExitPayload {
            task_id: task_id.to_string(),
            success: true,
            reason,
            new_status: None,
        },
        Err(error) => AgentExitPayload {
            task_id: task_id.to_string(),
            success: false,
            reason: format!(
                "CLI の処理は完了しましたが、タスク状態の確認に失敗しました: {}",
                error
            ),
            new_status: None,
        },
    }
}

pub(super) async fn record_cli_usage_event(
    app_handle: &AppHandle,
    session_info: &ActiveAgentSession,
    usage_context: &AgentUsageContext,
    success: bool,
    reason: String,
) {
    let completed_at = current_timestamp_millis().unwrap_or(session_info.started_at);

    if let Err(error) = llm_observability::record_cli_usage(
        app_handle,
        llm_observability::CliUsageRecordInput {
            project_id: usage_context.project_id.clone(),
            task_id: usage_context.db_task_id.clone(),
            sprint_id: usage_context.sprint_id.clone(),
            source_kind: usage_context.source_kind.clone(),
            cli_type: session_info.cli_type.clone(),
            model: session_info.model.clone(),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            cached_input_tokens: None,
            request_started_at: session_info.started_at,
            request_completed_at: completed_at,
            success,
            error_message: (!success).then_some(reason),
        },
    )
    .await
    {
        log::warn!(
            "Failed to record {} usage for session {}: {}",
            session_info.cli_type,
            session_info.task_id,
            error
        );
    }
}

pub(super) async fn persist_agent_retro_run_for_session(
    app_handle: &AppHandle,
    session_info: &ActiveAgentSession,
    usage_context: &AgentUsageContext,
    retro_capture: &Arc<Mutex<agent_retro::AgentRetroCapture>>,
    response_capture_path: Option<&Path>,
    success: bool,
    reason: String,
) {
    let final_answer_override =
        response_capture_path.and_then(super::prompting::read_response_capture_file);
    let finalized = match retro_capture.lock() {
        Ok(mut guard) => guard.finalize(final_answer_override),
        Err(error) => {
            log::warn!(
                "Failed to lock retro capture for session {}: {}",
                session_info.task_id,
                error
            );
            return;
        }
    };

    let changed_files = if let Some(task_id) = usage_context.db_task_id.as_deref() {
        match list_substantive_worktree_changes(app_handle, task_id).await {
            Ok(Some(files)) => files,
            Ok(None) => Vec::new(),
            Err(error) => {
                log::warn!(
                    "Failed to collect substantive worktree changes for retro log task {}: {}",
                    task_id,
                    error
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    if let Err(error) = agent_retro::persist_agent_retro_run(
        app_handle,
        agent_retro::AgentRetroPersistInput {
            project_id: usage_context.project_id.clone(),
            task_id: usage_context.db_task_id.clone(),
            sprint_id: usage_context.sprint_id.clone(),
            source_kind: usage_context.source_kind.clone(),
            role_name: session_info.role_name.clone(),
            cli_type: session_info.cli_type.clone(),
            model: session_info.model.clone(),
            started_at: session_info.started_at,
            completed_at: current_timestamp_millis().unwrap_or(session_info.started_at),
            success,
            error_message: (!success).then_some(reason),
            changed_files,
            finalized,
        },
    )
    .await
    {
        log::warn!(
            "Failed to persist retro log for session {}: {}",
            session_info.task_id,
            error
        );
    }
}
