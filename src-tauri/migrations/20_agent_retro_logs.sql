CREATE TABLE IF NOT EXISTS agent_retro_runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    sprint_id TEXT REFERENCES sprints(id) ON DELETE SET NULL,
    source_kind TEXT NOT NULL,
    role_name TEXT NOT NULL,
    cli_type TEXT NOT NULL,
    model TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL DEFAULT 1,
    error_message TEXT,
    reasoning_log TEXT,
    final_answer TEXT,
    changed_files_json TEXT,
    tool_event_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS agent_retro_tool_events (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES agent_retro_runs(id) ON DELETE CASCADE,
    sequence_number INTEGER NOT NULL,
    tool_name TEXT NOT NULL,
    status TEXT NOT NULL,
    summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_agent_retro_runs_project_id_created_at
    ON agent_retro_runs(project_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_retro_runs_task_id_created_at
    ON agent_retro_runs(task_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_retro_runs_sprint_id_created_at
    ON agent_retro_runs(sprint_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_retro_runs_cli_type_created_at
    ON agent_retro_runs(cli_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_retro_tool_events_run_id_sequence
    ON agent_retro_tool_events(run_id, sequence_number);
