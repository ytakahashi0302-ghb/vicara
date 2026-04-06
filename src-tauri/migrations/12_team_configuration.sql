CREATE TABLE IF NOT EXISTS team_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    max_concurrent_agents INTEGER NOT NULL DEFAULT 1
        CHECK (max_concurrent_agents BETWEEN 1 AND 5),
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO team_settings (id, max_concurrent_agents)
VALUES (1, 1);

CREATE TABLE IF NOT EXISTS team_roles (
    id TEXT PRIMARY KEY,
    team_settings_id INTEGER NOT NULL DEFAULT 1,
    name TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    model TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(team_settings_id) REFERENCES team_settings(id) ON DELETE CASCADE,
    UNIQUE(team_settings_id, sort_order)
);

INSERT OR IGNORE INTO team_roles (
    id,
    team_settings_id,
    name,
    system_prompt,
    model,
    sort_order
) VALUES (
    'seed-lead-engineer',
    1,
    'Lead Engineer',
    'あなたは優秀なリードエンジニアです。プロジェクト全体の技術方針を踏まえ、実装方針の整理、重要な設計判断、品質レビュー観点の提示を担当してください。',
    'claude-3-5-sonnet-20241022',
    0
);
