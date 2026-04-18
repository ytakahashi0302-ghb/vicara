use tauri::AppHandle;

use super::super::super::common::{
    execute_po_cli_prompt, parse_json_response, record_cli_usage, record_provider_usage,
    ChatTaskResponse, Message, PoAssistantExecutionPlan, ProjectBacklogCounts,
};

fn parse_team_leader_execution_plan(content: &str) -> Result<PoAssistantExecutionPlan, String> {
    parse_json_response::<PoAssistantExecutionPlan>(content)
}

pub(crate) async fn chat_team_leader_with_tools_with_retry(
    app: &AppHandle,
    provider: &crate::rig_provider::AiProvider,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_input: &str,
    prior_messages: &[Message],
    project_id: &str,
) -> Result<crate::rig_provider::LlmTextResponse, String> {
    let chat_history = crate::rig_provider::convert_messages(prior_messages);
    crate::rig_provider::chat_team_leader_with_tools(
        app,
        provider,
        api_key,
        model,
        system_prompt,
        user_input,
        chat_history,
        project_id,
    )
    .await
}

pub(crate) async fn execute_fallback_team_leader_plan(
    app: &AppHandle,
    provider: &crate::rig_provider::AiProvider,
    api_key: &str,
    model: &str,
    project_id: &str,
    context_md: &str,
    user_request: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ChatTaskResponse>, String> {
    let fallback_system_prompt =
        if super::super::heuristics::looks_like_generic_backlog_creation_request(user_request) {
            super::super::prompts::build_contextual_backlog_generation_system_prompt(context_md)
        } else {
            let quality_gates = super::super::prompts::build_po_assistant_quality_gates();
            format!(
                "あなたはバックログ登録計画を JSON で返すプランナーです。ユーザー依頼に対して、実行すべき `create_story_and_tasks` 相当の操作を JSON のみで返してください。\n\nルール:\n- 既存ストーリーにタスクを追加する場合は、必ず context 内に存在する story ID を `target_story_id` に設定する\n- 新規ストーリーを作る場合のみ `target_story_id` を null にし、`story_title` を必須で入れる\n- `tasks` は必ず1件以上含める\n- 各 task には `title`, `description`, `priority`, `blocked_by_indices` を入れる\n- priority は整数 1〜5\n- 実行不要なら `operations` は空配列にする\n- 出力は必ず JSON オブジェクトのみ\n\n{}\n\n返却形式:\n{{\"reply\":\"ユーザー向け要約\",\"operations\":[{{\"target_story_id\":null,\"story_title\":\"...\",\"story_description\":\"...\",\"acceptance_criteria\":\"...\",\"story_priority\":3,\"tasks\":[{{\"title\":\"...\",\"description\":\"...\",\"priority\":2,\"blocked_by_indices\":[0]}}]}}]}}\n\n【既存バックログ】\n{}",
                quality_gates,
                context_md
            )
        };

    let raw_plan = crate::rig_provider::chat_with_history(
        provider,
        api_key,
        model,
        &fallback_system_prompt,
        user_request,
        vec![],
    )
    .await?;
    record_provider_usage(app, project_id, "team_leader", &raw_plan).await;

    let plan = match parse_team_leader_execution_plan(&raw_plan.content) {
        Ok(plan) => plan,
        Err(_) => return Ok(None),
    };

    super::apply::apply_team_leader_execution_plan(app, project_id, plan, before_counts).await
}

pub(crate) async fn execute_contextual_cli_backlog_plan(
    app: &AppHandle,
    project_id: &str,
    cli_type: crate::cli_runner::CliType,
    model: &str,
    cwd: &str,
    context_md: &str,
    user_request: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ChatTaskResponse>, String> {
    let cli_prompt = format!(
        "{}\n\n【今回のユーザー依頼】\n{}",
        super::super::prompts::build_contextual_backlog_generation_system_prompt(context_md),
        user_request
    );
    let result =
        execute_po_cli_prompt::<PoAssistantExecutionPlan>(&cli_type, model, &cli_prompt, cwd)
            .await?;
    record_cli_usage(app, project_id, "team_leader", &cli_type, &result.metadata).await;

    super::apply::apply_team_leader_execution_plan(app, project_id, result.value, before_counts)
        .await
}
