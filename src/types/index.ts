export interface Project {
    id: string;
    name: string;
    description: string | null;
    local_path: string | null;
    created_at: string;
    updated_at: string;
}

export interface Sprint {
    id: string;
    started_at: number;
    completed_at: number;
    duration_ms: number;
    project_id: string;
}

export interface Story {
    id: string;
    title: string;
    description: string | null;
    acceptance_criteria: string | null;
    status: 'Backlog' | 'Ready' | 'In Progress' | 'Done';
    sprint_id?: string | null;
    project_id: string;
    archived: boolean;
    created_at: string;
    updated_at: string;
}

export interface Task {
    id: string;
    story_id: string;
    title: string;
    description: string | null;
    status: 'To Do' | 'In Progress' | 'Done';
    sprint_id?: string | null;
    project_id: string;
    archived: boolean;
    created_at: string;
    updated_at: string;
}
