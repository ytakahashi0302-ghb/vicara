use crate::db::{self, QueryResult};
use rig::completion::Usage as RigUsage;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use tauri::{AppHandle, Emitter};

const MEASUREMENT_CAPTURED: &str = "captured";
const MEASUREMENT_ESTIMATED: &str = "estimated";
const MEASUREMENT_UNAVAILABLE: &str = "unavailable";
const TRANSPORT_CLAUDE_CLI: &str = "claude_cli";
const TRANSPORT_GEMINI_CLI: &str = "gemini_cli";
const TRANSPORT_CODEX_CLI: &str = "codex_cli";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct NormalizedUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub cached_input_tokens: i64,
}

impl NormalizedUsage {
    pub fn unavailable() -> Self {
        Self::default()
    }

    pub fn has_usage(&self) -> bool {
        self.input_tokens > 0
            || self.output_tokens > 0
            || self.total_tokens > 0
            || self.cached_input_tokens > 0
    }
}

impl From<RigUsage> for NormalizedUsage {
    fn from(value: RigUsage) -> Self {
        Self {
            input_tokens: value.input_tokens as i64,
            output_tokens: value.output_tokens as i64,
            total_tokens: value.total_tokens as i64,
            cached_input_tokens: value.cached_input_tokens as i64,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PricingSnapshot {
    pub input_cost_per_million: f64,
    pub output_cost_per_million: f64,
    pub cache_creation_cost_per_million: f64,
    pub cache_read_cost_per_million: f64,
}

impl PricingSnapshot {
    pub fn zero() -> Self {
        Self {
            input_cost_per_million: 0.0,
            output_cost_per_million: 0.0,
            cache_creation_cost_per_million: 0.0,
            cache_read_cost_per_million: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordLlmUsageInput {
    pub project_id: String,
    pub task_id: Option<String>,
    pub sprint_id: Option<String>,
    pub source_kind: String,
    pub transport_kind: String,
    pub provider: String,
    pub model: String,
    pub usage: NormalizedUsage,
    pub measurement_status: Option<String>,
    pub request_started_at: Option<i64>,
    pub request_completed_at: Option<i64>,
    pub success: bool,
    pub error_message: Option<String>,
    pub raw_usage_json: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCliUsageRecordInput {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub sprint_id: Option<String>,
    pub source_kind: String,
    pub cli_type: String,
    pub model: String,
    pub request_started_at: i64,
    pub request_completed_at: i64,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LlmUsageUpdatedPayload {
    project_id: String,
    task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageAggregate {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: f64,
    pub event_count: i64,
    pub unavailable_event_count: i64,
}

impl Default for LlmUsageAggregate {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
            total_tokens: 0,
            estimated_cost_usd: 0.0,
            event_count: 0,
            unavailable_event_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
struct LlmUsageAggregateRow {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_creation_input_tokens: Option<i64>,
    cache_read_input_tokens: Option<i64>,
    total_tokens: Option<i64>,
    estimated_cost_usd: Option<f64>,
    event_count: Option<i64>,
    unavailable_event_count: Option<i64>,
}

impl From<LlmUsageAggregateRow> for LlmUsageAggregate {
    fn from(value: LlmUsageAggregateRow) -> Self {
        Self {
            input_tokens: value.input_tokens.unwrap_or(0),
            output_tokens: value.output_tokens.unwrap_or(0),
            cache_creation_input_tokens: value.cache_creation_input_tokens.unwrap_or(0),
            cache_read_input_tokens: value.cache_read_input_tokens.unwrap_or(0),
            total_tokens: value.total_tokens.unwrap_or(0),
            estimated_cost_usd: value.estimated_cost_usd.unwrap_or(0.0),
            event_count: value.event_count.unwrap_or(0),
            unavailable_event_count: value.unavailable_event_count.unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LlmUsageSourceBreakdown {
    pub source_kind: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: f64,
    pub event_count: i64,
    pub unavailable_event_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LlmUsageModelBreakdown {
    pub provider: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_usd: f64,
    pub event_count: i64,
    pub unavailable_event_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLlmUsageSummary {
    pub project_id: String,
    pub active_sprint_id: Option<String>,
    pub project_totals: LlmUsageAggregate,
    pub active_sprint_totals: LlmUsageAggregate,
    pub today_totals: LlmUsageAggregate,
    pub by_source: Vec<LlmUsageSourceBreakdown>,
    pub by_model: Vec<LlmUsageModelBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLlmUsageSummary {
    pub task_id: String,
    pub project_id: Option<String>,
    pub totals: LlmUsageAggregate,
    pub last_request_completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskLlmUsageListItem {
    pub task_id: String,
    pub task_title: String,
    pub total_tokens: i64,
    pub estimated_cost_usd: f64,
    pub event_count: i64,
    pub unavailable_event_count: i64,
    pub last_request_completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
struct TaskSummaryRow {
    project_id: Option<String>,
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_creation_input_tokens: Option<i64>,
    cache_read_input_tokens: Option<i64>,
    total_tokens: Option<i64>,
    estimated_cost_usd: Option<f64>,
    event_count: Option<i64>,
    unavailable_event_count: Option<i64>,
    last_request_completed_at: Option<i64>,
}

pub fn calculate_estimated_cost(
    usage: NormalizedUsage,
    pricing: PricingSnapshot,
) -> (f64, PricingSnapshot) {
    let cached_reads = usage.cached_input_tokens.max(0);
    let base_inputs = (usage.input_tokens - cached_reads).max(0);

    let input_cost = (base_inputs as f64 / 1_000_000.0) * pricing.input_cost_per_million.max(0.0);
    let output_cost = (usage.output_tokens.max(0) as f64 / 1_000_000.0)
        * pricing.output_cost_per_million.max(0.0);
    let cache_read_cost =
        (cached_reads as f64 / 1_000_000.0) * pricing.cache_read_cost_per_million.max(0.0);

    (input_cost + output_cost + cache_read_cost, pricing)
}

pub fn resolve_pricing(provider: &str, model: &str) -> PricingSnapshot {
    let provider = provider.to_lowercase();
    let model = model.to_lowercase();

    if provider.contains("ollama") {
        return PricingSnapshot::zero();
    }

    if provider.contains("anthropic") || provider.contains("claude") || model.contains("claude") {
        if model.contains("opus") {
            return PricingSnapshot {
                input_cost_per_million: 15.0,
                output_cost_per_million: 75.0,
                cache_creation_cost_per_million: 18.75,
                cache_read_cost_per_million: 1.50,
            };
        }

        if model.contains("sonnet") {
            return PricingSnapshot {
                input_cost_per_million: 3.0,
                output_cost_per_million: 15.0,
                cache_creation_cost_per_million: 3.75,
                cache_read_cost_per_million: 0.30,
            };
        }

        if model.contains("haiku") {
            return PricingSnapshot {
                input_cost_per_million: 0.80,
                output_cost_per_million: 4.0,
                cache_creation_cost_per_million: 1.0,
                cache_read_cost_per_million: 0.08,
            };
        }
    }

    if provider.contains("gemini") || model.contains("gemini") {
        if model.contains("2.5-flash-lite") {
            return PricingSnapshot {
                input_cost_per_million: 0.10,
                output_cost_per_million: 0.40,
                cache_creation_cost_per_million: 0.01,
                cache_read_cost_per_million: 0.0,
            };
        }

        if model.contains("2.5-flash") {
            return PricingSnapshot {
                input_cost_per_million: 0.30,
                output_cost_per_million: 2.50,
                cache_creation_cost_per_million: 0.025,
                cache_read_cost_per_million: 0.0,
            };
        }

        if model.contains("2.0-flash") {
            return PricingSnapshot {
                input_cost_per_million: 0.10,
                output_cost_per_million: 0.40,
                cache_creation_cost_per_million: 0.025,
                cache_read_cost_per_million: 0.0,
            };
        }
    }

    if provider.contains("openai") {
        if model.starts_with("gpt-5.2") {
            return PricingSnapshot {
                input_cost_per_million: 1.75,
                output_cost_per_million: 14.0,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.175,
            };
        }

        if model.starts_with("gpt-5.1") || model == "gpt-5" || model.starts_with("gpt-5-chat") {
            return PricingSnapshot {
                input_cost_per_million: 1.25,
                output_cost_per_million: 10.0,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.125,
            };
        }

        if model.starts_with("gpt-5-mini") {
            return PricingSnapshot {
                input_cost_per_million: 0.25,
                output_cost_per_million: 2.0,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.025,
            };
        }

        if model.starts_with("gpt-5-nano") {
            return PricingSnapshot {
                input_cost_per_million: 0.05,
                output_cost_per_million: 0.40,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.005,
            };
        }

        if model.starts_with("gpt-4.1-mini") {
            return PricingSnapshot {
                input_cost_per_million: 0.40,
                output_cost_per_million: 1.60,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.10,
            };
        }

        if model.starts_with("gpt-4.1-nano") {
            return PricingSnapshot {
                input_cost_per_million: 0.10,
                output_cost_per_million: 0.40,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.025,
            };
        }

        if model.starts_with("gpt-4.1") {
            return PricingSnapshot {
                input_cost_per_million: 2.0,
                output_cost_per_million: 8.0,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.50,
            };
        }

        if model.starts_with("gpt-4o-mini") {
            return PricingSnapshot {
                input_cost_per_million: 0.15,
                output_cost_per_million: 0.60,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.075,
            };
        }

        if model.starts_with("gpt-4o-2024-05-13") {
            return PricingSnapshot {
                input_cost_per_million: 5.0,
                output_cost_per_million: 15.0,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.0,
            };
        }

        if model.starts_with("gpt-4o") {
            return PricingSnapshot {
                input_cost_per_million: 2.50,
                output_cost_per_million: 10.0,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 1.25,
            };
        }

        if model.starts_with("o4-mini") {
            return PricingSnapshot {
                input_cost_per_million: 1.10,
                output_cost_per_million: 4.40,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.275,
            };
        }

        if model == "o3" || model.starts_with("o3-") {
            return PricingSnapshot {
                input_cost_per_million: 2.0,
                output_cost_per_million: 8.0,
                cache_creation_cost_per_million: 0.0,
                cache_read_cost_per_million: 0.50,
            };
        }
    }

    PricingSnapshot::zero()
}

fn determine_measurement_status(
    requested: Option<&str>,
    usage: NormalizedUsage,
    transport_kind: &str,
) -> String {
    if let Some(status) = requested {
        return status.to_string();
    }

    if usage.has_usage() {
        MEASUREMENT_CAPTURED.to_string()
    } else if transport_kind.ends_with("_cli") {
        MEASUREMENT_UNAVAILABLE.to_string()
    } else {
        MEASUREMENT_ESTIMATED.to_string()
    }
}

fn normalize_cli_transport_kind(cli_type: &str, model: &str) -> &'static str {
    let normalized_cli_type = cli_type.trim().to_ascii_lowercase();
    let normalized_model = model.trim().to_ascii_lowercase();

    if normalized_cli_type == "gemini" || normalized_model.contains("gemini") {
        TRANSPORT_GEMINI_CLI
    } else if normalized_cli_type == "codex"
        || normalized_model == "o3"
        || normalized_model.starts_with("o4")
        || normalized_model.starts_with("gpt-")
    {
        TRANSPORT_CODEX_CLI
    } else {
        TRANSPORT_CLAUDE_CLI
    }
}

async fn resolve_sprint_id(
    app: &AppHandle,
    project_id: &str,
    requested_sprint_id: Option<String>,
) -> Result<Option<String>, String> {
    if requested_sprint_id.is_some() {
        return Ok(requested_sprint_id);
    }

    let query = "SELECT id FROM sprints WHERE project_id = ? AND status = 'Active' LIMIT 1";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    let result = db::select_query::<(String,)>(app, query, values).await?;
    Ok(result.first().map(|row| row.0.clone()))
}

pub async fn record_llm_usage(
    app: &AppHandle,
    input: RecordLlmUsageInput,
) -> Result<QueryResult, String> {
    let measurement_status = determine_measurement_status(
        input.measurement_status.as_deref(),
        input.usage,
        &input.transport_kind,
    );
    let pricing = resolve_pricing(&input.provider, &input.model);
    let (estimated_cost_usd, pricing_snapshot) = calculate_estimated_cost(input.usage, pricing);
    let sprint_id = resolve_sprint_id(app, &input.project_id, input.sprint_id.clone()).await?;
    let latency_ms = match (input.request_started_at, input.request_completed_at) {
        (Some(start), Some(end)) if end >= start => Some(end - start),
        _ => None,
    };

    let query = r#"
        INSERT INTO llm_usage_events (
            id,
            project_id,
            task_id,
            sprint_id,
            source_kind,
            transport_kind,
            provider,
            model,
            input_tokens,
            output_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            total_tokens,
            estimated_cost_usd,
            input_cost_per_million,
            output_cost_per_million,
            cache_creation_cost_per_million,
            cache_read_cost_per_million,
            measurement_status,
            request_started_at,
            request_completed_at,
            latency_ms,
            success,
            error_message,
            raw_usage_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    "#;

    let values = vec![
        serde_json::to_value(uuid::Uuid::new_v4().to_string()).unwrap(),
        serde_json::to_value(&input.project_id).unwrap(),
        serde_json::to_value(&input.task_id).unwrap(),
        serde_json::to_value(&sprint_id).unwrap(),
        serde_json::to_value(&input.source_kind).unwrap(),
        serde_json::to_value(&input.transport_kind).unwrap(),
        serde_json::to_value(&input.provider).unwrap(),
        serde_json::to_value(&input.model).unwrap(),
        serde_json::to_value(input.usage.input_tokens).unwrap(),
        serde_json::to_value(input.usage.output_tokens).unwrap(),
        serde_json::to_value(0_i64).unwrap(),
        serde_json::to_value(input.usage.cached_input_tokens).unwrap(),
        serde_json::to_value(input.usage.total_tokens).unwrap(),
        serde_json::to_value(estimated_cost_usd).unwrap(),
        serde_json::to_value(pricing_snapshot.input_cost_per_million).unwrap(),
        serde_json::to_value(pricing_snapshot.output_cost_per_million).unwrap(),
        serde_json::to_value(pricing_snapshot.cache_creation_cost_per_million).unwrap(),
        serde_json::to_value(pricing_snapshot.cache_read_cost_per_million).unwrap(),
        serde_json::to_value(&measurement_status).unwrap(),
        serde_json::to_value(input.request_started_at).unwrap(),
        serde_json::to_value(input.request_completed_at).unwrap(),
        serde_json::to_value(latency_ms).unwrap(),
        serde_json::to_value(if input.success { 1_i64 } else { 0_i64 }).unwrap(),
        serde_json::to_value(&input.error_message).unwrap(),
        serde_json::to_value(input.raw_usage_json.map(|value| value.to_string())).unwrap(),
    ];

    let result = db::execute_query(app, query, values).await?;

    let _ = app.emit(
        "llm_usage_updated",
        LlmUsageUpdatedPayload {
            project_id: input.project_id,
            task_id: input.task_id,
        },
    );

    Ok(result)
}

pub async fn record_claude_cli_usage(
    app: &AppHandle,
    input: ClaudeCliUsageRecordInput,
) -> Result<(), String> {
    let resolved_project_id = if let Some(project_id) = input.project_id.clone() {
        Some(project_id)
    } else if let Some(task_id) = input.task_id.as_deref() {
        db::get_task_by_id(app, task_id)
            .await?
            .map(|task| task.project_id)
    } else {
        None
    };

    let Some(project_id) = resolved_project_id else {
        log::warn!(
            "Skipping Claude CLI usage record because no project_id could be resolved (task_id={:?})",
            input.task_id
        );
        return Ok(());
    };

    let transport_kind = normalize_cli_transport_kind(&input.cli_type, &input.model).to_string();

    record_llm_usage(
        app,
        RecordLlmUsageInput {
            project_id,
            task_id: input.task_id,
            sprint_id: input.sprint_id,
            source_kind: input.source_kind,
            transport_kind: transport_kind.clone(),
            provider: transport_kind,
            model: input.model,
            usage: NormalizedUsage::unavailable(),
            measurement_status: Some(MEASUREMENT_UNAVAILABLE.to_string()),
            request_started_at: Some(input.request_started_at),
            request_completed_at: Some(input.request_completed_at),
            success: input.success,
            error_message: input.error_message,
            raw_usage_json: Some(json!({
                "measurement_status": MEASUREMENT_UNAVAILABLE,
                "reason": "Claude CLI usage is not machine-readable in the current integration"
            })),
        },
    )
    .await?;

    Ok(())
}

fn empty_aggregate_query() -> &'static str {
    r#"
        SELECT
            COALESCE(SUM(input_tokens), 0) AS input_tokens,
            COALESCE(SUM(output_tokens), 0) AS output_tokens,
            COALESCE(SUM(cache_creation_input_tokens), 0) AS cache_creation_input_tokens,
            COALESCE(SUM(cache_read_input_tokens), 0) AS cache_read_input_tokens,
            COALESCE(SUM(total_tokens), 0) AS total_tokens,
            COALESCE(SUM(estimated_cost_usd), 0.0) AS estimated_cost_usd,
            COUNT(*) AS event_count,
            COALESCE(SUM(CASE WHEN measurement_status = 'unavailable' THEN 1 ELSE 0 END), 0) AS unavailable_event_count
        FROM llm_usage_events
    "#
}

async fn fetch_aggregate(
    app: &AppHandle,
    query: &str,
    values: Vec<JsonValue>,
) -> Result<LlmUsageAggregate, String> {
    let rows = db::select_query::<LlmUsageAggregateRow>(app, query, values).await?;
    Ok(rows.into_iter().next().map(Into::into).unwrap_or_default())
}

#[tauri::command]
pub async fn get_project_llm_usage_summary(
    app: AppHandle,
    project_id: String,
) -> Result<ProjectLlmUsageSummary, String> {
    let active_sprint_id = resolve_sprint_id(&app, &project_id, None).await?;
    let project_totals = fetch_aggregate(
        &app,
        &format!("{} WHERE project_id = ?", empty_aggregate_query()),
        vec![serde_json::to_value(&project_id).unwrap()],
    )
    .await?;

    let active_sprint_totals = if let Some(active_sprint_id) = active_sprint_id.clone() {
        fetch_aggregate(
            &app,
            &format!(
                "{} WHERE project_id = ? AND sprint_id = ?",
                empty_aggregate_query()
            ),
            vec![
                serde_json::to_value(&project_id).unwrap(),
                serde_json::to_value(active_sprint_id).unwrap(),
            ],
        )
        .await?
    } else {
        LlmUsageAggregate::default()
    };
    let today_totals = fetch_aggregate(
        &app,
        &format!(
            "{} WHERE project_id = ? AND date(created_at, 'localtime') = date('now', 'localtime')",
            empty_aggregate_query()
        ),
        vec![serde_json::to_value(&project_id).unwrap()],
    )
    .await?;

    let by_source = db::select_query::<LlmUsageSourceBreakdown>(
        &app,
        r#"
            SELECT
                source_kind,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(estimated_cost_usd), 0.0) AS estimated_cost_usd,
                COUNT(*) AS event_count,
                COALESCE(SUM(CASE WHEN measurement_status = 'unavailable' THEN 1 ELSE 0 END), 0) AS unavailable_event_count
            FROM llm_usage_events
            WHERE project_id = ?
            GROUP BY source_kind
            ORDER BY estimated_cost_usd DESC, total_tokens DESC
        "#,
        vec![serde_json::to_value(&project_id).unwrap()],
    )
    .await?;

    let by_model = db::select_query::<LlmUsageModelBreakdown>(
        &app,
        r#"
            SELECT
                provider,
                model,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(estimated_cost_usd), 0.0) AS estimated_cost_usd,
                COUNT(*) AS event_count,
                COALESCE(SUM(CASE WHEN measurement_status = 'unavailable' THEN 1 ELSE 0 END), 0) AS unavailable_event_count
            FROM llm_usage_events
            WHERE project_id = ?
            GROUP BY provider, model
            ORDER BY estimated_cost_usd DESC, total_tokens DESC
        "#,
        vec![serde_json::to_value(&project_id).unwrap()],
    )
    .await?;

    Ok(ProjectLlmUsageSummary {
        project_id,
        active_sprint_id,
        project_totals,
        active_sprint_totals,
        today_totals,
        by_source,
        by_model,
    })
}

#[tauri::command]
pub async fn get_task_llm_usage_summary(
    app: AppHandle,
    task_id: String,
) -> Result<TaskLlmUsageSummary, String> {
    let rows = db::select_query::<TaskSummaryRow>(
        &app,
        r#"
            SELECT
                MAX(project_id) AS project_id,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(cache_creation_input_tokens), 0) AS cache_creation_input_tokens,
                COALESCE(SUM(cache_read_input_tokens), 0) AS cache_read_input_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(estimated_cost_usd), 0.0) AS estimated_cost_usd,
                COUNT(*) AS event_count,
                COALESCE(SUM(CASE WHEN measurement_status = 'unavailable' THEN 1 ELSE 0 END), 0) AS unavailable_event_count,
                MAX(request_completed_at) AS last_request_completed_at
            FROM llm_usage_events
            WHERE task_id = ?
        "#,
        vec![serde_json::to_value(&task_id).unwrap()],
    )
    .await?;

    let row = rows.into_iter().next().unwrap_or(TaskSummaryRow {
        project_id: None,
        input_tokens: Some(0),
        output_tokens: Some(0),
        cache_creation_input_tokens: Some(0),
        cache_read_input_tokens: Some(0),
        total_tokens: Some(0),
        estimated_cost_usd: Some(0.0),
        event_count: Some(0),
        unavailable_event_count: Some(0),
        last_request_completed_at: None,
    });

    Ok(TaskLlmUsageSummary {
        task_id,
        project_id: row.project_id,
        totals: LlmUsageAggregate {
            input_tokens: row.input_tokens.unwrap_or(0),
            output_tokens: row.output_tokens.unwrap_or(0),
            cache_creation_input_tokens: row.cache_creation_input_tokens.unwrap_or(0),
            cache_read_input_tokens: row.cache_read_input_tokens.unwrap_or(0),
            total_tokens: row.total_tokens.unwrap_or(0),
            estimated_cost_usd: row.estimated_cost_usd.unwrap_or(0.0),
            event_count: row.event_count.unwrap_or(0),
            unavailable_event_count: row.unavailable_event_count.unwrap_or(0),
        },
        last_request_completed_at: row.last_request_completed_at,
    })
}

#[tauri::command]
pub async fn list_project_task_llm_usage(
    app: AppHandle,
    project_id: String,
) -> Result<Vec<TaskLlmUsageListItem>, String> {
    db::select_query::<TaskLlmUsageListItem>(
        &app,
        r#"
            SELECT
                e.task_id AS task_id,
                t.title AS task_title,
                COALESCE(SUM(e.total_tokens), 0) AS total_tokens,
                COALESCE(SUM(e.estimated_cost_usd), 0.0) AS estimated_cost_usd,
                COUNT(*) AS event_count,
                COALESCE(SUM(CASE WHEN e.measurement_status = 'unavailable' THEN 1 ELSE 0 END), 0) AS unavailable_event_count,
                MAX(e.request_completed_at) AS last_request_completed_at
            FROM llm_usage_events e
            JOIN tasks t ON t.id = e.task_id
            WHERE e.project_id = ? AND e.task_id IS NOT NULL
            GROUP BY e.task_id, t.title
            ORDER BY estimated_cost_usd DESC, total_tokens DESC, last_request_completed_at DESC
        "#,
        vec![serde_json::to_value(&project_id).unwrap()],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_for_anthropic_sonnet_is_resolved() {
        let pricing = resolve_pricing("anthropic", "claude-3-5-sonnet-20241022");
        assert_eq!(pricing.input_cost_per_million, 3.0);
        assert_eq!(pricing.output_cost_per_million, 15.0);
        assert_eq!(pricing.cache_read_cost_per_million, 0.30);
    }

    #[test]
    fn pricing_for_gemini_flash_is_resolved() {
        let pricing = resolve_pricing("gemini", "gemini-2.0-flash");
        assert_eq!(pricing.input_cost_per_million, 0.10);
        assert_eq!(pricing.output_cost_per_million, 0.40);
    }

    #[test]
    fn pricing_for_openai_gpt_4o_is_resolved() {
        let pricing = resolve_pricing("openai", "gpt-4o");
        assert_eq!(pricing.input_cost_per_million, 2.50);
        assert_eq!(pricing.output_cost_per_million, 10.0);
        assert_eq!(pricing.cache_read_cost_per_million, 1.25);
    }

    #[test]
    fn ollama_is_always_treated_as_zero_cost() {
        let pricing = resolve_pricing("ollama", "llama3.2");
        assert_eq!(pricing.input_cost_per_million, 0.0);
        assert_eq!(pricing.output_cost_per_million, 0.0);
        assert_eq!(pricing.cache_read_cost_per_million, 0.0);
    }

    #[test]
    fn cost_calculation_separates_cached_reads() {
        let usage = NormalizedUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            total_tokens: 1_500_000,
            cached_input_tokens: 200_000,
        };
        let pricing = PricingSnapshot {
            input_cost_per_million: 3.0,
            output_cost_per_million: 15.0,
            cache_creation_cost_per_million: 3.75,
            cache_read_cost_per_million: 0.30,
        };

        let (cost, _) = calculate_estimated_cost(usage, pricing);
        let expected = (800_000.0 / 1_000_000.0) * 3.0
            + (500_000.0 / 1_000_000.0) * 15.0
            + (200_000.0 / 1_000_000.0) * 0.30;
        assert!((cost - expected).abs() < 0.000_001);
    }

    #[test]
    fn unavailable_measurement_is_selected_for_zero_cli_usage() {
        let status =
            determine_measurement_status(None, NormalizedUsage::default(), TRANSPORT_CLAUDE_CLI);
        assert_eq!(status, MEASUREMENT_UNAVAILABLE);
    }
}
