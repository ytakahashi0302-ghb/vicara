use crate::{cli_runner::CliType, db};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tauri::AppHandle;

const REASONING_LOG_CHAR_LIMIT: usize = 32_768;
const FINAL_ANSWER_CHAR_LIMIT: usize = 16_384;
const TOOL_SUMMARY_CHAR_LIMIT: usize = 512;

#[derive(Debug, Clone)]
pub struct AgentRetroToolEvent {
    pub sequence_number: i32,
    pub tool_name: String,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub struct FinalizedAgentRetroCapture {
    pub reasoning_log: Option<String>,
    pub final_answer: Option<String>,
    pub tool_events: Vec<AgentRetroToolEvent>,
}

#[derive(Debug, Clone)]
pub struct AgentRetroPersistInput {
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub sprint_id: Option<String>,
    pub source_kind: String,
    pub role_name: String,
    pub cli_type: String,
    pub model: String,
    pub started_at: i64,
    pub completed_at: i64,
    pub success: bool,
    pub error_message: Option<String>,
    pub changed_files: Vec<String>,
    pub finalized: FinalizedAgentRetroCapture,
}

#[derive(Debug, Clone)]
pub struct AgentRetroCapture {
    cli_type: CliType,
    line_buffer: String,
    reasoning_log: String,
    final_answer_fallback: String,
    final_answer: String,
    tool_events: Vec<AgentRetroToolEvent>,
    tool_event_lookup: HashMap<String, usize>,
    next_tool_sequence: i32,
}

impl AgentRetroCapture {
    pub fn new(cli_type: CliType) -> Self {
        Self {
            cli_type,
            line_buffer: String::new(),
            reasoning_log: String::new(),
            final_answer_fallback: String::new(),
            final_answer: String::new(),
            tool_events: Vec::new(),
            tool_event_lookup: HashMap::new(),
            next_tool_sequence: 1,
        }
    }

    pub fn ingest_chunk(&mut self, output: &str) {
        match self.cli_type {
            CliType::Claude => self.ingest_claude_stream_json(output),
            CliType::Gemini => self.ingest_plain_text(output, true),
            CliType::Codex => self.ingest_plain_text(output, false),
        }
    }

    pub fn finalize(
        &mut self,
        final_answer_override: Option<String>,
    ) -> FinalizedAgentRetroCapture {
        if matches!(self.cli_type, CliType::Claude) && !self.line_buffer.trim().is_empty() {
            let remaining = self.line_buffer.clone();
            self.line_buffer.clear();
            self.handle_claude_line(&remaining);
        }

        let final_answer = final_answer_override
            .and_then(|text| normalize_text(&text))
            .or_else(|| normalize_text(&self.final_answer))
            .or_else(|| normalize_text(&self.final_answer_fallback))
            .or_else(|| {
                if matches!(self.cli_type, CliType::Gemini) {
                    normalize_text(&self.reasoning_log)
                } else {
                    None
                }
            });

        FinalizedAgentRetroCapture {
            reasoning_log: normalize_text(&self.reasoning_log),
            final_answer,
            tool_events: self.tool_events.clone(),
        }
    }

    fn ingest_plain_text(&mut self, output: &str, mirror_to_final_answer: bool) {
        let normalized = output.replace("\r\n", "\n");
        append_with_limit(
            &mut self.reasoning_log,
            &normalized,
            REASONING_LOG_CHAR_LIMIT,
        );

        if mirror_to_final_answer {
            append_with_limit(
                &mut self.final_answer_fallback,
                &normalized,
                FINAL_ANSWER_CHAR_LIMIT,
            );
        }
    }

