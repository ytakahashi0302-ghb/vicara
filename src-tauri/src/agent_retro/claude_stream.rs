use serde_json::Value as JsonValue;

use super::{summarize_json_value, truncate_chars, CaptureMutation, TOOL_SUMMARY_CHAR_LIMIT};

#[derive(Debug, Clone, Default)]
pub(super) struct ClaudeStreamJsonParser {
    line_buffer: String,
}

impl ClaudeStreamJsonParser {
    pub(super) fn ingest_chunk(&mut self, output: &str) -> Vec<CaptureMutation> {
        let mut mutations = Vec::new();
        self.line_buffer.push_str(&output.replace("\r\n", "\n"));

        while let Some(newline_index) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..newline_index].to_string();
            self.line_buffer.drain(..=newline_index);
            mutations.extend(parse_stream_line(&line));
        }

        mutations
    }

    pub(super) fn finish(&mut self) -> Vec<CaptureMutation> {
        if self.line_buffer.trim().is_empty() {
            return Vec::new();
        }

        let remaining = self.line_buffer.clone();
        self.line_buffer.clear();
        parse_stream_line(&remaining)
    }
}

fn parse_stream_line(line: &str) -> Vec<CaptureMutation> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let Ok(root) = serde_json::from_str::<JsonValue>(trimmed) else {
        return vec![CaptureMutation::AppendReasoning(format!("{line}\n"))];
    };

    match root.get("type").and_then(JsonValue::as_str) {
        Some("stream_event") => parse_stream_event(&root),
        Some("assistant") => parse_assistant_message(&root),
        Some("user") => parse_user_message(&root),
        _ => Vec::new(),
    }
}

fn parse_stream_event(root: &JsonValue) -> Vec<CaptureMutation> {
    let Some(event) = root.get("event") else {
        return Vec::new();
    };

    match event.get("type").and_then(JsonValue::as_str) {
        Some("content_block_start") => parse_content_block_start(event),
        Some("content_block_delta") => parse_content_block_delta(event),
        _ => Vec::new(),
    }
}

fn parse_content_block_start(event: &JsonValue) -> Vec<CaptureMutation> {
    let Some(content_block) = event.get("content_block") else {
        return Vec::new();
    };
    if content_block.get("type").and_then(JsonValue::as_str) != Some("tool_use") {
        return Vec::new();
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

    vec![CaptureMutation::UpsertToolEvent {
        tool_name,
        status: "requested".to_string(),
        summary,
        tool_use_id: tool_id,
    }]
}

fn parse_content_block_delta(event: &JsonValue) -> Vec<CaptureMutation> {
    let Some(delta) = event.get("delta") else {
        return Vec::new();
    };

    match delta.get("type").and_then(JsonValue::as_str) {
        Some("thinking_delta") => delta
            .get("thinking")
            .and_then(JsonValue::as_str)
            .map(|thinking| vec![CaptureMutation::AppendReasoning(thinking.to_string())])
            .unwrap_or_default(),
        Some("text_delta") => delta
            .get("text")
            .and_then(JsonValue::as_str)
            .map(|text| vec![CaptureMutation::AppendFallbackAnswer(text.to_string())])
            .unwrap_or_default(),
        Some("input_json_delta") => {
            let partial_json = delta
                .get("partial_json")
                .and_then(JsonValue::as_str)
                .unwrap_or("");
            if partial_json.is_empty() {
                return Vec::new();
            }

            vec![CaptureMutation::MergeLastToolSummary(truncate_chars(
                partial_json.trim(),
                TOOL_SUMMARY_CHAR_LIMIT,
            ))]
        }
        _ => Vec::new(),
    }
}

fn parse_assistant_message(root: &JsonValue) -> Vec<CaptureMutation> {
    let Some(message) = root.get("message") else {
        return Vec::new();
    };
    let Some(content) = message.get("content").and_then(JsonValue::as_array) else {
        return Vec::new();
    };

    let mut mutations = Vec::new();
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
                mutations.push(CaptureMutation::UpsertToolEvent {
                    tool_name,
                    status: "requested".to_string(),
                    summary,
                    tool_use_id: tool_id,
                });
            }
            _ => {}
        }
    }

    if !answer.trim().is_empty() {
        mutations.push(CaptureMutation::SetFinalAnswer(answer));
    }

    mutations
}

fn parse_user_message(root: &JsonValue) -> Vec<CaptureMutation> {
    let Some(message) = root.get("message") else {
        return Vec::new();
    };
    let Some(content) = message.get("content").and_then(JsonValue::as_array) else {
        return Vec::new();
    };

    let mut mutations = Vec::new();
    for block in content {
        if block.get("type").and_then(JsonValue::as_str) != Some("tool_result") {
            continue;
        }

        let Some(tool_use_id) = block.get("tool_use_id").and_then(JsonValue::as_str) else {
            continue;
        };

        let result_summary = block
            .get("content")
            .and_then(JsonValue::as_str)
            .map(|text| truncate_chars(text.trim(), TOOL_SUMMARY_CHAR_LIMIT))
            .unwrap_or_default();

        mutations.push(CaptureMutation::ResolveToolResult {
            tool_use_id: tool_use_id.to_string(),
            is_error: block.get("is_error").and_then(JsonValue::as_bool) == Some(true),
            result_summary,
        });
    }

    mutations
}
