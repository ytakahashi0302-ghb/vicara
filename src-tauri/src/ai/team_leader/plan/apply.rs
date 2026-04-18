use tauri::{AppHandle, Emitter};

use super::super::super::common::{
    get_project_backlog_counts, ChatTaskResponse, PoAction, PoAssistantExecutionPlan,
    ProjectBacklogCounts,
};

async fn apply_team_leader_action(
    app: &AppHandle,
    project_id: &str,
    action: PoAction,
) -> Result<Option<String>, String> {
    match action.action.as_str() {
        "create_story" => {
            let args: crate::ai_tools::CreateStoryAndTasksArgs =
                serde_json::from_value(action.payload)
                    .map_err(|e| format!("create_story payload のパースに失敗しました: {}", e))?;
            if args.tasks.is_empty() {
                return Ok(None);
            }
            crate::ai_tools::guard_story_creation_against_duplicates(
                app,
                project_id,
                args.target_story_id.as_deref(),
                args.story_title.as_deref(),
            )
            .await?;
            let story_draft = crate::db::StoryDraftInput {
                target_story_id: args.target_story_id.clone(),
                title: args
                    .story_title
                    .clone()
                    .unwrap_or_else(|| "Untitled Story".to_string()),
                description: args.story_description.clone(),
                acceptance_criteria: args.acceptance_criteria.clone(),
                priority: args.story_priority,
            };
            crate::db::insert_story_with_tasks(app, project_id, story_draft, args.tasks).await?;
            let _ = app.emit("kanban-updated", ());
            Ok(Some("PBI・タスクを登録しました。".to_string()))
        }
        "add_note" => {
            let args: crate::ai_tools::AddProjectNoteArgs = serde_json::from_value(action.payload)
                .map_err(|e| format!("add_note payload のパースに失敗しました: {}", e))?;
            crate::db::add_project_note(
                app.clone(),
                project_id.to_string(),
                args.sprint_id,
                args.title.clone(),
                args.content,
                Some("po_assistant".to_string()),
            )
            .await
            .map_err(|e| format!("ふせんの追加に失敗しました: {}", e))?;
            let _ = app.emit("kanban-updated", ());
            Ok(Some(format!(
                "ふせん「{}」をボードに追加しました。",
                args.title
            )))
        }
        "suggest_retro" => {
            let args: crate::ai_tools::SuggestRetroItemArgs =
                serde_json::from_value(action.payload)
                    .map_err(|e| format!("suggest_retro payload のパースに失敗しました: {}", e))?;
            let sessions = crate::db::get_retro_sessions(app.clone(), project_id.to_string())
                .await
                .map_err(|e| format!("レトロセッションの取得に失敗しました: {}", e))?;
            let active = sessions
                .iter()
                .find(|s| s.status == "draft" || s.status == "in_progress");
            match active {
                Some(session) => {
                    crate::db::add_retro_item(
                        app.clone(),
                        session.id.clone(),
                        args.category.clone(),
                        args.content.clone(),
                        "po".to_string(),
                        None,
                        None,
                    )
                    .await
                    .map_err(|e| format!("レトロアイテムの追加に失敗しました: {}", e))?;
                    let _ = app.emit("kanban-updated", ());
                    let label = match args.category.as_str() {
                        "keep" => "Keep",
                        "problem" => "Problem",
                        "try" => "Try",
                        _ => &args.category,
                    };
                    Ok(Some(format!(
                        "レトロの {} に「{}」を追加しました。",
                        label, args.content
                    )))
                }
                None => Ok(Some(
                    "アクティブなレトロセッションがないため、レトロアイテムの追加をスキップしました。"
                        .to_string(),
                )),
            }
        }
        unknown => Ok(Some(format!(
            "不明なアクション種別「{}」はスキップしました。",
            unknown
        ))),
    }
}

async fn apply_team_leader_operations(
    app: &AppHandle,
    project_id: &str,
    operations: Vec<crate::ai_tools::CreateStoryAndTasksArgs>,
) -> Result<(), String> {
    for operation in operations {
        if operation.tasks.is_empty() {
            continue;
        }

        crate::ai_tools::guard_story_creation_against_duplicates(
            app,
            project_id,
            operation.target_story_id.as_deref(),
            operation.story_title.as_deref(),
        )
        .await?;

        let story_draft = crate::db::StoryDraftInput {
            target_story_id: operation.target_story_id.clone(),
            title: operation
                .story_title
                .clone()
                .unwrap_or_else(|| "Untitled Story".to_string()),
            description: operation.story_description.clone(),
            acceptance_criteria: operation.acceptance_criteria.clone(),
            priority: operation.story_priority,
        };

        crate::db::insert_story_with_tasks(app, project_id, story_draft, operation.tasks).await?;
    }

    Ok(())
}

pub(crate) async fn apply_team_leader_execution_plan(
    app: &AppHandle,
    project_id: &str,
    plan: PoAssistantExecutionPlan,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ChatTaskResponse>, String> {
    let PoAssistantExecutionPlan {
        reply,
        operations,
        actions,
    } = plan;

    let mut action_results = Vec::new();
    for action in actions {
        if let Some(result) = apply_team_leader_action(app, project_id, action).await? {
            action_results.push(result);
        }
    }

    if !action_results.is_empty() {
        let summary = if let Some(r) = reply {
            format!("{}\n\n{}", r, action_results.join("\n"))
        } else {
            action_results.join("\n")
        };
        return Ok(Some(ChatTaskResponse { reply: summary }));
    }

    if operations.is_empty() {
        return Ok(None);
    }

    apply_team_leader_operations(app, project_id, operations).await?;

    let after_counts = get_project_backlog_counts(app, project_id).await?;
    let Some(response) = super::super::heuristics::build_backlog_counts_reply(
        reply.unwrap_or_else(|| "バックログ登録を実行しました。".to_string()),
        before_counts,
        after_counts,
    ) else {
        return Ok(None);
    };

    let _ = app.emit("kanban-updated", ());
    Ok(Some(response))
}
