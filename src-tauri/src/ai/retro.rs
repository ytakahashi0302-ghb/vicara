use super::common::{
    execute_po_cli_prompt, parse_json_response, resolve_po_transport, PoTransport,
};
use serde::Deserialize;
use tauri::AppHandle;

const RETRO_REVIEW_SOURCE_KIND: &str = "retrospective";
const RETRO_MAX_RUNS_IN_PROMPT: usize = 10;
const RETRO_REASONING_TAIL_CHARS: usize = 1_500;
const RETRO_FINAL_ANSWER_HEAD_CHARS: usize = 2_000;
const RETRO_CHANGED_FILES_CHARS: usize = 1_000;
const RETRO_LOG_SECTION_CAP_CHARS: usize = 20_000;
const RETRO_SUMMARY_FALLBACK_MAX_CHARS: usize = 4_000;

#[derive(Debug, Clone, Deserialize)]
struct RetroReviewItem {
    category: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RetroSynthesisResponse {
    summary_markdown: String,
    #[serde(default)]
    items: Vec<RetroReviewItem>,
}

fn take_head_chars(input: &str, max_chars: usize) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let total = trimmed.chars().count();
    if total <= max_chars {
        return trimmed.to_string();
    }
    let head: String = trimmed.chars().take(max_chars).collect();
    format!("{head}…(先頭 {max_chars} 文字)")
}

fn take_tail_chars(input: &str, max_chars: usize) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let total = trimmed.chars().count();
    if total <= max_chars {
        return trimmed.to_string();
    }
    let tail: String = trimmed
        .chars()
        .skip(total.saturating_sub(max_chars))
        .collect();
    format!("…(末尾 {max_chars} 文字)\n{tail}")
}

fn truncate_to_chars(input: &str, max_chars: usize) -> String {
    let total = input.chars().count();
    if total <= max_chars {
        return input.to_string();
    }
    let head: String = input.chars().take(max_chars).collect();
    format!("{head}\n…(truncated)")
}

fn format_changed_files(raw: &Option<String>) -> String {
    let value = match raw {
        Some(value) => value.trim(),
        None => return "(なし)".to_string(),
    };
    if value.is_empty() {
        return "(なし)".to_string();
    }
    if let Ok(json) = serde_json::from_str::<Vec<String>>(value) {
        if json.is_empty() {
            return "(なし)".to_string();
        }
        let joined = json
            .iter()
            .map(|path| format!("- {path}"))
            .collect::<Vec<_>>()
            .join("\n");
        return take_head_chars(&joined, RETRO_CHANGED_FILES_CHARS);
    }
    take_head_chars(value, RETRO_CHANGED_FILES_CHARS)
}

