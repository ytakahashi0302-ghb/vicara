-- Review ステータス追加と worktrees テーブル新設
PRAGMA foreign_keys = OFF;

CREATE TABLE tasks_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL DEFAULT 'default' REFERENCES projects(id) ON DELETE CASCADE,
    story_id TEXT NOT NULL REFERENCES stories(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL CHECK(status IN ('To Do', 'In Progress', 'Review', 'Done')),
    sprint_id TEXT REFERENCES sprints(id) ON DELETE SET NULL,
    priority INTEGER NOT NULL DEFAULT 3,
    archived BOOLEAN DEFAULT FALSE,
    assignee_type TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    assigned_role_id TEXT REFERENCES team_roles(id) ON DELETE SET NULL
);

INSERT INTO tasks_new (
    id,
    project_id,
    story_id,
    title,
    description,
    status,
    sprint_id,
    priority,
    archived,
    assignee_type,
    created_at,
    updated_at,
    assigned_role_id
)
SELECT
    id,
    project_id,
    story_id,
    title,
    description,
    status,
    sprint_id,
    priority,
    archived,
    assignee_type,
    created_at,
    updated_at,
    assigned_role_id
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

CREATE INDEX IF NOT EXISTS idx_tasks_sprint_id ON tasks(sprint_id);
CREATE INDEX IF NOT EXISTS idx_tasks_assigned_role_id ON tasks(assigned_role_id);

CREATE TABLE IF NOT EXISTS worktrees (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    worktree_path TEXT NOT NULL,
    branch_name TEXT NOT NULL,
    preview_port INTEGER,
    preview_pid INTEGER,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK(status IN ('active', 'merging', 'merged', 'conflict', 'removed')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_worktrees_task_id ON worktrees(task_id);
CREATE INDEX IF NOT EXISTS idx_worktrees_project_id ON worktrees(project_id);
CREATE INDEX IF NOT EXISTS idx_worktrees_status ON worktrees(status);

PRAGMA foreign_keys = ON;
