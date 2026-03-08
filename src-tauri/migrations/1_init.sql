-- ユーザーストーリー（親）を管理するテーブル
CREATE TABLE stories (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    acceptance_criteria TEXT, -- 受け入れ条件
    status TEXT DEFAULT 'Ready', -- 'Backlog', 'Ready', 'In Progress', 'Done'
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 技術タスク（子）を管理するテーブル
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT DEFAULT 'To Do', -- 'To Do', 'In Progress', 'Done'
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(story_id) REFERENCES stories(id) ON DELETE CASCADE
);
