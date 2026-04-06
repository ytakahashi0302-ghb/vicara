ALTER TABLE tasks
ADD COLUMN assigned_role_id TEXT REFERENCES team_roles(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_tasks_assigned_role_id
ON tasks(assigned_role_id);
