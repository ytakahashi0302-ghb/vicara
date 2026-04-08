export type TaskStatus = 'To Do' | 'In Progress' | 'Review' | 'Done';

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
    project_id: string;
    status: 'Planned' | 'Active' | 'Completed';
    started_at: number | null;
    completed_at: number | null;
    duration_ms: number | null;
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
    priority: number;
}

export interface Task {
    id: string;
    story_id: string;
    title: string;
    description: string | null;
    status: TaskStatus;
    sprint_id?: string | null;
    project_id: string;
    archived: boolean;
    assignee_type?: string | null;
    assigned_role_id?: string | null;
    created_at: string;
    updated_at: string;
    priority: number;
}

export interface WorktreeRecord {
    id: string;
    task_id: string;
    project_id: string;
    worktree_path: string;
    branch_name: string;
    preview_port: number | null;
    preview_pid: number | null;
    status: 'active' | 'merging' | 'merged' | 'conflict' | 'removed';
    created_at: string;
    updated_at: string;
}

export interface TaskDependency {
    task_id: string;
    blocked_by_task_id: string;
}

export interface TeamChatMessage {
    id: string;
    project_id: string;
    role: 'user' | 'assistant' | 'model';
    content: string;
    created_at: string;
}

export interface TeamRoleSetting {
    id: string;
    name: string;
    system_prompt: string;
    model: string;
    avatar_image?: string | null;
    sort_order: number;
}

export interface TeamConfiguration {
    max_concurrent_agents: number;
    roles: TeamRoleSetting[];
}

export interface LlmUsageAggregate {
    input_tokens: number;
    output_tokens: number;
    cache_creation_input_tokens: number;
    cache_read_input_tokens: number;
    total_tokens: number;
    estimated_cost_usd: number;
    event_count: number;
    unavailable_event_count: number;
}

export interface LlmUsageSourceBreakdown {
    source_kind: string;
    input_tokens: number;
    output_tokens: number;
    total_tokens: number;
    estimated_cost_usd: number;
    event_count: number;
    unavailable_event_count: number;
}

export interface LlmUsageModelBreakdown {
    provider: string;
    model: string;
    input_tokens: number;
    output_tokens: number;
    total_tokens: number;
    estimated_cost_usd: number;
    event_count: number;
    unavailable_event_count: number;
}

export interface ProjectLlmUsageSummary {
    project_id: string;
    active_sprint_id: string | null;
    project_totals: LlmUsageAggregate;
    active_sprint_totals: LlmUsageAggregate;
    today_totals: LlmUsageAggregate;
    by_source: LlmUsageSourceBreakdown[];
    by_model: LlmUsageModelBreakdown[];
}
