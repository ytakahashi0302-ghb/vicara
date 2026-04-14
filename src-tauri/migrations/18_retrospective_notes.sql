CREATE TABLE retro_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sprint_id TEXT NOT NULL REFERENCES sprints(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','in_progress','completed')),
    summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_retro_sessions_project_sprint ON retro_sessions(project_id, sprint_id);

CREATE TABLE retro_items (
    id TEXT PRIMARY KEY,
    retro_session_id TEXT NOT NULL REFERENCES retro_sessions(id) ON DELETE CASCADE,
    category TEXT NOT NULL CHECK (category IN ('keep','problem','try')),
    content TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('agent','po','sm','user')),
    source_role_id TEXT,
    is_approved INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_retro_items_session ON retro_items(retro_session_id);

CREATE TABLE retro_rules (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    retro_item_id TEXT REFERENCES retro_items(id) ON DELETE SET NULL,
    sprint_id TEXT REFERENCES sprints(id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_retro_rules_project_active ON retro_rules(project_id, is_active);

CREATE TABLE project_notes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sprint_id TEXT REFERENCES sprints(id) ON DELETE SET NULL,
    title TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT 'user' CHECK (source IN ('user','po_assistant')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_project_notes_project ON project_notes(project_id, created_at DESC);

ALTER TABLE tasks ADD COLUMN execution_log_summary TEXT;
