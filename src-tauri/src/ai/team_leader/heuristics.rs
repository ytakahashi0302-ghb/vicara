use super::super::common::{get_project_backlog_counts, ChatTaskResponse, ProjectBacklogCounts};
use tauri::{AppHandle, Emitter};

fn looks_like_backlog_mutation_request(message: &str) -> bool {
    let normalized = message.to_lowercase();
    let has_action = [
        "追加", "作成", "登録", "生成", "append", "create", "add", "register",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));
    let has_target = [
        "バックログ",
        "ストーリー",
        "story",
        "stories",
        "タスク",
        "task",
        "tasks",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));

    has_action && has_target
}

pub(super) fn looks_like_generic_backlog_creation_request(message: &str) -> bool {
    let normalized = message.to_lowercase();
    let mentions_story_scope = ["バックログ", "ストーリー", "story", "stories", "backlog"]
        .iter()
        .any(|keyword| normalized.contains(keyword));
    let mentions_existing_target = [
        "既存",
        "このストーリー",
        "そのストーリー",
        "story id",
        "target_story_id",
        "id:",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));
    let asks_task_only = normalized.contains("タスク")
        && !normalized.contains("バックログ")
        && !normalized.contains("ストーリー")
        && !normalized.contains("story");

    looks_like_backlog_mutation_request(message)
        && mentions_story_scope
        && !mentions_existing_target
        && !asks_task_only
}

pub(super) fn looks_like_mutation_request(message: &str) -> bool {
    looks_like_backlog_mutation_request(message)
}

pub(super) fn has_product_context_document(context_md: &str) -> bool {
    context_md.contains("--- PRODUCT_CONTEXT.md ---")
}

pub(super) fn build_missing_product_context_reply() -> String {
    "PRODUCT_CONTEXT.md を含むプロジェクト文脈を取得できないため、コンテキスト起点のバックログ生成は実行できません。プロジェクトの Local Path 設定と対象フォルダを確認してください。".to_string()
}

fn backlog_counts_changed(before: ProjectBacklogCounts, after: ProjectBacklogCounts) -> bool {
    before.stories != after.stories
        || before.tasks != after.tasks
        || before.dependencies != after.dependencies
}

async fn get_changed_backlog_counts_with_retry(
    app: &AppHandle,
    project_id: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ProjectBacklogCounts>, String> {
    let after_counts = get_project_backlog_counts(app, project_id).await?;
    if backlog_counts_changed(before_counts, after_counts) {
        return Ok(Some(after_counts));
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let retry_counts = get_project_backlog_counts(app, project_id).await?;
    if backlog_counts_changed(before_counts, retry_counts) {
        return Ok(Some(retry_counts));
    }

    Ok(None)
}

pub(super) async fn detect_backlog_change_with_retry(
    app: &AppHandle,
    project_id: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<bool, String> {
    Ok(
        get_changed_backlog_counts_with_retry(app, project_id, before_counts)
            .await?
            .is_some(),
    )
}

pub(super) fn build_backlog_counts_reply(
    reply_prefix: String,
    before_counts: ProjectBacklogCounts,
    after_counts: ProjectBacklogCounts,
) -> Option<ChatTaskResponse> {
    let added_stories = after_counts.stories.saturating_sub(before_counts.stories);
    let added_tasks = after_counts.tasks.saturating_sub(before_counts.tasks);
    let added_dependencies = after_counts
        .dependencies
        .saturating_sub(before_counts.dependencies);

    if added_stories == 0 && added_tasks == 0 && added_dependencies == 0 {
        return None;
    }

    Some(ChatTaskResponse {
        reply: format!(
            "{}\n\n追加結果: stories +{}, tasks +{}, dependencies +{}",
            reply_prefix, added_stories, added_tasks, added_dependencies
        ),
    })
}

pub(super) async fn build_partial_team_leader_success_response(
    app: &AppHandle,
    project_id: &str,
    before_counts: ProjectBacklogCounts,
    provider_error: &str,
) -> Result<Option<ChatTaskResponse>, String> {
    let Some(after_counts) =
        get_changed_backlog_counts_with_retry(app, project_id, before_counts).await?
    else {
        return Ok(None);
    };

    let _ = app.emit("kanban-updated", ());
    let error_summary = summarize_provider_error(provider_error);

    Ok(build_backlog_counts_reply(
        format!(
            "バックログ更新は反映されましたが、最終のAI応答生成で一時的なエラーが発生しました。内容確認中に再送せず、そのまま追加結果を返します。\n原因: {}",
            error_summary
        ),
        before_counts,
        after_counts,
    ))
}

fn summarize_provider_error(provider_error: &str) -> &str {
    provider_error
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("LLM provider error")
}

pub(super) fn is_transient_provider_unavailable(provider_error: &str) -> bool {
    let normalized = provider_error.to_ascii_lowercase();
    normalized.contains("503")
        && (normalized.contains("service unavailable")
            || normalized.contains("\"status\": \"unavailable\"")
            || normalized.contains("high demand")
            || normalized.contains("status\": \"unavailable\"")
            || normalized.contains("unavailable"))
}

pub(super) fn build_team_leader_provider_unavailable_reply(
    provider_error: &str,
    mutation_requested: bool,
) -> ChatTaskResponse {
    let error_summary = summarize_provider_error(provider_error);
    let reply = if mutation_requested {
        format!(
            "AI プロバイダーが一時的に高負荷のため、今回はバックログを作成していません。少し待って再試行するか、CLI もしくは別プロバイダーへ切り替えてください。\n原因: {}",
            error_summary
        )
    } else {
        format!(
            "AI プロバイダーが一時的に高負荷のため、今回は応答を返せませんでした。少し待って再試行するか、CLI もしくは別プロバイダーへ切り替えてください。\n原因: {}",
            error_summary
        )
    };

    ChatTaskResponse { reply }
}