    fn ingest_claude_stream_json(&mut self, output: &str) {
        self.line_buffer.push_str(&output.replace("\r\n", "\n"));

        while let Some(newline_index) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..newline_index].to_string();
            self.line_buffer.drain(..=newline_index);
            self.handle_claude_line(&line);
        }
    }

    fn handle_claude_line(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }

        let Ok(root) = serde_json::from_str::<JsonValue>(trimmed) else {
            self.ingest_plain_text(line, false);
            append_with_limit(&mut self.reasoning_log, "\n", REASONING_LOG_CHAR_LIMIT);
            return;
        };

        match root.get("type").and_then(JsonValue::as_str) {
            Some("stream_event") => self.handle_claude_stream_event(&root),
            Some("assistant") => self.handle_claude_assistant_message(&root),
            Some("user") => self.handle_claude_user_message(&root),
            _ => {}
        }
    }

    fn handle_claude_stream_event(&mut self, root: &JsonValue) {
        let Some(event) = root.get("event") else {
            return;
        };

        match event.get("type").and_then(JsonValue::as_str) {
            Some("content_block_start") => {
                let Some(content_block) = event.get("content_block") else {
                    return;
                };
                if content_block.get("type").and_then(JsonValue::as_str) != Some("tool_use") {
                    return;
                }

                let tool_name = content_block
                    .get("name")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("tool")
                    .to_string();
                let summary = summarize_json_value(content_block.get("input"));
                let tool_id = content_block
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
                self.push_tool_event(tool_name, "requested".to_string(), summary, tool_id);
            }
            Some("content_block_delta") => {
                let Some(delta) = event.get("delta") else {
                    return;
                };
                match delta.get("type").and_then(JsonValue::as_str) {
                    Some("thinking_delta") => {
                        if let Some(thinking) = delta.get("thinking").and_then(JsonValue::as_str) {
                            append_with_limit(
                                &mut self.reasoning_log,
                                thinking,
                                REASONING_LOG_CHAR_LIMIT,
                            );
                        }
                    }
                    Some("text_delta") => {
                        if let Some(text) = delta.get("text").and_then(JsonValue::as_str) {
                            append_with_limit(
                                &mut self.final_answer_fallback,
                                text,
                                FINAL_ANSWER_CHAR_LIMIT,
                            );
                        }
                    }
                    Some("input_json_delta") => {
                        let partial_json = delta
                            .get("partial_json")
                            .and_then(JsonValue::as_str)
                            .unwrap_or("");
                        if partial_json.is_empty() {
                            return;
                        }

                        let summary = truncate_chars(partial_json.trim(), TOOL_SUMMARY_CHAR_LIMIT);
                        if let Some(index) = self.tool_events.len().checked_sub(1) {
                            if !summary.is_empty() {
                                self.tool_events[index].summary = merge_tool_summaries(
                                    &self.tool_events[index].summary,
                                    &summary,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn handle_claude_assistant_message(&mut self, root: &JsonValue) {
        let Some(message) = root.get("message") else {
            return;
        };
        let Some(content) = message.get("content").and_then(JsonValue::as_array) else {
            return;
        };

        let mut answer = String::new();
        for block in content {
            match block.get("type").and_then(JsonValue::as_str) {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(JsonValue::as_str) {
                        answer.push_str(text);
                    }
                }
                Some("tool_use") => {
                    let tool_name = block
                        .get("name")
                        .and_then(JsonValue::as_str)
                        .unwrap_or("tool")
                        .to_string();
                    let summary = summarize_json_value(block.get("input"));
                    let tool_id = block
                        .get("id")
                        .and_then(JsonValue::as_str)
                        .map(str::to_string);
                    self.push_tool_event(tool_name, "requested".to_string(), summary, tool_id);
                }
                _ => {}
            }
        }

        if !answer.trim().is_empty() {
            self.final_answer = truncate_chars(answer.trim(), FINAL_ANSWER_CHAR_LIMIT);
        }
    }

    fn handle_claude_user_message(&mut self, root: &JsonValue) {
        let Some(message) = root.get("message") else {
            return;
        };
        let Some(content) = message.get("content").and_then(JsonValue::as_array) else {
            return;
        };

        for block in content {
            if block.get("type").and_then(JsonValue::as_str) != Some("tool_result") {
                continue;
            }

            let Some(tool_use_id) = block.get("tool_use_id").and_then(JsonValue::as_str) else {
                continue;
            };
            let Some(index) = self.tool_event_lookup.get(tool_use_id).copied() else {
                continue;
            };

            self.tool_events[index].status =
                if block.get("is_error").and_then(JsonValue::as_bool) == Some(true) {
                    "failed".to_string()
                } else {
                    "completed".to_string()
                };

            let result_summary = block
                .get("content")
                .and_then(JsonValue::as_str)
                .map(|text| truncate_chars(text.trim(), TOOL_SUMMARY_CHAR_LIMIT))
                .unwrap_or_default();
            if !result_summary.is_empty() {
                self.tool_events[index].summary =
                    merge_tool_summaries(&self.tool_events[index].summary, &result_summary);
            }
        }
    }

    fn push_tool_event(
        &mut self,
        tool_name: String,
        status: String,
        summary: String,
        tool_use_id: Option<String>,
    ) {
        if let Some(existing_index) = tool_use_id
            .as_ref()
            .and_then(|id| self.tool_event_lookup.get(id).copied())
        {
            self.tool_events[existing_index].status = status;
            if !summary.is_empty() {
                self.tool_events[existing_index].summary =
                    merge_tool_summaries(&self.tool_events[existing_index].summary, &summary);
            }
            return;
        }

        let event = AgentRetroToolEvent {
            sequence_number: self.next_tool_sequence,
            tool_name,
            status,
            summary,
        };
        self.next_tool_sequence += 1;
        self.tool_events.push(event);

        if let Some(tool_use_id) = tool_use_id {
            self.tool_event_lookup
                .insert(tool_use_id, self.tool_events.len() - 1);
        }
    }
}

pub async fn persist_agent_retro_run(
    app: &AppHandle,
    input: AgentRetroPersistInput,
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
            "Skipping agent retro run persistence because no project_id could be resolved (task_id={:?})",
            input.task_id
        );
        return Ok(());
    };

    let run_id = uuid::Uuid::new_v4().to_string();
    let tool_events = input.finalized.tool_events.clone();

    db::insert_agent_retro_run(
        app,
        db::AgentRetroRunInsertInput {
            id: run_id.clone(),
            project_id,
            task_id: input.task_id,
            sprint_id: input.sprint_id,
            source_kind: input.source_kind,
            role_name: input.role_name,
            cli_type: input.cli_type,
            model: input.model,
            started_at: input.started_at,
            completed_at: input.completed_at,
            duration_ms: (input.completed_at - input.started_at).max(0),
            success: input.success,
            error_message: input.error_message.and_then(|text| normalize_text(&text)),
            reasoning_log: input.finalized.reasoning_log,
            final_answer: input.finalized.final_answer,
            changed_files_json: if input.changed_files.is_empty() {
                None
            } else {
                Some(serde_json::to_string(&input.changed_files).map_err(|e| e.to_string())?)
            },
            tool_event_count: tool_events.len() as i32,
        },
    )
    .await?;

    for tool_event in tool_events {
        db::insert_agent_retro_tool_event(
            app,
            db::AgentRetroToolEventInsertInput {
                id: uuid::Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                sequence_number: tool_event.sequence_number,
                tool_name: tool_event.tool_name,
                status: tool_event.status,
                summary: normalize_text(&tool_event.summary),
            },
        )
        .await?;
    }

    Ok(())
}

fn normalize_text(value: &str) -> Option<String> {
    let normalized = value.replace("\r\n", "\n").trim().to_string();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn append_with_limit(target: &mut String, fragment: &str, limit: usize) {
    if fragment.is_empty() || limit == 0 {
        return;
    }

    let current_len = target.chars().count();
    if current_len >= limit {
        return;
    }

    let remaining = limit - current_len;
    target.push_str(&truncate_chars(fragment, remaining));
}

fn truncate_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn summarize_json_value(value: Option<&JsonValue>) -> String {
    let Some(value) = value else {
        return String::new();
    };

    let summary = if let Some(command) = value.get("command").and_then(JsonValue::as_str) {
        format!("command: {}", command)
    } else if let Some(path) = value.get("file_path").and_then(JsonValue::as_str) {
        format!("file: {}", path)
    } else if let Some(pattern) = value.get("pattern").and_then(JsonValue::as_str) {
        format!("pattern: {}", pattern)
    } else if let Some(text) = value.as_str() {
        text.to_string()
    } else {
        value.to_string()
    };

    truncate_chars(summary.trim(), TOOL_SUMMARY_CHAR_LIMIT)
}

fn merge_tool_summaries(existing: &str, addition: &str) -> String {
    if addition.is_empty() {
        return existing.to_string();
    }
    if existing.is_empty() {
        return truncate_chars(addition, TOOL_SUMMARY_CHAR_LIMIT);
    }

    truncate_chars(
        &format!("{} | {}", existing.trim(), addition.trim()),
        TOOL_SUMMARY_CHAR_LIMIT,
    )
}

#[cfg(test)]
mod tests {
    use super::{AgentRetroCapture, CliType};

    #[test]
    fn claude_capture_extracts_thinking_answer_and_tool_use() {
        let mut capture = AgentRetroCapture::new(CliType::Claude);
        capture.ingest_chunk(
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"まず状況を確認します。\"}}}\n",
        );
        capture.ingest_chunk(
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_start\",\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"Bash\",\"input\":{\"command\":\"npm test\"}}}}\n",
        );
        capture.ingest_chunk(
            "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"toolu_1\",\"content\":\"ok\",\"is_error\":false}]}}\n",
        );
        capture.ingest_chunk(
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"修正しました。\"}]}}\n",
        );

        let finalized = capture.finalize(None);

        assert_eq!(
            finalized.reasoning_log.as_deref(),
            Some("まず状況を確認します。")
        );
        assert_eq!(finalized.final_answer.as_deref(), Some("修正しました。"));
        assert_eq!(finalized.tool_events.len(), 1);
        assert_eq!(finalized.tool_events[0].tool_name, "Bash");
        assert_eq!(finalized.tool_events[0].status, "completed");
    }

    #[test]
    fn gemini_capture_falls_back_to_plain_text_for_answer() {
        let mut capture = AgentRetroCapture::new(CliType::Gemini);
        capture.ingest_chunk("調査しています...\n");
        capture.ingest_chunk("最終回答です。\n");

        let finalized = capture.finalize(None);

        assert_eq!(
            finalized.reasoning_log.as_deref(),
            Some("調査しています...\n最終回答です。")
        );
        assert_eq!(
            finalized.final_answer.as_deref(),
            Some("調査しています...\n最終回答です。")
        );
    }
}
