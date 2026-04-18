use super::common::{
    execute_po_cli_prompt, parse_json_response, record_cli_usage, record_provider_usage,
    resolve_po_transport, serialize_chat_history, Message, PoTransport, RefinedIdeaResponse,
};
use tauri::AppHandle;

const IDEA_REFINE_SYSTEM_PROMPT: &str = r#"あなたは vicara の POアシスタントです。ユーザーの曖昧なアイデアを、実装前に判断しやすいユーザーストーリー草案へ整理してください。

【役割】
- ユーザーの意図・価値・対象ユーザー・制約を短く整理する
- 不足情報があっても、現時点で妥当な草案を作り、足りない点は reply で簡潔に補う
- まだ断定できない情報は、過剰に作り込まず安全な表現でまとめる

【完了条件】
- `reply` がユーザーに返す短い整理メッセージとして自然である
- `story_draft.title` が具体的で、プレースホルダ名になっていない
- `story_draft.description` に背景・価値・範囲が含まれている
- `story_draft.acceptance_criteria` が確認可能な完了条件になっている

【自己検証】
- reply と story_draft の内容が矛盾していないか確認する
- 「いい感じにする」「詳細を詰める」などの曖昧語だけで終わっていないか確認する
- 出力が JSON オブジェクトのみで、前後に説明や Markdown を付けていないか確認する

【出力フォーマット】
以下の JSON オブジェクトのみを返してください。
{
  "reply": "ユーザーへ返す短い整理メッセージ",
  "story_draft": {
    "title": "ストーリータイトル",
    "description": "背景・価値・範囲が分かる説明",
    "acceptance_criteria": "受け入れ条件"
  }
}"#;

fn build_idea_refine_api_prompt(context_md: &str, idea_seed: &str) -> String {
    format!(
        r#"【プロジェクトコンテキスト】
{context_md}

【今回のユーザー入力】
{idea_seed}

会話履歴も考慮しつつ、上記入力を実装前のユーザーストーリー草案に整理してください。"#
    )
}

fn build_idea_refine_cli_prompt(context_md: &str, history_block: &str, idea_seed: &str) -> String {
    format!(
        r#"{IDEA_REFINE_SYSTEM_PROMPT}

【プロジェクトコンテキスト】
{context_md}

【これまでの会話】
{history_block}

【今回のユーザー入力】
{idea_seed}"#
    )
}

#[tauri::command]
pub async fn refine_idea(
    app: AppHandle,
    idea_seed: String,
    previous_context: Option<Vec<Message>>,
    project_id: String,
) -> Result<RefinedIdeaResponse, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let previous_messages = previous_context.unwrap_or_default();

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let chat_history = crate::rig_provider::convert_messages(&previous_messages);
            let prompt = build_idea_refine_api_prompt(&context_md, &idea_seed);
            let content = crate::rig_provider::chat_with_history(
                &provider,
                &api_key,
                &model,
                IDEA_REFINE_SYSTEM_PROMPT,
                &prompt,
                chat_history,
            )
            .await?;
            record_provider_usage(&app, &project_id, "idea_refine", &content).await;

            parse_json_response(&content.content)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let history_block = if previous_messages.is_empty() {
                "（会話履歴なし）".to_string()
            } else {
                serialize_chat_history(&previous_messages)
            };
            let cli_prompt = build_idea_refine_cli_prompt(&context_md, &history_block, &idea_seed);
            let result =
                execute_po_cli_prompt::<RefinedIdeaResponse>(&cli_type, &model, &cli_prompt, &cwd)
                    .await?;
            record_cli_usage(
                &app,
                &project_id,
                "idea_refine",
                &cli_type,
                &result.metadata,
            )
            .await;

            Ok(result.value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_idea_refine_api_prompt, build_idea_refine_cli_prompt, IDEA_REFINE_SYSTEM_PROMPT,
    };

    #[test]
    fn idea_refine_system_prompt_describes_completion_and_self_check() {
        assert!(IDEA_REFINE_SYSTEM_PROMPT.contains("【完了条件】"));
        assert!(IDEA_REFINE_SYSTEM_PROMPT.contains("【自己検証】"));
        assert!(IDEA_REFINE_SYSTEM_PROMPT.contains("\"story_draft\""));
    }

    #[test]
    fn idea_refine_cli_prompt_includes_history_and_context() {
        let prompt = build_idea_refine_cli_prompt(
            "# PRODUCT_CONTEXT.md\n- target user",
            "## ユーザー\n在庫管理を楽にしたい",
            "棚卸しをもっと簡単にしたい",
        );

        assert!(prompt.contains("【これまでの会話】"));
        assert!(prompt.contains("棚卸しをもっと簡単にしたい"));
        assert!(prompt.contains("PRODUCT_CONTEXT.md"));
    }

    #[test]
    fn idea_refine_api_prompt_includes_latest_input() {
        let prompt = build_idea_refine_api_prompt(
            "# PRODUCT_CONTEXT.md\n- user",
            "複数店舗の在庫差分を見たい",
        );

        assert!(prompt.contains("【今回のユーザー入力】"));
        assert!(prompt.contains("複数店舗の在庫差分を見たい"));
    }
}
