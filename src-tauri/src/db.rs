use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{sqlite::SqliteRow, SqlitePool};
use std::collections::HashSet;
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};

// Define shared types
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub rows_affected: u64,
    pub last_insert_id: i64,
}

const VALID_TASK_STATUSES: &[&str] = &["To Do", "In Progress", "Review", "Done"];
const VALID_RETRO_SESSION_STATUSES: &[&str] = &["draft", "in_progress", "completed"];
const VALID_RETRO_ITEM_CATEGORIES: &[&str] = &["keep", "problem", "try"];
const VALID_RETRO_ITEM_SOURCES: &[&str] = &["agent", "po", "sm", "user"];
const VALID_PROJECT_NOTE_SOURCES: &[&str] = &["user", "po_assistant"];
#[allow(dead_code)]
const VALID_WORKTREE_STATUSES: &[&str] = &["active", "merging", "merged", "conflict", "removed"];

fn validate_task_status(status: &str) -> Result<(), String> {
    if VALID_TASK_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(format!(
            "status には {} のいずれかを指定してください。",
            VALID_TASK_STATUSES.join(", ")
        ))
    }
}

fn validate_retro_session_status(status: &str) -> Result<(), String> {
    if VALID_RETRO_SESSION_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(format!(
            "retro session status には {} のいずれかを指定してください。",
            VALID_RETRO_SESSION_STATUSES.join(", ")
        ))
    }
}

fn validate_retro_item_category(category: &str) -> Result<(), String> {
    if VALID_RETRO_ITEM_CATEGORIES.contains(&category) {
        Ok(())
    } else {
        Err(format!(
            "retro item category には {} のいずれかを指定してください。",
            VALID_RETRO_ITEM_CATEGORIES.join(", ")
        ))
    }
}

fn validate_retro_item_source(source: &str) -> Result<(), String> {
    if VALID_RETRO_ITEM_SOURCES.contains(&source) {
        Ok(())
    } else {
        Err(format!(
            "retro item source には {} のいずれかを指定してください。",
            VALID_RETRO_ITEM_SOURCES.join(", ")
        ))
    }
}

fn validate_project_note_source(source: &str) -> Result<(), String> {
    if VALID_PROJECT_NOTE_SOURCES.contains(&source) {
        Ok(())
    } else {
        Err(format!(
            "project note source には {} のいずれかを指定してください。",
            VALID_PROJECT_NOTE_SOURCES.join(", ")
        ))
    }
}

#[allow(dead_code)]
fn validate_worktree_status(status: &str) -> Result<(), String> {
    if VALID_WORKTREE_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(format!(
            "worktree status には {} のいずれかを指定してください。",
            VALID_WORKTREE_STATUSES.join(", ")
        ))
    }
}

