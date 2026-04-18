use super::common::{
    execute_po_cli_prompt, parse_json_response, record_cli_usage, record_provider_usage,
    resolve_po_transport, GeneratedTask, PoTransport,
};
use tauri::AppHandle;

const TASK_GENERATION_SYSTEM_PROMPT: &str = r#"You are a task decomposition expert for agile software development.
Goal:
- Convert one backlog story into 3-8 concrete implementation tasks that a single assignee can start without restating the story.
- Return all task titles and descriptions in natural Japanese.
- Keep proper nouns, product names, API names, protocols, and identifiers unchanged only when translation would make them less accurate.

Each task object must include:
- "title": string (concise and action-oriented)
- "description": string (implementation details and scope)
- "priority": integer 1-5 (REQUIRED; lower number = higher priority)
- "blocked_by_indices": number[] (zero-based indices of prerequisite tasks in this array; omit or use [] if none)

Required description format:
- "description" must be a single Japanese string with the following labeled sections in this order:
  - "やること: ..."
  - "対象範囲: ..."
  - "完了状態: ..."
  - "検証観点: ..."

Priority guidelines (integer 1-5, lower = more urgent):
- 1: Most critical - architecture foundation, blocking everything else
- 2: High priority - core functionality on the critical path
- 3: Medium - important feature work, not blocking others (default)
- 4: Low - supporting tasks, tests, minor improvements
- 5: Lowest - documentation, polish, optional enhancements

Dependency guidelines:
- Use blocked_by_indices to express "this task cannot start until task N is done"
- Only reference earlier tasks in the same array
- Keep dependency chains short and avoid circular references

Quality bar:
- Use product-specific wording instead of placeholders like "Implement feature"
- Cover the acceptance criteria with the whole task set
- Include validation or test work when it is necessary to prove the story is done
- Avoid duplicate tasks or overly fine-grained busywork
- Do not output vague tasks that only say "implement", "support", "handle", "adjust", or "test" without concrete behavior
- Each task must explain what should change, which behavior or workflow is covered, what observable state means it is done, and what should be checked
- Focus on what should be achieved and how the work should progress; do not mention file paths, function names, or code locations unless the story explicitly requires them

Decomposition checklist:
- Check whether the story needs work in these areas: state/data, business logic, user interaction, failure handling, and validation
- Create tasks only for the areas that are actually needed, but do not leave obvious gaps in the acceptance criteria

Self-check before answering:
- Verify the output is valid JSON array with no markdown fences
- Verify every priority is an integer between 1 and 5
- Verify blocked_by_indices never reference missing, future, or same-index tasks
- Verify every title and description is written in Japanese
- Verify every description contains "やること:", "対象範囲:", "完了状態:", and "検証観点:"
- Verify the task set covers the story and acceptance criteria without obvious duplicates

Return ONLY a valid JSON array.
Do not wrap the array in markdown code fences.
Do not include any explanation before or after the JSON."#;

fn build_task_generation_prompt(
    context_md: &str,
    title: &str,
    description: &str,
    acceptance_criteria: &str,
) -> String {
    format!(
        r#"【プロジェクトコンテキスト】
{context_md}

【対象ストーリー】
- title: {title}
- description: {description}
- acceptance_criteria: {acceptance_criteria}

【依頼】
- 受け入れ条件を満たすために必要な実装タスクへ分解してください
- タスクは着手可能な粒度で、プレースホルダ名や曖昧な表現を避けてください
- 出力する task の `title` と `description` は自然な日本語にしてください
- 各 task の `description` には、必ず次の 4 項目をこの順番で含めてください
  - `やること: ...`
  - `対象範囲: ...`
  - `完了状態: ...`
  - `検証観点: ...`
- `description` では、実現したい振る舞い・状態・業務ルール・検証内容を具体的に書いてください
- 「どのファイルや関数を触るか」は不要です。実装場所ではなく、達成すべき内容を具体化してください
- story が抽象的でも、開発担当がそのまま着手できる具体度まで分解してください
- 必要に応じて、状態/データ、処理ロジック、UI/操作、失敗時の扱い、検証の観点を確認し、抜けがないようにしてください
- テスト・検証が必要なら、依存関係を踏まえてタスクに含めてください"#
    )
}

#[tauri::command]
pub async fn generate_tasks_from_story(
    app: AppHandle,
    title: String,
    description: String,
    acceptance_criteria: String,
    provider: String,
    project_id: String,
) -> Result<Vec<GeneratedTask>, String> {
    let transport = resolve_po_transport(&app, &project_id, Some(provider)).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let prompt =
        build_task_generation_prompt(&context_md, &title, &description, &acceptance_criteria);

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let response = crate::rig_provider::chat_with_history(
                &provider,
                &api_key,
                &model,
                TASK_GENERATION_SYSTEM_PROMPT,
                &prompt,
                vec![],
            )
            .await?;
            record_provider_usage(&app, &project_id, "task_generation", &response).await;

            parse_json_response(&response.content)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let cli_prompt = format!("{TASK_GENERATION_SYSTEM_PROMPT}\n\n{prompt}");
            let result =
                execute_po_cli_prompt::<Vec<GeneratedTask>>(&cli_type, &model, &cli_prompt, &cwd)
                    .await?;
            record_cli_usage(
                &app,
                &project_id,
                "task_generation",
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
    use super::{build_task_generation_prompt, TASK_GENERATION_SYSTEM_PROMPT};

    #[test]
    fn task_generation_system_prompt_includes_quality_gate_and_json_contract() {
        assert!(TASK_GENERATION_SYSTEM_PROMPT.contains("Self-check before answering"));
        assert!(TASK_GENERATION_SYSTEM_PROMPT.contains("valid JSON array"));
        assert!(TASK_GENERATION_SYSTEM_PROMPT.contains("blocked_by_indices"));
        assert!(TASK_GENERATION_SYSTEM_PROMPT.contains("natural Japanese"));
        assert!(TASK_GENERATION_SYSTEM_PROMPT.contains("やること:"));
        assert!(TASK_GENERATION_SYSTEM_PROMPT.contains("完了状態:"));
    }

    #[test]
    fn task_generation_prompt_includes_story_context_and_acceptance_criteria() {
        let prompt = build_task_generation_prompt(
            "# PRODUCT_CONTEXT.md\n- target user",
            "通知設定を編集する",
            "通知チャネルを切り替えたい",
            "メール通知のON/OFFを保存できる",
        );

        assert!(prompt.contains("【プロジェクトコンテキスト】"));
        assert!(prompt.contains("通知設定を編集する"));
        assert!(prompt.contains("メール通知のON/OFFを保存できる"));
        assert!(prompt.contains("title` と `description` は自然な日本語"));
        assert!(prompt.contains("やること:"));
        assert!(prompt.contains("検証観点:"));
    }
}
