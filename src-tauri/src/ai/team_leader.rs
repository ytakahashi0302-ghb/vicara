use super::common::{
    execute_po_cli_prompt, get_project_backlog_counts, parse_json_response, record_cli_usage,
    record_provider_usage, resolve_po_transport, serialize_chat_history, ChatTaskResponse, Message,
    PoAssistantExecutionPlan, PoTransport, ProjectBacklogCounts,
};
use tauri::AppHandle;

mod heuristics;
mod plan;
mod prompts;

#[tauri::command]
pub async fn chat_with_team_leader(
    app: AppHandle,
    project_id: String,
    messages_history: Vec<Message>,
) -> Result<ChatTaskResponse, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let before_counts: ProjectBacklogCounts = get_project_backlog_counts(&app, &project_id).await?;
    let latest_user_index = messages_history
        .iter()
        .rposition(|message| message.role == "user");
    let (latest_user_message, prior_messages) = if let Some(index) = latest_user_index {
        let latest = messages_history[index].content.clone();
        let prior = messages_history[..index].to_vec();
        (latest, prior)
    } else {
        (String::new(), messages_history.clone())
    };
    let generic_backlog_request =
        heuristics::looks_like_generic_backlog_creation_request(&latest_user_message);
    let has_product_context = heuristics::has_product_context_document(&context_md);

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let mutation_requested = heuristics::looks_like_mutation_request(&latest_user_message);
            let system_prompt = prompts::build_po_assistant_api_system_prompt(&context_md);

            let raw_text = match plan::chat_team_leader_with_tools_with_retry(
                &app,
                &provider,
                &api_key,
                &model,
                &system_prompt,
                &latest_user_message,
                &prior_messages,
                &project_id,
            )
            .await
            {
                Ok(response) => response,
                Err(error) => {
                    if mutation_requested {
                        if let Some(partial_success_response) =
                            heuristics::build_partial_team_leader_success_response(
                                &app,
                                &project_id,
                                before_counts,
                                &error,
                            )
                            .await?
                        {
                            return Ok(partial_success_response);
                        }
                    }

                    if heuristics::is_transient_provider_unavailable(&error) {
                        return Ok(heuristics::build_team_leader_provider_unavailable_reply(
                            &error,
                            mutation_requested,
                        ));
                    }

                    return Err(error);
                }
            };
            record_provider_usage(&app, &project_id, "team_leader", &raw_text).await;
            let data_changed =
                heuristics::detect_backlog_change_with_retry(&app, &project_id, before_counts)
                    .await?;

            if mutation_requested && !data_changed {
                if generic_backlog_request && !has_product_context {
                    return Ok(ChatTaskResponse {
                        reply: heuristics::build_missing_product_context_reply(),
                    });
                }

                if let Some(fallback_response) = plan::execute_fallback_team_leader_plan(
                    &app,
                    &provider,
                    &api_key,
                    &model,
                    &project_id,
                    &context_md,
                    &latest_user_message,
                    before_counts,
                )
                .await?
                {
                    return Ok(fallback_response);
                }

                return Ok(ChatTaskResponse {
                    reply: if generic_backlog_request {
                        "PRODUCT_CONTEXT.md を踏まえた具体的なバックログ案を生成できず、実際のバックログ件数変化も確認できませんでした。今回は成功扱いにせず停止します。プロジェクトの Local Path と PRODUCT_CONTEXT.md の内容を確認してから再試行してください。".to_string()
                    } else {
                        "登録・追加系の依頼として解釈しましたが、実際にはバックログの件数変化を確認できませんでした。今回は成功扱いにせず停止します。`create_story_and_tasks` の未実行または失敗が疑われるため、再試行時は対象ストーリーIDを明示して実行してください。".to_string()
                    },
                });
            }

            let resp: ChatTaskResponse = match parse_json_response(&raw_text.content) {
                Ok(r) => r,
                Err(_) => ChatTaskResponse {
                    reply: raw_text.content,
                },
            };

            Ok(resp)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let history_block = if prior_messages.is_empty() {
                "（会話履歴なし）".to_string()
            } else {
                serialize_chat_history(&prior_messages)
            };
            let cli_prompt = prompts::build_po_assistant_cli_prompt(
                &context_md,
                &history_block,
                &latest_user_message,
            );
            let result = execute_po_cli_prompt::<PoAssistantExecutionPlan>(
                &cli_type,
                &model,
                &cli_prompt,
                &cwd,
            )
            .await?;
            record_cli_usage(
                &app,
                &project_id,
                "team_leader",
                &cli_type,
                &result.metadata,
            )
            .await;

            let plan_result = result.value;
            if plan_result.operations.is_empty() && plan_result.actions.is_empty() {
                if generic_backlog_request {
                    if !has_product_context {
                        return Ok(ChatTaskResponse {
                            reply: heuristics::build_missing_product_context_reply(),
                        });
                    }

                    if let Some(applied_response) = plan::execute_contextual_cli_backlog_plan(
                        &app,
                        &project_id,
                        cli_type,
                        &model,
                        &cwd,
                        &context_md,
                        &latest_user_message,
                        before_counts,
                    )
                    .await?
                    {
                        return Ok(applied_response);
                    }
                }

                return Ok(ChatTaskResponse {
                    reply: plan_result
                        .reply
                        .unwrap_or_else(|| "判断材料を整理しました。".to_string()),
                });
            }

            if let Some(applied_response) = plan::apply_team_leader_execution_plan(
                &app,
                &project_id,
                plan_result,
                before_counts,
            )
            .await?
            {
                return Ok(applied_response);
            }

            if generic_backlog_request {
                if !has_product_context {
                    return Ok(ChatTaskResponse {
                        reply: heuristics::build_missing_product_context_reply(),
                    });
                }

                if let Some(applied_response) = plan::execute_contextual_cli_backlog_plan(
                    &app,
                    &project_id,
                    cli_type,
                    &model,
                    &cwd,
                    &context_md,
                    &latest_user_message,
                    before_counts,
                )
                .await?
                {
                    return Ok(applied_response);
                }
            }

            Ok(ChatTaskResponse {
                reply: "登録・追加系の計画を受け取りましたが、実際にはバックログの件数変化を確認できませんでした。今回は成功扱いにせず停止します。対象ストーリーIDや生成タスク内容を見直して再試行してください。".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{heuristics, prompts};
    use crate::ai::common::ProjectBacklogCounts;

    #[test]
    fn generic_backlog_creation_request_is_detected() {
        assert!(heuristics::looks_like_generic_backlog_creation_request(
            "バックログを1つ作成してください"
        ));
    }

    #[test]
    fn task_addition_to_existing_story_does_not_use_generic_story_fallback() {
        assert!(!heuristics::looks_like_generic_backlog_creation_request(
            "既存 story ID: abc にタスクを追加してください"
        ));
    }

    #[test]
    fn product_context_document_is_detected_from_project_context_block() {
        assert!(heuristics::has_product_context_document(
            "\n【プロジェクト既存ドキュメントコンテキスト】\n--- PRODUCT_CONTEXT.md ---\n# sample"
        ));
        assert!(!heuristics::has_product_context_document(
            "\n【現在のバックログ】\nstory-1: 既存ストーリー"
        ));
    }

    #[test]
    fn backlog_counts_reply_reports_actual_deltas() {
        let response = heuristics::build_backlog_counts_reply(
            "部分成功".to_string(),
            ProjectBacklogCounts {
                stories: 2,
                tasks: 5,
                dependencies: 1,
            },
            ProjectBacklogCounts {
                stories: 3,
                tasks: 8,
                dependencies: 4,
            },
        )
        .expect("reply should exist when backlog changes");

        assert!(response.reply.contains("部分成功"));
        assert!(response.reply.contains("stories +1"));
        assert!(response.reply.contains("tasks +3"));
        assert!(response.reply.contains("dependencies +3"));
    }

    #[test]
    fn transient_provider_unavailable_detects_gemini_503() {
        let error = "Gemini error: CompletionError: HttpError: Invalid status code 503 Service Unavailable with message: {\"error\":{\"status\":\"UNAVAILABLE\",\"message\":\"high demand\"}}";
        assert!(heuristics::is_transient_provider_unavailable(error));
    }

    #[test]
    fn provider_unavailable_reply_mentions_no_creation_for_mutation() {
        let response = heuristics::build_team_leader_provider_unavailable_reply(
            "Gemini error: 503 Service Unavailable",
            true,
        );

        assert!(response
            .reply
            .contains("今回はバックログを作成していません"));
        assert!(response.reply.contains("503 Service Unavailable"));
    }

    #[test]
    fn po_assistant_prompts_share_quality_gates() {
        let api_prompt = prompts::build_po_assistant_api_system_prompt("context");
        let cli_prompt = prompts::build_po_assistant_cli_prompt("context", "history", "依頼");

        assert!(api_prompt.contains("【完了条件】"));
        assert!(cli_prompt.contains("【完了条件】"));
        assert!(api_prompt.contains("【自己検証】"));
        assert!(cli_prompt.contains("【自己検証】"));
        assert!(api_prompt.contains("PBI"));
        assert!(cli_prompt.contains("PBI"));
        assert!(api_prompt.contains("自然な日本語"));
        assert!(cli_prompt.contains("やること:"));
        assert!(cli_prompt.contains("検証観点:"));
    }

    #[test]
    fn contextual_backlog_prompt_requires_self_check() {
        let prompt = prompts::build_contextual_backlog_generation_system_prompt(
            "--- PRODUCT_CONTEXT.md ---",
        );

        assert!(prompt.contains("完了条件"));
        assert!(prompt.contains("自己検証"));
        assert!(prompt.contains("operations"));
        assert!(prompt.contains("自然な日本語"));
        assert!(prompt.contains("やること:"));
        assert!(prompt.contains("完了状態:"));
    }
}