// Model types
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub local_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Story {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub status: String,
    pub sprint_id: Option<String>,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
    pub priority: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub story_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub sprint_id: Option<String>,
    pub archived: bool,
    pub assignee_type: Option<String>,
    pub assigned_role_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub priority: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct TaskDependency {
    pub task_id: String,
    pub blocked_by_task_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct WorktreeRecord {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub branch_name: String,
    pub preview_port: Option<i32>,
    pub preview_pid: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct WorktreeUpsertInput {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub branch_name: String,
    pub preview_port: Option<i32>,
    pub preview_pid: Option<i64>,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct Sprint {
    pub id: String,
    pub project_id: String,
    pub status: String,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct RetroSession {
    pub id: String,
    pub project_id: String,
    pub sprint_id: String,
    pub status: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct RetroItem {
    pub id: String,
    pub retro_session_id: String,
    pub category: String,
    pub content: String,
    pub source: String,
    pub source_role_id: Option<String>,
    pub is_approved: bool,
    pub sort_order: i32,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct RetroRule {
    pub id: String,
    pub project_id: String,
    pub retro_item_id: Option<String>,
    pub sprint_id: Option<String>,
    pub content: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct ProjectNote {
    pub id: String,
    pub project_id: String,
    pub sprint_id: Option<String>,
    pub title: String,
    pub content: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct TeamChatMessage {
    pub id: String,
    pub project_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct TeamSettings {
    pub max_concurrent_agents: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::FromRow)]
pub struct TeamRole {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub cli_type: String,
    pub model: String,
    pub avatar_image: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamConfiguration {
    pub max_concurrent_agents: i32,
    pub roles: Vec<TeamRole>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamRoleInput {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub cli_type: String,
    pub model: String,
    pub avatar_image: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamConfigurationInput {
    pub max_concurrent_agents: i32,
    pub roles: Vec<TeamRoleInput>,
}

#[derive(Clone, Copy)]
struct DefaultTeamRoleSeed {
    id: &'static str,
    name: &'static str,
    system_prompt: &'static str,
    cli_type: &'static str,
    model: &'static str,
    avatar_image: &'static str,
    sort_order: i32,
}

const DEFAULT_TEAM_ROLE_SEEDS: [DefaultTeamRoleSeed; 5] = [
    DefaultTeamRoleSeed {
        id: "seed-lead-engineer",
        name: "Lead Engineer",
        system_prompt: "あなたはVicaraのLead Engineerです。プロジェクト全体の技術方針を踏まえ、実装方針の整理、設計判断、レビュー観点の提示、各ロールの成果統合を担当してください。重要なトレードオフを明確にし、チーム全体の品質と速度の両立を導いてください。",
        cli_type: "claude",
        model: "claude-haiku-4-5",
        avatar_image: "/avatars/dev-agent-1.png",
        sort_order: 0,
    },
    DefaultTeamRoleSeed {
        id: "seed-security-system-architect",
        name: "Security & System Architect",
        system_prompt: "あなたはSecurity & System Architectです。複雑なシステム設計、コアビジネスロジックの構築、脅威分析、厳密なセキュリティレビューを担当してください。境界条件、認可、データフロー、将来の拡張性まで踏み込み、深い論理で設計上の妥当性を確認してください。",
        cli_type: "claude",
        model: "claude-haiku-4-5",
        avatar_image: "/avatars/dev-agent-2.png",
        sort_order: 1,
    },
    DefaultTeamRoleSeed {
        id: "seed-ui-ux-multimedia-specialist",
        name: "UI/UX Designer & Multimedia Specialist",
        system_prompt: "あなたはUI/UX Designer & Multimedia Specialistです。Webフロントエンドの実装、UIプロトタイプ生成、視覚表現の改善、プロモーション素材作成、マルチメディア体験の検討、ユーザー向け導線の最適化を担当してください。使いやすさと印象の強さを両立させてください。",
        cli_type: "gemini",
        model: "gemini-3-flash-preview",
        avatar_image: "/avatars/dev-agent-3.png",
        sort_order: 2,
    },
    DefaultTeamRoleSeed {
        id: "seed-qa-engineer",
        name: "QA Engineer",
        system_prompt: "あなたはQA Engineerです。テスト計画の策定、エッジケースの洗い出し、回帰リスクの特定、E2Eを含むテスト実装と実行確認を担当してください。失敗時は再現条件と観測結果を明確に整理し、品質向上に必要な具体策まで提示してください。",
        cli_type: "claude",
        model: "claude-haiku-4-5",
        avatar_image: "/avatars/dev-agent-4.png",
        sort_order: 3,
    },
    DefaultTeamRoleSeed {
        id: "seed-pmo-document-manager",
        name: "PMO & Document Manager",
        system_prompt: "あなたはPMO & Document Managerです。プロジェクト推進、仕様整合性の確認、論点整理、進捗管理、議事録や設計メモやタスクリストなどのドキュメント品質維持を担当してください。意思決定に必要な情報を簡潔かつ漏れなく整理してください。",
        cli_type: "gemini",
        model: "gemini-3-flash-preview",
        avatar_image: "/avatars/dev-agent-5.png",
        sort_order: 4,
    },
];

fn default_team_role_avatar_for_sort_order(sort_order: i32) -> Option<&'static str> {
    DEFAULT_TEAM_ROLE_SEEDS
        .iter()
        .find(|seed| seed.sort_order == sort_order)
        .map(|seed| seed.avatar_image)
}

const DB_STRING: &str = "sqlite:vicara.db";

pub async fn execute_query(
    app: &AppHandle,
    query: &str,
    values: Vec<JsonValue>,
) -> Result<QueryResult, String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances
        .get(DB_STRING)
        .ok_or("Database instance not found")?;

    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut q = sqlx::query(query);
    for v in values {
        if let Some(s) = v.as_str() {
            q = q.bind(s.to_string());
        } else if let Some(n) = v.as_i64() {
            q = q.bind(n);
        } else if v.is_null() {
            q = q.bind(Option::<String>::None);
        } else {
            q = q.bind(v.to_string());
        }
    }

    match q.execute(pool).await {
        Ok(result) => Ok(QueryResult {
            rows_affected: result.rows_affected(),
            last_insert_id: result.last_insert_rowid(),
        }),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn select_query<T>(
    app: &AppHandle,
    query: &str,
    values: Vec<JsonValue>,
) -> Result<Vec<T>, String>
where
    T: for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin,
{
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances
        .get(DB_STRING)
        .ok_or("Database instance not found")?;

    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut q = sqlx::query_as::<_, T>(query);
    for v in values {
        if let Some(s) = v.as_str() {
            q = q.bind(s.to_string());
        } else if let Some(n) = v.as_i64() {
            q = q.bind(n);
        } else if v.is_null() {
            q = q.bind(Option::<String>::None);
        } else {
            q = q.bind(v.to_string());
        }
    }

    match q.fetch_all(pool).await {
        Ok(result) => Ok(result),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn get_task_by_id(app: &AppHandle, task_id: &str) -> Result<Option<Task>, String> {
    let query = "SELECT * FROM tasks WHERE id = ? LIMIT 1";
    let values = vec![serde_json::to_value(task_id).unwrap()];
    let mut tasks = select_query::<Task>(app, query, values).await?;
    Ok(tasks.pop())
}

#[allow(dead_code)]
pub async fn get_worktree_by_task_id(
    app: &AppHandle,
    task_id: &str,
) -> Result<Option<WorktreeRecord>, String> {
    let query = r#"
        SELECT *
        FROM worktrees
        WHERE task_id = ?
        ORDER BY updated_at DESC, created_at DESC
        LIMIT 1
    "#;
    let values = vec![serde_json::to_value(task_id).unwrap()];
    let mut worktrees = select_query::<WorktreeRecord>(app, query, values).await?;
    Ok(worktrees.pop())
}

#[allow(dead_code)]
pub async fn get_worktree_record(
    app: AppHandle,
    task_id: String,
) -> Result<Option<WorktreeRecord>, String> {
    get_worktree_by_task_id(&app, &task_id).await
}

#[allow(dead_code)]
pub async fn list_worktrees(app: &AppHandle) -> Result<Vec<WorktreeRecord>, String> {
    let query = "SELECT * FROM worktrees ORDER BY created_at ASC";
    select_query::<WorktreeRecord>(app, query, vec![]).await
}

#[allow(dead_code)]
pub async fn list_worktrees_by_project_id(
    app: &AppHandle,
    project_id: &str,
) -> Result<Vec<WorktreeRecord>, String> {
    let query = "SELECT * FROM worktrees WHERE project_id = ? ORDER BY created_at ASC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<WorktreeRecord>(app, query, values).await
}

#[allow(dead_code)]
pub async fn upsert_worktree_record(
    app: &AppHandle,
    input: WorktreeUpsertInput,
) -> Result<QueryResult, String> {
    validate_worktree_status(&input.status)?;

    let query = r#"
        INSERT INTO worktrees (
            id,
            task_id,
            project_id,
            worktree_path,
            branch_name,
            preview_port,
            preview_pid,
            status
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            task_id = excluded.task_id,
            project_id = excluded.project_id,
            worktree_path = excluded.worktree_path,
            branch_name = excluded.branch_name,
            preview_port = excluded.preview_port,
            preview_pid = excluded.preview_pid,
            status = excluded.status,
            updated_at = CURRENT_TIMESTAMP
    "#;
    let values = vec![
        serde_json::to_value(input.id).unwrap(),
        serde_json::to_value(input.task_id).unwrap(),
        serde_json::to_value(input.project_id).unwrap(),
        serde_json::to_value(input.worktree_path).unwrap(),
        serde_json::to_value(input.branch_name).unwrap(),
        serde_json::to_value(input.preview_port).unwrap(),
        serde_json::to_value(input.preview_pid).unwrap(),
        serde_json::to_value(input.status).unwrap(),
    ];
    execute_query(app, query, values).await
}

#[allow(dead_code)]
pub async fn update_worktree_record_state(
    app: &AppHandle,
    task_id: &str,
    preview_port: Option<i32>,
    preview_pid: Option<i64>,
    status: &str,
) -> Result<QueryResult, String> {
    validate_worktree_status(status)?;

    let query = r#"
        UPDATE worktrees
        SET preview_port = ?,
            preview_pid = ?,
            status = ?,
            updated_at = CURRENT_TIMESTAMP
        WHERE task_id = ?
    "#;
    let values = vec![
        serde_json::to_value(preview_port).unwrap(),
        serde_json::to_value(preview_pid).unwrap(),
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(task_id).unwrap(),
    ];
    execute_query(app, query, values).await
}

#[allow(dead_code)]
pub async fn delete_worktree_record_by_task_id(
    app: &AppHandle,
    task_id: &str,
) -> Result<QueryResult, String> {
    let query = "DELETE FROM worktrees WHERE task_id = ?";
    let values = vec![serde_json::to_value(task_id).unwrap()];
    execute_query(app, query, values).await
}

pub async fn get_team_role_by_id(
    app: &AppHandle,
    role_id: &str,
) -> Result<Option<TeamRole>, String> {
    let query = r#"
        SELECT id, name, system_prompt, cli_type, model, avatar_image, sort_order
        FROM team_roles
        WHERE id = ?
        LIMIT 1
    "#;
    let values = vec![serde_json::to_value(role_id).unwrap()];
    let mut roles = select_query::<TeamRole>(app, query, values).await?;
    Ok(roles.pop())
}

pub async fn get_max_concurrent_agents_value(app: &AppHandle) -> Result<i32, String> {
    let query = "SELECT max_concurrent_agents FROM team_settings WHERE id = 1 LIMIT 1";
    let mut settings = select_query::<TeamSettings>(app, query, vec![]).await?;
    Ok(settings.pop().map(|s| s.max_concurrent_agents).unwrap_or(5))
}

async fn insert_default_team_role(
    app: &AppHandle,
    seed: DefaultTeamRoleSeed,
) -> Result<(), String> {
    let query = r#"
        INSERT OR IGNORE INTO team_roles (
            id,
            team_settings_id,
            name,
            system_prompt,
            cli_type,
            model,
            avatar_image,
            sort_order,
            updated_at
        ) VALUES (?, 1, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
    "#;
    let values = vec![
        serde_json::to_value(seed.id).unwrap(),
        serde_json::to_value(seed.name).unwrap(),
        serde_json::to_value(seed.system_prompt).unwrap(),
        serde_json::to_value(seed.cli_type).unwrap(),
        serde_json::to_value(seed.model).unwrap(),
        serde_json::to_value(seed.avatar_image).unwrap(),
        serde_json::to_value(seed.sort_order).unwrap(),
    ];

    execute_query(app, query, values).await?;
    Ok(())
}

async fn ensure_default_team_role_avatars(app: &AppHandle) -> Result<(), String> {
    let roles = select_query::<TeamRole>(
        app,
        r#"
        SELECT id, name, system_prompt, cli_type, model, avatar_image, sort_order
        FROM team_roles
        WHERE team_settings_id = 1
        ORDER BY sort_order ASC, created_at ASC
        "#,
        vec![],
    )
    .await?;

    for role in roles {
        let needs_avatar = role
            .avatar_image
            .as_deref()
            .map(str::trim)
            .map(|value| value.is_empty())
            .unwrap_or(true);

        if !needs_avatar {
            continue;
        }

        let Some(default_avatar) = default_team_role_avatar_for_sort_order(role.sort_order) else {
            continue;
        };

        execute_query(
            app,
            r#"
            UPDATE team_roles
            SET avatar_image = ?, updated_at = CURRENT_TIMESTAMP
            WHERE id = ? AND (avatar_image IS NULL OR TRIM(avatar_image) = '')
            "#,
            vec![
                serde_json::to_value(default_avatar).unwrap(),
                serde_json::to_value(role.id).unwrap(),
            ],
        )
        .await?;
    }

    Ok(())
}

pub async fn ensure_default_team_templates(app: &AppHandle) -> Result<(), String> {
    execute_query(
        app,
        r#"
        INSERT OR IGNORE INTO team_settings (id, max_concurrent_agents, updated_at)
        VALUES (1, 5, CURRENT_TIMESTAMP)
        "#,
        vec![],
    )
    .await?;

    let roles_query = r#"
        SELECT id, name, system_prompt, cli_type, model, avatar_image, sort_order
        FROM team_roles
        WHERE team_settings_id = 1
        ORDER BY sort_order ASC, created_at ASC
    "#;
    let roles = select_query::<TeamRole>(app, roles_query, vec![]).await?;

    if roles.is_empty() {
        for seed in DEFAULT_TEAM_ROLE_SEEDS {
            insert_default_team_role(app, seed).await?;
        }
        return Ok(());
    }

    if roles.len() == 1 {
        let existing_role = &roles[0];
        execute_query(
            app,
            "UPDATE team_roles SET sort_order = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            vec![serde_json::to_value(existing_role.id.clone()).unwrap()],
        )
        .await?;

        for seed in DEFAULT_TEAM_ROLE_SEEDS.iter().skip(1).copied() {
            insert_default_team_role(app, seed).await?;
        }
    }

    ensure_default_team_role_avatars(app).await?;

    Ok(())
}

pub async fn get_project_by_local_path(
    app: &AppHandle,
    local_path: &str,
) -> Result<Option<Project>, String> {
    let query = "SELECT * FROM projects WHERE local_path = ? LIMIT 1";
    let values = vec![serde_json::to_value(local_path).unwrap()];
    let mut projects = select_query::<Project>(app, query, values).await?;
    Ok(projects.pop())
}

// ------------------------------------------------------------------------------------------------
// Projects CRUD
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_projects(app: AppHandle) -> Result<Vec<Project>, String> {
    let query = "SELECT * FROM projects ORDER BY created_at ASC";
    select_query::<Project>(&app, query, vec![]).await
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    id: String,
    name: String,
    description: Option<String>,
) -> Result<QueryResult, String> {
    let query = "INSERT INTO projects (id, name, description) VALUES (?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(name).unwrap(),
        serde_json::to_value(description).unwrap(),
    ];
    let result = execute_query(&app, query, values).await?;
    ensure_default_team_templates(&app).await?;
    Ok(result)
}

#[tauri::command]
pub async fn update_project(
    app: AppHandle,
    id: String,
    name: String,
    description: Option<String>,
) -> Result<QueryResult, String> {
    let query = "UPDATE projects SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(name).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn delete_project(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM projects WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectPathUpdateResult {
    pub success: bool,
    pub has_product_context: bool,
    pub has_architecture: bool,
    pub has_rule: bool,
}

#[tauri::command]
pub async fn update_project_path(
    app: AppHandle,
    id: String,
    local_path: Option<String>,
) -> Result<ProjectPathUpdateResult, String> {
    let query = "UPDATE projects SET local_path = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(&local_path).unwrap(),
        serde_json::to_value(&id).unwrap(),
    ];
    execute_query(&app, query, values).await?;

    let mut has_product_context = false;
    let mut has_architecture = false;
    let mut has_rule = false;

    if let Some(path) = local_path {
        let p = std::path::Path::new(&path);
        if p.exists() && p.is_dir() {
            has_product_context = p.join("PRODUCT_CONTEXT.md").exists();
            has_architecture = p.join("ARCHITECTURE.md").exists();
            has_rule = p.join("Rule.md").exists();
        }
    }

    Ok(ProjectPathUpdateResult {
        success: true,
        has_product_context,
        has_architecture,
        has_rule,
    })
}

// ------------------------------------------------------------------------------------------------
// Stories CRUD
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_stories(app: AppHandle, project_id: String) -> Result<Vec<Story>, String> {
    let query =
        "SELECT * FROM stories WHERE archived = 0 AND project_id = ? ORDER BY created_at DESC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    let stories = select_query::<Story>(&app, query, values).await?;
    log::debug!("Fetched stories: {:?}", stories);
    Ok(stories)
}

#[tauri::command]
pub async fn get_archived_stories(
    app: AppHandle,
    project_id: String,
) -> Result<Vec<Story>, String> {
    let query = "SELECT * FROM stories WHERE archived = 1 AND project_id = ?";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<Story>(&app, query, values).await
}

#[tauri::command]
pub async fn add_story(
    app: AppHandle,
    id: String,
    project_id: String,
    title: String,
    description: Option<String>,
    acceptance_criteria: Option<String>,
    status: String,
    priority: Option<i32>,
) -> Result<QueryResult, String> {
    let priority_val = priority.unwrap_or(3);
    let query = "INSERT INTO stories (id, project_id, title, description, acceptance_criteria, status, priority) VALUES (?, ?, ?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(acceptance_criteria).unwrap(),
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(priority_val).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn update_story(
    app: AppHandle,
    id: String,
    title: String,
    description: Option<String>,
    acceptance_criteria: Option<String>,
    status: String,
    priority: Option<i32>,
) -> Result<QueryResult, String> {
    let priority_val = priority.unwrap_or(3);
    let query = "UPDATE stories SET title = ?, description = ?, acceptance_criteria = ?, status = ?, priority = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(acceptance_criteria).unwrap(),
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(priority_val).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn delete_story(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM stories WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

// ------------------------------------------------------------------------------------------------
// Tasks CRUD
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_tasks(app: AppHandle, project_id: String) -> Result<Vec<Task>, String> {
    let query = "SELECT tasks.* FROM tasks JOIN stories ON tasks.story_id = stories.id WHERE stories.archived = 0 AND tasks.project_id = ? ORDER BY tasks.created_at ASC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    let tasks = select_query::<Task>(&app, query, values).await?;
    log::debug!("Fetched tasks: {:?}", tasks);
    Ok(tasks)
}

#[tauri::command]
pub async fn get_archived_tasks(app: AppHandle, project_id: String) -> Result<Vec<Task>, String> {
    let query = "SELECT * FROM tasks WHERE archived = 1 AND project_id = ?";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<Task>(&app, query, values).await
}

#[tauri::command]
pub async fn get_tasks_by_story_id(
    app: AppHandle,
    story_id: String,
    project_id: String,
) -> Result<Vec<Task>, String> {
    let query = "SELECT * FROM tasks WHERE story_id = ? AND archived = 0 AND project_id = ? ORDER BY created_at ASC";
    let values = vec![
        serde_json::to_value(story_id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
    ];
    select_query::<Task>(&app, query, values).await
}

#[tauri::command]
pub async fn add_task(
    app: AppHandle,
    id: String,
    project_id: String,
    story_id: String,
    title: String,
    description: Option<String>,
    status: String,
    assignee_type: Option<String>,
    assigned_role_id: Option<String>,
    priority: Option<i32>,
) -> Result<QueryResult, String> {
    validate_task_status(&status)?;
    let priority_val = priority.unwrap_or(3);
    let query = "INSERT INTO tasks (id, project_id, story_id, title, description, status, assignee_type, assigned_role_id, priority) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
        serde_json::to_value(story_id).unwrap(),
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(assignee_type).unwrap(),
        serde_json::to_value(assigned_role_id).unwrap(),
        serde_json::to_value(priority_val).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn update_task_status(
    app: AppHandle,
    id: String,
    status: String,
) -> Result<QueryResult, String> {
    validate_task_status(&status)?;
    let query = "UPDATE tasks SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn update_task(
    app: AppHandle,
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    assignee_type: Option<String>,
    assigned_role_id: Option<String>,
    priority: Option<i32>,
) -> Result<QueryResult, String> {
    validate_task_status(&status)?;
    let priority_val = priority.unwrap_or(3);
    let query = "UPDATE tasks SET title = ?, description = ?, status = ?, assignee_type = ?, assigned_role_id = ?, priority = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(assignee_type).unwrap(),
        serde_json::to_value(assigned_role_id).unwrap(),
        serde_json::to_value(priority_val).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn delete_task(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM tasks WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

// ------------------------------------------------------------------------------------------------
// Team Chat Messages CRUD (POアシスタント)
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_team_chat_messages(
    app: AppHandle,
    project_id: String,
) -> Result<Vec<TeamChatMessage>, String> {
    let query = "SELECT * FROM team_chat_messages WHERE project_id = ? ORDER BY created_at ASC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<TeamChatMessage>(&app, query, values).await
}

#[tauri::command]
pub async fn add_team_chat_message(
    app: AppHandle,
    id: String,
    project_id: String,
    role: String,
    content: String,
) -> Result<QueryResult, String> {
    let query =
        "INSERT INTO team_chat_messages (id, project_id, role, content) VALUES (?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
        serde_json::to_value(role).unwrap(),
        serde_json::to_value(content).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn clear_team_chat_messages(
    app: AppHandle,
    project_id: String,
) -> Result<QueryResult, String> {
    let query = "DELETE FROM team_chat_messages WHERE project_id = ?";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    execute_query(&app, query, values).await
}

// ------------------------------------------------------------------------------------------------
// Team Configuration CRUD
// ------------------------------------------------------------------------------------------------

fn validate_team_configuration(config: &TeamConfigurationInput) -> Result<(), String> {
    if !(1..=5).contains(&config.max_concurrent_agents) {
        return Err("max_concurrent_agents は 1〜5 の範囲で指定してください".to_string());
    }

    if config.roles.is_empty() {
        return Err("roles は最低 1 件必要です".to_string());
    }

    for (index, role) in config.roles.iter().enumerate() {
        if role.id.trim().is_empty() {
            return Err(format!("roles[{}].id は必須です", index));
        }
        if role.name.trim().is_empty() {
            return Err(format!("roles[{}].name は必須です", index));
        }
        if role.system_prompt.trim().is_empty() {
            return Err(format!("roles[{}].system_prompt は必須です", index));
        }
        if role.cli_type.trim().is_empty() {
            return Err(format!("roles[{}].cli_type は必須です", index));
        }
        if role.model.trim().is_empty() {
            return Err(format!("roles[{}].model は必須です", index));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_team_configuration(app: AppHandle) -> Result<TeamConfiguration, String> {
    ensure_default_team_templates(&app).await?;

    let settings_query = "SELECT max_concurrent_agents FROM team_settings WHERE id = 1";
    let settings = select_query::<TeamSettings>(&app, settings_query, vec![]).await?;

    let roles_query = r#"
        SELECT id, name, system_prompt, cli_type, model, avatar_image, sort_order
        FROM team_roles
        WHERE team_settings_id = 1
        ORDER BY sort_order ASC, created_at ASC
    "#;
    let roles = select_query::<TeamRole>(&app, roles_query, vec![]).await?;

    Ok(TeamConfiguration {
        max_concurrent_agents: settings
            .first()
            .map(|s| s.max_concurrent_agents)
            .unwrap_or(5),
        roles,
    })
}

#[tauri::command]
pub async fn save_team_configuration(
    app: AppHandle,
    config: TeamConfigurationInput,
) -> Result<(), String> {
    validate_team_configuration(&config)?;

    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances
        .get(DB_STRING)
        .ok_or("Database instance not found")?;

    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    sqlx::query(
        r#"
        INSERT INTO team_settings (id, max_concurrent_agents, updated_at)
        VALUES (1, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(id) DO UPDATE SET
            max_concurrent_agents = excluded.max_concurrent_agents,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(config.max_concurrent_agents)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM team_roles WHERE team_settings_id = 1")
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    for (index, role) in config.roles.iter().enumerate() {
        let cli_type = if role.cli_type.trim().is_empty() {
            "claude"
        } else {
            role.cli_type.trim()
        };
        let avatar_image = role
            .avatar_image
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| default_team_role_avatar_for_sort_order(index as i32).map(str::to_string));
        sqlx::query(
            r#"
            INSERT INTO team_roles (
                id,
                team_settings_id,
                name,
                system_prompt,
                cli_type,
                model,
                avatar_image,
                sort_order,
                updated_at
            ) VALUES (?, 1, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(role.id.trim())
        .bind(role.name.trim())
        .bind(role.system_prompt.trim())
        .bind(cli_type)
        .bind(role.model.trim())
        .bind(avatar_image)
        .bind(index as i32)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Task Dependencies CRUD
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_all_task_dependencies(
    app: AppHandle,
    project_id: String,
) -> Result<Vec<TaskDependency>, String> {
    let query = r#"
        SELECT td.task_id, td.blocked_by_task_id
        FROM task_dependencies td
        JOIN tasks t ON td.task_id = t.id
        WHERE t.project_id = ?
    "#;
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<TaskDependency>(&app, query, values).await
}

#[tauri::command]
pub async fn set_task_dependencies(
    app: AppHandle,
    task_id: String,
    blocked_by_ids: Vec<String>,
) -> Result<(), String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances
        .get(DB_STRING)
        .ok_or("Database instance not found")?;

    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 既存の依存関係をすべて削除
    let _ = sqlx::query("DELETE FROM task_dependencies WHERE task_id = ?")
        .bind(&task_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 新しい依存関係を挿入
    for blocker_id in &blocked_by_ids {
        if blocker_id != &task_id {
            let _ = sqlx::query("INSERT OR IGNORE INTO task_dependencies (task_id, blocked_by_task_id) VALUES (?, ?)")
                .bind(&task_id)
                .bind(blocker_id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
        }
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------------------------------------
// Sprints CRUD
// ------------------------------------------------------------------------------------------------

async fn ensure_draft_retro_session(
    pool: &SqlitePool,
    project_id: &str,
    sprint_id: &str,
) -> Result<(), String> {
    let existing_session_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM retro_sessions WHERE sprint_id = ? LIMIT 1")
            .bind(sprint_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    if existing_session_id.is_some() {
        return Ok(());
    }

    let retro_session_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO retro_sessions (id, project_id, sprint_id, status) VALUES (?, ?, ?, 'draft')",
    )
    .bind(&retro_session_id)
    .bind(project_id)
    .bind(sprint_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_sprints(app: AppHandle, project_id: String) -> Result<Vec<Sprint>, String> {
    let query = "SELECT * FROM sprints WHERE project_id = ? ORDER BY started_at DESC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<Sprint>(&app, query, values).await
}

#[tauri::command]
pub async fn create_planned_sprint(app: AppHandle, project_id: String) -> Result<Sprint, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let query = "INSERT INTO sprints (id, project_id, status) VALUES (?, ?, 'Planned')";
    let values = vec![
        serde_json::to_value(&id).unwrap(),
        serde_json::to_value(&project_id).unwrap(),
    ];
    execute_query(&app, query, values).await?;

    let get_query = "SELECT * FROM sprints WHERE id = ?";
    let get_values = vec![serde_json::to_value(&id).unwrap()];
    let mut sprints = select_query::<Sprint>(&app, get_query, get_values).await?;
    sprints
        .pop()
        .ok_or("Failed to fetch created sprint".to_string())
}

#[tauri::command]
pub async fn start_sprint(
    app: AppHandle,
    sprint_id: String,
    duration_ms: i64,
) -> Result<QueryResult, String> {
    let started_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let query =
        "UPDATE sprints SET status = 'Active', started_at = ?, duration_ms = ? WHERE id = ?";
    let values = vec![
        serde_json::to_value(started_at).unwrap(),
        serde_json::to_value(duration_ms).unwrap(),
        serde_json::to_value(sprint_id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn complete_sprint(
    app: AppHandle,
    sprint_id: String,
    project_id: String,
    completed_at: i64,
) -> Result<bool, String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances
        .get(DB_STRING)
        .ok_or("Database instance not found")?;

    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Update sprint status to Completed
    let query_sprint = "UPDATE sprints SET status = 'Completed', completed_at = ? WHERE id = ?";
    let _ = sqlx::query(query_sprint)
        .bind(completed_at)
        .bind(&sprint_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 2. Archive Done tasks belonging to this sprint
    let query_tasks_done = "UPDATE tasks SET archived = 1, updated_at = CURRENT_TIMESTAMP WHERE status = 'Done' AND archived = 0 AND sprint_id = ?";
    let _ = sqlx::query(query_tasks_done)
        .bind(&sprint_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 3. Find or Create a Planned Sprint for rollover
    let query_planned =
        "SELECT id FROM sprints WHERE project_id = ? AND status = 'Planned' LIMIT 1";
    let planned_sprint: Option<(String,)> = sqlx::query_as(query_planned)
        .bind(&project_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    let rollover_sprint_id = match planned_sprint {
        Some((id,)) => id,
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            let query_create =
                "INSERT INTO sprints (id, project_id, status) VALUES (?, ?, 'Planned')";
            let _ = sqlx::query(query_create)
                .bind(&new_id)
                .bind(&project_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            new_id
        }
    };

    // 4. Move not-Done tasks to the Planned Sprint (rollover)
    let query_tasks_undone = "UPDATE tasks SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE status != 'Done' AND archived = 0 AND sprint_id = ?";
    let _ = sqlx::query(query_tasks_undone)
        .bind(&rollover_sprint_id)
        .bind(&sprint_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 4. Archive Stories if ALL their children are archived
    let query_stories = r#"
        UPDATE stories 
        SET archived = 1, updated_at = CURRENT_TIMESTAMP 
        WHERE archived = 0 AND project_id = ?
        AND EXISTS (
            SELECT 1 FROM tasks 
            WHERE tasks.story_id = stories.id
        )
        AND NOT EXISTS (
            SELECT 1 FROM tasks 
            WHERE tasks.story_id = stories.id 
            AND tasks.archived = 0
        )
    "#;
    let _ = sqlx::query(query_stories)
        .bind(&project_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    // 6. Move unarchived stories to the Planned Sprint (rollover)
    let query_stories_undone = "UPDATE stories SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE archived = 0 AND sprint_id = ?";
    let _ = sqlx::query(query_stories_undone)
        .bind(&rollover_sprint_id)
        .bind(&sprint_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    if let Err(error) = ensure_draft_retro_session(pool, &project_id, &sprint_id).await {
        log::warn!(
            "Failed to create draft retro session for sprint {} in project {}: {}",
            sprint_id,
            project_id,
            error
        );
    }

    Ok(true)
}

// ------------------------------------------------------------------------------------------------
// Retrospective CRUD
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_retro_sessions(
    app: AppHandle,
    project_id: String,
) -> Result<Vec<RetroSession>, String> {
    let query =
        "SELECT * FROM retro_sessions WHERE project_id = ? ORDER BY created_at DESC, id DESC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<RetroSession>(&app, query, values).await
}

#[tauri::command]
pub async fn get_retro_session(app: AppHandle, id: String) -> Result<Option<RetroSession>, String> {
    let query = "SELECT * FROM retro_sessions WHERE id = ? LIMIT 1";
    let values = vec![serde_json::to_value(id).unwrap()];
    let mut sessions = select_query::<RetroSession>(&app, query, values).await?;
    Ok(sessions.pop())
}

#[tauri::command]
pub async fn create_retro_session(
    app: AppHandle,
    project_id: String,
    sprint_id: String,
    status: Option<String>,
    summary: Option<String>,
) -> Result<RetroSession, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let status_value = status.unwrap_or_else(|| "draft".to_string());
    validate_retro_session_status(&status_value)?;

    let existing_query = "SELECT * FROM retro_sessions WHERE sprint_id = ? LIMIT 1";
    let existing_values = vec![serde_json::to_value(&sprint_id).unwrap()];
    let existing_sessions =
        select_query::<RetroSession>(&app, existing_query, existing_values).await?;
    if !existing_sessions.is_empty() {
        return Err("この sprint_id には既に retro_session が存在します".to_string());
    }

    let query =
        "INSERT INTO retro_sessions (id, project_id, sprint_id, status, summary) VALUES (?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(&id).unwrap(),
        serde_json::to_value(&project_id).unwrap(),
        serde_json::to_value(&sprint_id).unwrap(),
        serde_json::to_value(&status_value).unwrap(),
        serde_json::to_value(&summary).unwrap(),
    ];
    execute_query(&app, query, values).await?;

    let get_query = "SELECT * FROM retro_sessions WHERE id = ? LIMIT 1";
    let get_values = vec![serde_json::to_value(&id).unwrap()];
    let mut sessions = select_query::<RetroSession>(&app, get_query, get_values).await?;
    sessions
        .pop()
        .ok_or("Failed to fetch created retro session".to_string())
}

#[tauri::command]
pub async fn update_retro_session(
    app: AppHandle,
    id: String,
    status: String,
    summary: Option<String>,
) -> Result<QueryResult, String> {
    validate_retro_session_status(&status)?;
    let query =
        "UPDATE retro_sessions SET status = ?, summary = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(summary).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn delete_retro_session(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM retro_sessions WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn get_retro_items(
    app: AppHandle,
    retro_session_id: String,
) -> Result<Vec<RetroItem>, String> {
    let query = "SELECT * FROM retro_items WHERE retro_session_id = ? ORDER BY sort_order ASC, created_at ASC, id ASC";
    let values = vec![serde_json::to_value(retro_session_id).unwrap()];
    select_query::<RetroItem>(&app, query, values).await
}

#[tauri::command]
pub async fn add_retro_item(
    app: AppHandle,
    retro_session_id: String,
    category: String,
    content: String,
    source: String,
    source_role_id: Option<String>,
    sort_order: Option<i32>,
) -> Result<RetroItem, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let sort_order_value = sort_order.unwrap_or(0);
    validate_retro_item_category(&category)?;
    validate_retro_item_source(&source)?;

    let query = "INSERT INTO retro_items (id, retro_session_id, category, content, source, source_role_id, sort_order) VALUES (?, ?, ?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(&id).unwrap(),
        serde_json::to_value(&retro_session_id).unwrap(),
        serde_json::to_value(&category).unwrap(),
        serde_json::to_value(&content).unwrap(),
        serde_json::to_value(&source).unwrap(),
        serde_json::to_value(&source_role_id).unwrap(),
        serde_json::to_value(sort_order_value).unwrap(),
    ];
    execute_query(&app, query, values).await?;

    let get_query = "SELECT * FROM retro_items WHERE id = ? LIMIT 1";
    let get_values = vec![serde_json::to_value(&id).unwrap()];
    let mut items = select_query::<RetroItem>(&app, get_query, get_values).await?;
    items
        .pop()
        .ok_or("Failed to fetch created retro item".to_string())
}

#[tauri::command]
pub async fn update_retro_item(
    app: AppHandle,
    id: String,
    category: String,
    content: String,
    source: String,
    source_role_id: Option<String>,
    sort_order: Option<i32>,
) -> Result<QueryResult, String> {
    let sort_order_value = sort_order.unwrap_or(0);
    validate_retro_item_category(&category)?;
    validate_retro_item_source(&source)?;

    let query =
        "UPDATE retro_items SET category = ?, content = ?, source = ?, source_role_id = ?, sort_order = ? WHERE id = ?";
    let values = vec![
        serde_json::to_value(category).unwrap(),
        serde_json::to_value(content).unwrap(),
        serde_json::to_value(source).unwrap(),
        serde_json::to_value(source_role_id).unwrap(),
        serde_json::to_value(sort_order_value).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn delete_retro_item(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM retro_items WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn approve_retro_item(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "UPDATE retro_items SET is_approved = 1 WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn get_retro_rules(app: AppHandle, project_id: String) -> Result<Vec<RetroRule>, String> {
    let query = "SELECT * FROM retro_rules WHERE project_id = ? ORDER BY updated_at DESC, created_at DESC, id DESC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<RetroRule>(&app, query, values).await
}

#[tauri::command]
pub async fn add_retro_rule(
    app: AppHandle,
    project_id: String,
    retro_item_id: Option<String>,
    sprint_id: Option<String>,
    content: String,
    is_active: Option<bool>,
) -> Result<RetroRule, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let is_active_value = if is_active.unwrap_or(true) { 1 } else { 0 };

    let query = "INSERT INTO retro_rules (id, project_id, retro_item_id, sprint_id, content, is_active) VALUES (?, ?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(&id).unwrap(),
        serde_json::to_value(&project_id).unwrap(),
        serde_json::to_value(&retro_item_id).unwrap(),
        serde_json::to_value(&sprint_id).unwrap(),
        serde_json::to_value(&content).unwrap(),
        serde_json::to_value(is_active_value).unwrap(),
    ];
    execute_query(&app, query, values).await?;

    let get_query = "SELECT * FROM retro_rules WHERE id = ? LIMIT 1";
    let get_values = vec![serde_json::to_value(&id).unwrap()];
    let mut rules = select_query::<RetroRule>(&app, get_query, get_values).await?;
    rules
        .pop()
        .ok_or("Failed to fetch created retro rule".to_string())
}

#[tauri::command]
pub async fn update_retro_rule(
    app: AppHandle,
    id: String,
    retro_item_id: Option<String>,
    sprint_id: Option<String>,
    content: String,
    is_active: bool,
) -> Result<QueryResult, String> {
    let is_active_value = if is_active { 1 } else { 0 };
    let query = "UPDATE retro_rules SET retro_item_id = ?, sprint_id = ?, content = ?, is_active = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(retro_item_id).unwrap(),
        serde_json::to_value(sprint_id).unwrap(),
        serde_json::to_value(content).unwrap(),
        serde_json::to_value(is_active_value).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn delete_retro_rule(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM retro_rules WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn get_project_notes(
    app: AppHandle,
    project_id: String,
) -> Result<Vec<ProjectNote>, String> {
    let query =
        "SELECT * FROM project_notes WHERE project_id = ? ORDER BY created_at DESC, id DESC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<ProjectNote>(&app, query, values).await
}

#[tauri::command]
pub async fn add_project_note(
    app: AppHandle,
    project_id: String,
    sprint_id: Option<String>,
    title: String,
    content: String,
    source: Option<String>,
) -> Result<ProjectNote, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let source_value = source.unwrap_or_else(|| "user".to_string());
    validate_project_note_source(&source_value)?;

    let query = "INSERT INTO project_notes (id, project_id, sprint_id, title, content, source) VALUES (?, ?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(&id).unwrap(),
        serde_json::to_value(&project_id).unwrap(),
        serde_json::to_value(&sprint_id).unwrap(),
        serde_json::to_value(&title).unwrap(),
        serde_json::to_value(&content).unwrap(),
        serde_json::to_value(&source_value).unwrap(),
    ];
    execute_query(&app, query, values).await?;

    let get_query = "SELECT * FROM project_notes WHERE id = ? LIMIT 1";
    let get_values = vec![serde_json::to_value(&id).unwrap()];
    let mut notes = select_query::<ProjectNote>(&app, get_query, get_values).await?;
    notes
        .pop()
        .ok_or("Failed to fetch created project note".to_string())
}

#[tauri::command]
pub async fn update_project_note(
    app: AppHandle,
    id: String,
    sprint_id: Option<String>,
    title: String,
    content: String,
    source: String,
) -> Result<QueryResult, String> {
    validate_project_note_source(&source)?;
    let query = "UPDATE project_notes SET sprint_id = ?, title = ?, content = ?, source = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(sprint_id).unwrap(),
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(content).unwrap(),
        serde_json::to_value(source).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn delete_project_note(app: AppHandle, id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM project_notes WHERE id = ?";
    let values = vec![serde_json::to_value(id).unwrap()];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn assign_story_to_sprint(
    app: AppHandle,
    story_id: String,
    sprint_id: Option<String>,
) -> Result<bool, String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances
        .get(DB_STRING)
        .ok_or("Database instance not found")?;

    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let q1 = "UPDATE stories SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let _ = sqlx::query(q1)
        .bind(&sprint_id)
        .bind(&story_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    let q2 = "UPDATE tasks SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE story_id = ? AND archived = 0";
    let _ = sqlx::query(q2)
        .bind(&sprint_id)
        .bind(&story_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn assign_task_to_sprint(
    app: AppHandle,
    task_id: String,
    sprint_id: Option<String>,
) -> Result<QueryResult, String> {
    let query = "UPDATE tasks SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        sprint_id
            .map(|s| serde_json::to_value(s).unwrap())
            .unwrap_or(serde_json::Value::Null),
        serde_json::to_value(task_id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

const CONTEXT_SUMMARY_TEXT_LIMIT: usize = 80;
const ARCHIVED_CONTEXT_STORY_LIMIT: usize = 8;
const ARCHIVED_CONTEXT_TASK_LIMIT_PER_STORY: usize = 3;
const ARCHIVED_CONTEXT_ORPHAN_TASK_LIMIT: usize = 6;

fn summarize_context_value(value: Option<&str>, max_chars: usize) -> Option<String> {
    let normalized = value
        .unwrap_or_default()
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return None;
    }

    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        Some(trimmed.to_string())
    } else {
        Some(format!(
            "{}...",
            trimmed.chars().take(max_chars).collect::<String>()
        ))
    }
}

fn render_story_context_block(
    stories_filtered: &[&Story],
    tasks_all: &[Task],
    dependencies: &[TaskDependency],
) -> String {
    let mut out = String::new();
    for story in stories_filtered {
        let desc_str =
            summarize_context_value(story.description.as_deref(), CONTEXT_SUMMARY_TEXT_LIMIT)
                .map(|summary| format!(": {summary}"))
                .unwrap_or_default();
        out.push_str(&format!(
            "- Story [P{}][ID: {}]: {}{} (Status: {})\n",
            story.priority, story.id, story.title, desc_str, story.status
        ));

        for task in tasks_all.iter().filter(|task| task.story_id == story.id) {
            let status_icon = match task.status.as_str() {
                "Done" => " ✅",
                "In Progress" => " 🔄",
                _ => "",
            };
            let blockers: Vec<&str> = dependencies
                .iter()
                .filter(|dependency| dependency.task_id == task.id)
                .map(|dependency| dependency.blocked_by_task_id.as_str())
                .collect();
            let blocker_str = if blockers.is_empty() {
                String::new()
            } else {
                format!(" [blocked_by: {}]", blockers.join(", "))
            };
            out.push_str(&format!(
                "  - Task [P{}]: {} (Status: {}){}{}\n",
                task.priority, task.title, task.status, status_icon, blocker_str
            ));
        }
    }

    out
}

fn render_archived_context_summary(archived_stories: &[Story], archived_tasks: &[Task]) -> String {
    let mut out = String::new();
    let mut rendered_story_ids = HashSet::new();

    for story in archived_stories.iter().take(ARCHIVED_CONTEXT_STORY_LIMIT) {
        rendered_story_ids.insert(story.id.clone());

        let story_summary =
            summarize_context_value(story.description.as_deref(), CONTEXT_SUMMARY_TEXT_LIMIT)
                .map(|summary| format!(": {summary}"))
                .unwrap_or_default();
        let story_tasks = archived_tasks
            .iter()
            .filter(|task| task.story_id == story.id)
            .collect::<Vec<_>>();

        out.push_str(&format!(
            "- 完了済み Story [P{}][ID: {}]: {}{} (Status: {})\n",
            story.priority, story.id, story.title, story_summary, story.status
        ));

        for task in story_tasks
            .iter()
            .take(ARCHIVED_CONTEXT_TASK_LIMIT_PER_STORY)
        {
            let task_summary =
                summarize_context_value(task.description.as_deref(), CONTEXT_SUMMARY_TEXT_LIMIT)
                    .map(|summary| format!(": {summary}"))
                    .unwrap_or_default();
            out.push_str(&format!(
                "  - 完了済み Task [P{}]: {}{}\n",
                task.priority, task.title, task_summary
            ));
        }

        if story_tasks.len() > ARCHIVED_CONTEXT_TASK_LIMIT_PER_STORY {
            out.push_str(&format!(
                "  - 他 {} 件の完了済み Task\n",
                story_tasks.len() - ARCHIVED_CONTEXT_TASK_LIMIT_PER_STORY
            ));
        }
    }

    let orphan_tasks = archived_tasks
        .iter()
        .filter(|task| !rendered_story_ids.contains(&task.story_id))
        .take(ARCHIVED_CONTEXT_ORPHAN_TASK_LIMIT)
        .collect::<Vec<_>>();
    if !orphan_tasks.is_empty() {
        out.push_str("- 完了済み Task（関連 Story は現行 backlog 外）:\n");
        for task in orphan_tasks {
            let task_summary =
                summarize_context_value(task.description.as_deref(), CONTEXT_SUMMARY_TEXT_LIMIT)
                    .map(|summary| format!(": {summary}"))
                    .unwrap_or_default();
            out.push_str(&format!(
                "  - [Story ID: {}][P{}] {}{}\n",
                task.story_id, task.priority, task.title, task_summary
            ));
        }
    }

    out
}

pub async fn build_project_context(app: &AppHandle, project_id: &str) -> Result<String, String> {
    let query_project = "SELECT * FROM projects WHERE id = ?";
    let projects = select_query::<Project>(
        app,
        query_project,
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;
    let local_path = projects.first().and_then(|p| p.local_path.clone());

    let mut md = String::new();

    // 1. プロジェクトドキュメントの読み込み
    if let Some(path) = local_path {
        let p = std::path::Path::new(&path);
        if p.exists() && p.is_dir() {
            md.push_str("\n【プロジェクト既存ドキュメントコンテキスト】\n");
            let files = ["PRODUCT_CONTEXT.md", "ARCHITECTURE.md", "Rule.md"];
            for f in files {
                let fp = p.join(f);
                if fp.exists() {
                    let content = std::fs::read_to_string(&fp).unwrap_or_default();
                    md.push_str(&format!("--- {} ---\n{}\n", f, content));
                }
            }
        }
    }

    // 2. スプリント情報の取得
    let query_sprints =
        "SELECT * FROM sprints WHERE project_id = ? AND status != 'Completed' ORDER BY status ASC";
    let sprints = select_query::<Sprint>(
        app,
        query_sprints,
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;

    let active_sprint_id = sprints
        .iter()
        .find(|s| s.status == "Active")
        .map(|s| s.id.clone());
    let planned_sprint_ids: Vec<String> = sprints
        .iter()
        .filter(|s| s.status == "Planned")
        .map(|s| s.id.clone())
        .collect();

    // 3. ストーリーとタスクの取得
    let query_stories =
        "SELECT * FROM stories WHERE archived = 0 AND project_id = ? ORDER BY created_at ASC";
    let stories = select_query::<Story>(
        app,
        query_stories,
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;

    let query_tasks =
        "SELECT * FROM tasks WHERE archived = 0 AND project_id = ? ORDER BY created_at ASC";
    let tasks = select_query::<Task>(
        app,
        query_tasks,
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;

    let archived_stories = select_query::<Story>(
        app,
        "SELECT * FROM stories WHERE archived = 1 AND project_id = ? ORDER BY updated_at DESC, created_at DESC",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;

    let archived_tasks = select_query::<Task>(
        app,
        "SELECT * FROM tasks WHERE archived = 1 AND project_id = ? ORDER BY updated_at DESC, created_at DESC",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;

    if stories.is_empty()
        && tasks.is_empty()
        && archived_stories.is_empty()
        && archived_tasks.is_empty()
        && md.is_empty()
    {
        return Ok(String::new());
    }

    // タスク依存関係の取得
    let query_deps = "SELECT td.task_id, td.blocked_by_task_id FROM task_dependencies td JOIN tasks t ON td.task_id = t.id WHERE t.project_id = ? AND t.archived = 0";
    let dependencies = select_query::<TaskDependency>(
        app,
        query_deps,
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await
    .unwrap_or_default();

    // 4. プロダクトバックログ（sprint_id IS NULL）
    let backlog_stories: Vec<&Story> = stories.iter().filter(|s| s.sprint_id.is_none()).collect();
    if !backlog_stories.is_empty() {
        md.push_str("\n【プロダクトバックログ（未着手）】\n");
        md.push_str(&render_story_context_block(
            &backlog_stories,
            &tasks,
            &dependencies,
        ));
    }

    // 5. アクティブスプリント
    if let Some(ref active_id) = active_sprint_id {
        let active_stories: Vec<&Story> = stories
            .iter()
            .filter(|s| s.sprint_id.as_deref() == Some(active_id.as_str()))
            .collect();
        if !active_stories.is_empty() {
            md.push_str(&format!(
                "\n【アクティブスプリント（進行中）】Sprint ID: {}\n",
                active_id
            ));
            md.push_str(&render_story_context_block(
                &active_stories,
                &tasks,
                &dependencies,
            ));
        }
    }

    // 6. 計画中スプリント (Planned)
    for planned_id in &planned_sprint_ids {
        let planned_stories: Vec<&Story> = stories
            .iter()
            .filter(|s| s.sprint_id.as_deref() == Some(planned_id.as_str()))
            .collect();
        if !planned_stories.is_empty() {
            md.push_str(&format!(
                "\n【計画中スプリント (Planned)】Sprint ID: {}\n",
                planned_id
            ));
            md.push_str(&render_story_context_block(
                &planned_stories,
                &tasks,
                &dependencies,
            ));
        }
    }

    let archived_summary = render_archived_context_summary(&archived_stories, &archived_tasks);
    if !archived_summary.trim().is_empty() {
        md.push_str("\n【完了済み実装サマリ（アーカイブ済み）】\n");
        md.push_str(&archived_summary);
    }

    Ok(md)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskDraft {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
    pub blocked_by_indices: Option<Vec<usize>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoryDraftInput {
    pub target_story_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub priority: Option<i32>,
}

pub async fn insert_story_with_tasks(
    app: &AppHandle,
    project_id: &str,
    story_draft: StoryDraftInput,
    tasks_draft: Vec<TaskDraft>,
) -> Result<String, String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances
        .get(DB_STRING)
        .ok_or("Database instance not found")?;

    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let story_priority = story_draft.priority.unwrap_or(3);

    let story_id = if let Some(existing_id) = story_draft.target_story_id {
        existing_id
    } else {
        let new_id = uuid::Uuid::new_v4().to_string();
        let q_story = "INSERT INTO stories (id, project_id, title, description, acceptance_criteria, status, priority) VALUES (?, ?, ?, ?, ?, ?, ?)";

        let _ = sqlx::query(q_story)
            .bind(&new_id)
            .bind(project_id)
            .bind(&story_draft.title)
            .bind(&story_draft.description)
            .bind(&story_draft.acceptance_criteria)
            .bind("Backlog")
            .bind(story_priority)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        new_id
    };

    // タスクIDを収集（依存関係のインデックス→ID変換に使用）
    let mut task_ids: Vec<String> = Vec::with_capacity(tasks_draft.len());

    for task in &tasks_draft {
        let task_id = uuid::Uuid::new_v4().to_string();
        let task_priority = task.priority.unwrap_or(3);
        let q_task = "INSERT INTO tasks (id, project_id, story_id, title, description, status, priority) VALUES (?, ?, ?, ?, ?, ?, ?)";
        let _ = sqlx::query(q_task)
            .bind(&task_id)
            .bind(project_id)
            .bind(&story_id)
            .bind(&task.title)
            .bind(&task.description)
            .bind("To Do")
            .bind(task_priority)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        task_ids.push(task_id);
    }

    // blocked_by_indices → 実IDに変換して task_dependencies に挿入
    for (i, task) in tasks_draft.iter().enumerate() {
        if let Some(indices) = &task.blocked_by_indices {
            let task_id = &task_ids[i];
            for &blocker_idx in indices {
                if blocker_idx < task_ids.len() && blocker_idx != i {
                    let blocker_id = &task_ids[blocker_idx];
                    let q_dep = "INSERT OR IGNORE INTO task_dependencies (task_id, blocked_by_task_id) VALUES (?, ?)";
                    let _ = sqlx::query(q_dep)
                        .bind(task_id)
                        .bind(blocker_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(story_id)
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_draft_retro_session, render_archived_context_summary, summarize_context_value,
        Story, Task,
    };
    use sqlx::SqlitePool;

    fn sample_story(id: &str, title: &str, archived: bool) -> Story {
        Story {
            id: id.to_string(),
            project_id: "project-1".to_string(),
            title: title.to_string(),
            description: Some("既存実装の要約".to_string()),
            acceptance_criteria: None,
            status: if archived {
                "Done".to_string()
            } else {
                "Backlog".to_string()
            },
            sprint_id: None,
            archived,
            created_at: "2026-04-10T00:00:00Z".to_string(),
            updated_at: "2026-04-10T00:00:00Z".to_string(),
            priority: 2,
        }
    }

    fn sample_task(story_id: &str, title: &str) -> Task {
        Task {
            id: format!("task-{title}"),
            project_id: "project-1".to_string(),
            story_id: story_id.to_string(),
            title: title.to_string(),
            description: Some("完了した作業の詳細".to_string()),
            status: "Done".to_string(),
            sprint_id: None,
            archived: true,
            assignee_type: None,
            assigned_role_id: None,
            created_at: "2026-04-10T00:00:00Z".to_string(),
            updated_at: "2026-04-10T00:00:00Z".to_string(),
            priority: 3,
        }
    }

    async fn setup_retro_session_test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

        sqlx::query("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                local_path TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE sprints (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                status TEXT NOT NULL,
                started_at INTEGER,
                completed_at INTEGER,
                duration_ms INTEGER
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE retro_sessions (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                sprint_id TEXT NOT NULL REFERENCES sprints(id) ON DELETE CASCADE,
                status TEXT NOT NULL DEFAULT 'draft',
                summary TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO projects (id, name) VALUES ('project-1', 'Retro Test Project')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO sprints (id, project_id, status) VALUES ('sprint-1', 'project-1', 'Completed')",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[test]
    fn summarize_context_value_collapses_whitespace() {
        assert_eq!(
            summarize_context_value(Some("line1\n line2"), 80).as_deref(),
            Some("line1 line2")
        );
    }

    #[test]
    fn archived_context_summary_includes_completed_story_and_tasks() {
        let summary = render_archived_context_summary(
            &[sample_story("story-1", "通知設定画面を追加", true)],
            &[sample_task("story-1", "設定保存APIを実装")],
        );

        assert!(summary.contains("完了済み Story"));
        assert!(summary.contains("通知設定画面を追加"));
        assert!(summary.contains("完了済み Task"));
        assert!(summary.contains("設定保存APIを実装"));
    }

    #[tokio::test]
    async fn ensure_draft_retro_session_creates_single_draft_session() {
        let pool = setup_retro_session_test_pool().await;

        ensure_draft_retro_session(&pool, "project-1", "sprint-1")
            .await
            .unwrap();

        let sessions: Vec<(String, String)> =
            sqlx::query_as("SELECT sprint_id, status FROM retro_sessions")
                .fetch_all(&pool)
                .await
                .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].0, "sprint-1");
        assert_eq!(sessions[0].1, "draft");
    }

    #[tokio::test]
    async fn ensure_draft_retro_session_skips_duplicate_creation() {
        let pool = setup_retro_session_test_pool().await;

        sqlx::query(
            "INSERT INTO retro_sessions (id, project_id, sprint_id, status) VALUES (?, ?, ?, ?)",
        )
        .bind("retro-existing")
        .bind("project-1")
        .bind("sprint-1")
        .bind("draft")
        .execute(&pool)
        .await
        .unwrap();

        ensure_draft_retro_session(&pool, "project-1", "sprint-1")
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM retro_sessions")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(count, 1);
    }
}