fn build_retro_review_prompt(
    role: &crate::db::TeamRole,
    tasks: &[crate::db::Task],
    runs: &[crate::db::AgentRetroRun],
    notes: &[crate::db::ProjectNote],
    usage: &crate::db::SprintLlmUsageSummary,
) -> String {
    let system_prompt_head = take_head_chars(&role.system_prompt, 300);

    let task_section = if tasks.is_empty() {
        "(担当タスクなし)".to_string()
    } else {
        tasks
            .iter()
            .map(|task| {
                let description = task
                    .description
                    .as_deref()
                    .map(|d| take_head_chars(d, 200))
                    .unwrap_or_default();
                format!(
                    "- [{status}] {title}{desc}",
                    status = task.status,
                    title = task.title,
                    desc = if description.is_empty() {
                        String::new()
                    } else {
                        format!("\n    {description}")
                    }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let run_count = runs.len();
    let selected_runs: Vec<&crate::db::AgentRetroRun> = runs
        .iter()
        .rev()
        .take(RETRO_MAX_RUNS_IN_PROMPT)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let run_section_raw = if selected_runs.is_empty() {
        "(実行ログなし)".to_string()
    } else {
        selected_runs
            .iter()
            .enumerate()
            .map(|(idx, run)| {
                let final_answer_excerpt = run
                    .final_answer
                    .as_deref()
                    .map(|value| take_head_chars(value, RETRO_FINAL_ANSWER_HEAD_CHARS))
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "(final_answer なし)".to_string());
                let reasoning_excerpt = run
                    .reasoning_log
                    .as_deref()
                    .map(|value| take_tail_chars(value, RETRO_REASONING_TAIL_CHARS))
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| "(reasoning_log なし)".to_string());
                let changed_files = format_changed_files(&run.changed_files_json);
                format!(
                    "### Run {index} — started_at={started} — success={success} — tool_events={tools}\n- final_answer:\n{final_answer}\n- reasoning_log (末尾):\n{reasoning}\n- changed_files:\n{changed_files}",
                    index = idx + 1,
                    started = run.started_at,
                    success = run.success,
                    tools = run.tool_event_count,
                    final_answer = final_answer_excerpt,
                    reasoning = reasoning_excerpt,
                    changed_files = changed_files,
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    let run_section = if run_section_raw.chars().count() > RETRO_LOG_SECTION_CAP_CHARS {
        log::warn!(
            "retro prompt: run section truncated ({} chars -> {})",
            run_section_raw.chars().count(),
            RETRO_LOG_SECTION_CAP_CHARS
        );
        truncate_to_chars(&run_section_raw, RETRO_LOG_SECTION_CAP_CHARS)
    } else {
        run_section_raw
    };

    let notes_section = if notes.is_empty() {
        "(関連ノートなし)".to_string()
    } else {
        notes
            .iter()
            .map(|note| {
                let content_excerpt = take_head_chars(&note.content, 500);
                format!(
                    "- **{title}**\n{content}",
                    title = note.title,
                    content = content_excerpt
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    };

    format!(
        r#"あなたは熟練のスクラムマスター補佐です。以下のスプリント活動ログを読み、担当ロール「{role_name}」の観点で Keep / Problem / Try を抽出してください。

# 担当ロール
- 名前: {role_name}
- 役割: {role_prompt}

# スプリント統計
- LLM呼び出し回数: {events}
- 失敗回数: {failures}
- 入出力トークン: {input_tokens} / {output_tokens}
- 推定コスト(USD): {cost:.4}

# 担当タスク一覧 ({task_count} 件)
{tasks}

# 実行ログ抜粋 (最新 {selected}/{total} 件)
{runs}

# プロジェクトノート
{notes}

# 出力フォーマット（厳守）
JSON 配列のみを返してください。前後に説明や ``` を付けないでください。
3 〜 6 件、Keep / Problem / Try のバランスを意識した日本語で記述してください。
- 各 item はログやタスクに根拠がある具体的な表現にし、「改善したい」「注意する」だけの抽象表現で終わらせないでください。
- 出力前に category が keep / problem / try のいずれかであること、重複した指摘がないことを確認してください。
[
  {{"category": "keep|problem|try", "content": "..."}},
  ...
]"#,
        role_name = role.name,
        role_prompt = system_prompt_head,
        events = usage.total_events,
        failures = usage.failure_count,
        input_tokens = usage.total_input_tokens,
        output_tokens = usage.total_output_tokens,
        cost = usage.total_cost_usd,
        task_count = tasks.len(),
        tasks = task_section,
        selected = selected_runs.len(),
        total = run_count,
        runs = run_section,
        notes = notes_section,
    )
}

fn build_retro_kpt_synthesis_prompt(
    items: &[crate::db::RetroItem],
    role_lookup: &std::collections::HashMap<String, String>,
    usage: &crate::db::SprintLlmUsageSummary,
) -> String {
    let source_items: Vec<&crate::db::RetroItem> =
        items.iter().filter(|item| item.source != "sm").collect();

    let grouped = |category: &str| -> String {
        let lines: Vec<String> = source_items
            .iter()
            .filter(|item| item.category == category)
            .map(|item| {
                let role_label = item
                    .source_role_id
                    .as_deref()
                    .and_then(|id| role_lookup.get(id))
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| match item.source.as_str() {
                        "po" => "PO",
                        "user" => "ユーザー",
                        _ => "不明",
                    });
                let content_excerpt = take_head_chars(&item.content, 600);
                format!(
                    "[{role}] {content}",
                    role = role_label,
                    content = content_excerpt
                )
            })
            .collect();
        if lines.is_empty() {
            "(該当アイテムなし)".to_string()
        } else {
            lines.join("\n")
        }
    };

    let item_count = source_items.len();

    format!(
        r##"あなたは経験豊富なスクラムマスター (SM) です。
以下に各ロール（開発エージェント・PO・ユーザー）が個別に出した KPT の生データを示します。
あなたの役割はこれらを**そのまま転記することではなく**、チーム全体を俯瞰した上で「ロール横断パターン」「根本原因」「プロセス改善機会」を抽出し、次スプリントに直結する洞察を生成することです。

## SM の視点で必ず行うこと
- 複数ロールに共通して見られる傾向をひとつの上位アイテムにまとめる
- 表面的な現象（例: "テストが遅い"）ではなく、根本原因（例: "テスト環境の共有によるボトルネック"）を示す
- Try は「誰が」「何を」「いつまでに」が想像できる具体的アクションにする
- 既存アイテムを言い換えるだけのアイテムは生成しない
- SM サマリは単なる箇条書きではなく、このスプリントの「物語」を 1〜2 段落で語ること

## スプリント規模
- LLM 呼び出し: {events} 回 / 失敗: {failures} 回
- トークン: 入力 {input_tokens} / 出力 {output_tokens}
- 推定コスト: ${cost:.4}

## 素材アイテム数: {item_count} 件

### Keep（良かったこと）
{keeps}

### Problem（課題・問題）
{problems}

### Try（次に試すこと）
{tries}

## 出力要件
1. 各カテゴリ 2〜5 件、チーム全体視点の**新たな洞察**として書く（入力の言い換え禁止）
2. summary_markdown は 400〜800 字程度のMarkdown。見出し「## ハイライト」「## リスク」「## 次スプリントへ」の構造を使う
3. 日本語で出力する
4. 出力前に summary_markdown の見出し構造、items のカテゴリ値、重複有無を自己確認する

## 出力フォーマット（厳守）
前後に説明や ``` を付けず、以下の JSON オブジェクトのみを返してください。
{{
  "summary_markdown": "...",
  "items": [
    {{"category": "keep|problem|try", "content": "チーム全体視点の洞察"}},
    ...
  ]
}}"##,
        events = usage.total_events,
        failures = usage.failure_count,
        input_tokens = usage.total_input_tokens,
        output_tokens = usage.total_output_tokens,
        cost = usage.total_cost_usd,
        item_count = item_count,
        keeps = grouped("keep"),
        problems = grouped("problem"),
        tries = grouped("try"),
    )
}

fn normalize_retro_category(raw: &str) -> String {
    let lowered = raw.trim().to_lowercase();
    match lowered.as_str() {
        "keep" | "problem" | "try" => lowered,
        _ => "problem".to_string(),
    }
}

fn parse_retro_review_items(content: &str) -> Vec<RetroReviewItem> {
    match parse_json_response::<Vec<RetroReviewItem>>(content) {
        Ok(items) => items
            .into_iter()
            .filter(|item| !item.content.trim().is_empty())
            .collect(),
        Err(error) => {
            log::warn!("retro review JSON parse failed: {error}");
            let fallback_content = take_head_chars(content, RETRO_SUMMARY_FALLBACK_MAX_CHARS);
            vec![RetroReviewItem {
                category: "problem".to_string(),
                content: format!("(自動分類に失敗) {fallback_content}"),
            }]
        }
    }
}

fn parse_retro_synthesis_response(content: &str) -> RetroSynthesisResponse {
    match parse_json_response::<RetroSynthesisResponse>(content) {
        Ok(parsed) => parsed,
        Err(error) => {
            log::warn!("retro synthesis JSON parse failed: {error}");
            RetroSynthesisResponse {
                summary_markdown: take_head_chars(content, RETRO_SUMMARY_FALLBACK_MAX_CHARS),
                items: Vec::new(),
            }
        }
    }
}

async fn call_retro_llm(
    app: &AppHandle,
    project_id: &str,
    sprint_id: &str,
    system_prompt: &str,
    prompt: &str,
    transport: PoTransport,
) -> Result<String, String> {
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
                system_prompt,
                prompt,
                vec![],
            )
            .await?;
            if let Err(error) = crate::llm_observability::record_llm_usage(
                app,
                crate::llm_observability::RecordLlmUsageInput {
                    project_id: project_id.to_string(),
                    task_id: None,
                    sprint_id: Some(sprint_id.to_string()),
                    source_kind: RETRO_REVIEW_SOURCE_KIND.to_string(),
                    transport_kind: "provider_api".to_string(),
                    provider: response.provider.clone(),
                    model: response.model.clone(),
                    usage: response.usage,
                    measurement_status: None,
                    request_started_at: Some(response.started_at),
                    request_completed_at: Some(response.completed_at),
                    success: true,
                    error_message: None,
                    raw_usage_json: Some(response.raw_usage_json.clone()),
                },
            )
            .await
            {
                log::warn!("Failed to record retro LLM usage (api): {error}");
            }
            Ok(response.content)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let cli_prompt = format!("{system_prompt}\n\n{prompt}");
            let result =
                execute_po_cli_prompt::<serde_json::Value>(&cli_type, &model, &cli_prompt, &cwd)
                    .await?;
            if let Err(error) = crate::llm_observability::record_cli_usage(
                app,
                crate::llm_observability::CliUsageRecordInput {
                    project_id: Some(project_id.to_string()),
                    task_id: None,
                    sprint_id: Some(sprint_id.to_string()),
                    source_kind: RETRO_REVIEW_SOURCE_KIND.to_string(),
                    cli_type: cli_type.as_str().to_string(),
                    model: result.metadata.model.clone(),
                    prompt_tokens: None,
                    completion_tokens: None,
                    total_tokens: None,
                    cached_input_tokens: None,
                    request_started_at: result.metadata.request_started_at,
                    request_completed_at: result.metadata.request_completed_at,
                    success: true,
                    error_message: None,
                },
            )
            .await
            {
                log::warn!("Failed to record retro LLM usage (cli): {error}");
            }
            Ok(result.value.to_string())
        }
    }
}

#[tauri::command]
pub async fn generate_agent_retro_review(
    app: AppHandle,
    project_id: String,
    sprint_id: String,
    retro_session_id: String,
    role_id: String,
    skip_inactive: bool,
) -> Result<Vec<crate::db::RetroItem>, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let role = crate::db::get_team_role_by_id(&app, &role_id)
        .await?
        .ok_or_else(|| format!("team_role が見つかりません: {role_id}"))?;
    let tasks = crate::db::get_tasks_by_sprint_and_role(&app, &sprint_id, &role_id).await?;
    let runs =
        crate::db::get_agent_retro_runs_by_sprint_and_role(&app, &sprint_id, &role.name).await?;

    if skip_inactive && tasks.is_empty() && runs.is_empty() {
        log::info!(
            "generate_agent_retro_review: スキップ (未稼働ロール) role={} sprint={}",
            role.name,
            sprint_id
        );
        return Ok(vec![]);
    }

    let notes = crate::db::get_project_notes_by_sprint(&app, &project_id, &sprint_id).await?;
    let usage = crate::db::get_llm_usage_summary_by_sprint(&app, &sprint_id).await?;

    let prompt = build_retro_review_prompt(&role, &tasks, &runs, &notes, &usage);
    let system_prompt =
        "あなたはスクラムレトロスペクティブを自動化する熟練のスクラムマスターです。必ず JSON のみを返します。";

    let response_text = call_retro_llm(
        &app,
        &project_id,
        &sprint_id,
        system_prompt,
        &prompt,
        transport,
    )
    .await?;

    let review_items = parse_retro_review_items(&response_text);

    let mut persisted = Vec::with_capacity(review_items.len());
    let base_sort_order = crate::db::get_retro_items(app.clone(), retro_session_id.clone())
        .await
        .unwrap_or_default()
        .len() as i32;
    for (index, item) in review_items.into_iter().enumerate() {
        let category = normalize_retro_category(&item.category);
        let content = item.content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        let created = crate::db::add_retro_item(
            app.clone(),
            retro_session_id.clone(),
            category,
            content,
            "agent".to_string(),
            Some(role_id.clone()),
            Some(base_sort_order + index as i32),
        )
        .await?;
        persisted.push(created);
    }

    Ok(persisted)
}

#[tauri::command]
pub async fn synthesize_retro_kpt(
    app: AppHandle,
    project_id: String,
    sprint_id: String,
    retro_session_id: String,
) -> Result<String, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let session = crate::db::get_retro_session(app.clone(), retro_session_id.clone())
        .await?
        .ok_or_else(|| format!("retro_session が見つかりません: {retro_session_id}"))?;
    if session.sprint_id != sprint_id {
        return Err("retro_session の sprint_id が一致しません".to_string());
    }

    let items = crate::db::get_retro_items(app.clone(), retro_session_id.clone()).await?;
    let usage = crate::db::get_llm_usage_summary_by_sprint(&app, &sprint_id).await?;

    let mut role_lookup: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for item in items.iter().filter(|i| i.source != "sm") {
        if let Some(role_id) = item.source_role_id.as_deref() {
            if !role_lookup.contains_key(role_id) {
                if let Ok(Some(role)) = crate::db::get_team_role_by_id(&app, role_id).await {
                    role_lookup.insert(role_id.to_string(), role.name);
                }
            }
        }
    }

    if let Err(e) = crate::db::delete_retro_items_by_source(&app, &retro_session_id, "sm").await {
        log::warn!("SM アイテムの事前削除に失敗しました (続行): {e}");
    }

    let prompt = build_retro_kpt_synthesis_prompt(&items, &role_lookup, &usage);
    let system_prompt =
        "あなたはスクラムマスター (SM) として、チーム全体の KPT を統合し Markdown サマリと統合 KPT を JSON で返します。";

    let response_text = call_retro_llm(
        &app,
        &project_id,
        &sprint_id,
        system_prompt,
        &prompt,
        transport,
    )
    .await?;

    let parsed = parse_retro_synthesis_response(&response_text);
    let summary_markdown = parsed.summary_markdown.trim().to_string();

    let base_sort_order = items.len() as i32;
    for (index, item) in parsed.items.into_iter().enumerate() {
        let category = normalize_retro_category(&item.category);
        let content = item.content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        let _ = crate::db::add_retro_item(
            app.clone(),
            retro_session_id.clone(),
            category,
            content,
            "sm".to_string(),
            None,
            Some(base_sort_order + index as i32),
        )
        .await?;
    }

    crate::db::update_retro_session(
        app.clone(),
        retro_session_id.clone(),
        "completed".to_string(),
        Some(summary_markdown.clone()),
    )
    .await?;

    Ok(summary_markdown)
}

#[cfg(test)]
mod tests {
    use super::{
        build_retro_kpt_synthesis_prompt, build_retro_review_prompt, normalize_retro_category,
        parse_retro_review_items, parse_retro_synthesis_response,
    };
    use crate::db::{AgentRetroRun, ProjectNote, RetroItem, SprintLlmUsageSummary, Task, TeamRole};
    use std::collections::HashMap;

    fn make_role(name: &str) -> TeamRole {
        TeamRole {
            id: format!("role-{name}"),
            name: name.to_string(),
            system_prompt: "あなたはテスト用ロールです。".to_string(),
            cli_type: "claude".to_string(),
            model: "claude-haiku-4-5".to_string(),
            avatar_image: None,
            sort_order: 0,
        }
    }

    fn make_run(idx: usize, reasoning: &str) -> AgentRetroRun {
        AgentRetroRun {
            id: format!("run-{idx}"),
            project_id: "project-1".to_string(),
            task_id: Some(format!("task-{idx}")),
            sprint_id: Some("sprint-1".to_string()),
            source_kind: "task_execution".to_string(),
            role_name: "Lead Engineer".to_string(),
            cli_type: "claude".to_string(),
            model: "claude-haiku-4-5".to_string(),
            started_at: 1_000 + idx as i64,
            completed_at: 2_000 + idx as i64,
            duration_ms: 1_000,
            success: true,
            error_message: None,
            reasoning_log: Some(reasoning.to_string()),
            final_answer: Some(format!("final answer {idx}")),
            changed_files_json: Some("[\"src/a.rs\",\"src/b.rs\"]".to_string()),
            tool_event_count: 2,
            created_at: "2026-04-15 00:00:00".to_string(),
        }
    }

    fn empty_usage() -> SprintLlmUsageSummary {
        SprintLlmUsageSummary {
            total_events: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
            failure_count: 0,
        }
    }

    #[test]
    fn retro_review_prompt_includes_role_and_tasks() {
        let role = make_role("Lead Engineer");
        let tasks = vec![Task {
            id: "task-1".to_string(),
            project_id: "project-1".to_string(),
            story_id: "story-1".to_string(),
            sequence_number: 1,
            title: "通知APIの設計".to_string(),
            description: Some("pub/sub の概要".to_string()),
            status: "Done".to_string(),
            sprint_id: Some("sprint-1".to_string()),
            archived: false,
            assignee_type: None,
            assigned_role_id: Some("role-Lead Engineer".to_string()),
            created_at: "2026-04-15 00:00:00".to_string(),
            updated_at: "2026-04-15 00:00:00".to_string(),
            priority: 3,
        }];
        let prompt = build_retro_review_prompt(&role, &tasks, &[], &[], &empty_usage());
        assert!(prompt.contains("Lead Engineer"));
        assert!(prompt.contains("通知APIの設計"));
        assert!(prompt.contains("JSON 配列のみ"));
        assert!(prompt.contains("(実行ログなし)"));
    }

    #[test]
    fn retro_review_prompt_truncates_long_reasoning_log() {
        let role = make_role("Lead Engineer");
        let huge = "あ".repeat(100_000);
        let runs: Vec<AgentRetroRun> = (0..3).map(|i| make_run(i, &huge)).collect();
        let prompt = build_retro_review_prompt(&role, &[], &runs, &[], &empty_usage());
        assert!(prompt.chars().count() < 25_000);
        assert!(prompt.contains("truncated") || prompt.contains("末尾"));
    }

    #[test]
    fn retro_review_prompt_limits_to_last_n_runs() {
        let role = make_role("Lead Engineer");
        let runs: Vec<AgentRetroRun> = (0..30).map(|i| make_run(i, "short")).collect();
        let prompt = build_retro_review_prompt(&role, &[], &runs, &[], &empty_usage());
        assert!(prompt.contains("最新 10/30 件"));
    }

    #[test]
    fn retro_review_prompt_includes_notes_section() {
        let role = make_role("Lead Engineer");
        let notes = vec![ProjectNote {
            id: "note-1".to_string(),
            project_id: "project-1".to_string(),
            sprint_id: Some("sprint-1".to_string()),
            title: "レビュー観点".to_string(),
            content: "境界条件を重点確認する".to_string(),
            source: "user".to_string(),
            created_at: "2026-04-15 00:00:00".to_string(),
            updated_at: "2026-04-15 00:00:00".to_string(),
        }];
        let prompt = build_retro_review_prompt(&role, &[], &[], &notes, &empty_usage());
        assert!(prompt.contains("レビュー観点"));
        assert!(prompt.contains("境界条件"));
    }

    #[test]
    fn parse_retro_review_items_valid_json() {
        let raw = "[{\"category\":\"keep\",\"content\":\"速度が良かった\"},{\"category\":\"problem\",\"content\":\"タスク見積が甘い\"}]";
        let items = parse_retro_review_items(raw);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category, "keep");
        assert!(items[1].content.contains("見積"));
    }

    #[test]
    fn parse_retro_review_items_falls_back_on_garbage() {
        let items = parse_retro_review_items("not json at all");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, "problem");
        assert!(items[0].content.contains("自動分類に失敗"));
    }

    #[test]
    fn parse_retro_review_items_with_surrounding_noise() {
        let raw =
            "以下の通りです:\n[{\"category\":\"try\",\"content\":\"CI並列化を試す\"}]\n以上です";
        let items = parse_retro_review_items(raw);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].category, "try");
    }

    #[test]
    fn parse_retro_synthesis_response_valid() {
        let raw = "{\"summary_markdown\":\"# Sprint\\n良かった\",\"items\":[{\"category\":\"keep\",\"content\":\"ペア作業\"}]}";
        let parsed = parse_retro_synthesis_response(raw);
        assert!(parsed.summary_markdown.contains("Sprint"));
        assert_eq!(parsed.items.len(), 1);
    }

    #[test]
    fn parse_retro_synthesis_response_missing_items_field() {
        let raw = "{\"summary_markdown\":\"サマリのみ\"}";
        let parsed = parse_retro_synthesis_response(raw);
        assert_eq!(parsed.summary_markdown, "サマリのみ");
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn parse_retro_synthesis_response_falls_back_on_plain_text() {
        let parsed = parse_retro_synthesis_response("ただのテキスト出力");
        assert!(parsed.summary_markdown.contains("ただのテキスト"));
        assert!(parsed.items.is_empty());
    }

    #[test]
    fn normalize_retro_category_coerces_unknown_to_problem() {
        assert_eq!(normalize_retro_category("Keep"), "keep");
        assert_eq!(normalize_retro_category("PROBLEM"), "problem");
        assert_eq!(normalize_retro_category("try"), "try");
        assert_eq!(normalize_retro_category("insight"), "problem");
        assert_eq!(normalize_retro_category(""), "problem");
    }

    #[test]
    fn retro_kpt_synthesis_prompt_groups_items_by_category() {
        let items = vec![
            RetroItem {
                id: "i1".to_string(),
                retro_session_id: "s1".to_string(),
                category: "keep".to_string(),
                content: "ペア作業が有効".to_string(),
                source: "agent".to_string(),
                source_role_id: Some("role-Lead Engineer".to_string()),
                is_approved: false,
                sort_order: 0,
                created_at: "2026-04-15 00:00:00".to_string(),
            },
            RetroItem {
                id: "i2".to_string(),
                retro_session_id: "s1".to_string(),
                category: "problem".to_string(),
                content: "見積精度".to_string(),
                source: "user".to_string(),
                source_role_id: None,
                is_approved: false,
                sort_order: 1,
                created_at: "2026-04-15 00:00:00".to_string(),
            },
        ];
        let mut role_lookup = HashMap::new();
        role_lookup.insert(
            "role-Lead Engineer".to_string(),
            "Lead Engineer".to_string(),
        );
        let prompt = build_retro_kpt_synthesis_prompt(&items, &role_lookup, &empty_usage());
        assert!(prompt.contains("### Keep（良かったこと）"));
        assert!(prompt.contains("ペア作業が有効"));
        assert!(prompt.contains("### Problem（課題・問題）"));
        assert!(prompt.contains("見積精度"));
        assert!(prompt.contains("summary_markdown"));
        assert!(prompt.contains("Lead Engineer"));
    }
}
