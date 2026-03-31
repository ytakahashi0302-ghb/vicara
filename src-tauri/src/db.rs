use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_sql::{DbInstances, DbPool};
use serde_json::Value as JsonValue;
use sqlx::sqlite::SqliteRow;

// Define shared types
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub rows_affected: u64,
    pub last_insert_id: i64,
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
    pub created_at: String,
    pub updated_at: String,
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
pub struct TeamChatMessage {
    pub id: String,
    pub project_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

const DB_STRING: &str = "sqlite:ai-scrum.db";

pub async fn execute_query(app: &AppHandle, query: &str, values: Vec<JsonValue>) -> Result<QueryResult, String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances.get(DB_STRING).ok_or("Database instance not found")?;
    
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

pub async fn select_query<T>(app: &AppHandle, query: &str, values: Vec<JsonValue>) -> Result<Vec<T>, String> 
where 
    T: for<'r> sqlx::FromRow<'r, SqliteRow> + Send + Unpin 
{
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances.get(DB_STRING).ok_or("Database instance not found")?;
    
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

// ------------------------------------------------------------------------------------------------
// Projects CRUD
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_projects(app: AppHandle) -> Result<Vec<Project>, String> {
    let query = "SELECT * FROM projects ORDER BY created_at ASC";
    select_query::<Project>(&app, query, vec![]).await
}

#[tauri::command]
pub async fn create_project(app: AppHandle, id: String, name: String, description: Option<String>) -> Result<QueryResult, String> {
    let query = "INSERT INTO projects (id, name, description) VALUES (?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(name).unwrap(),
        serde_json::to_value(description).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn update_project(app: AppHandle, id: String, name: String, description: Option<String>) -> Result<QueryResult, String> {
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
pub async fn update_project_path(app: AppHandle, id: String, local_path: Option<String>) -> Result<ProjectPathUpdateResult, String> {
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
    let query = "SELECT * FROM stories WHERE archived = 0 AND project_id = ? ORDER BY created_at DESC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    let stories = select_query::<Story>(&app, query, values).await?;
    log::debug!("Fetched stories: {:?}", stories);
    Ok(stories)
}

#[tauri::command]
pub async fn get_archived_stories(app: AppHandle, project_id: String) -> Result<Vec<Story>, String> {
    let query = "SELECT * FROM stories WHERE archived = 1 AND project_id = ?";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<Story>(&app, query, values).await
}

#[tauri::command]
pub async fn add_story(app: AppHandle, id: String, project_id: String, title: String, description: Option<String>, acceptance_criteria: Option<String>, status: String) -> Result<QueryResult, String> {
    let query = "INSERT INTO stories (id, project_id, title, description, acceptance_criteria, status) VALUES (?, ?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(acceptance_criteria).unwrap(),
        serde_json::to_value(status).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn update_story(app: AppHandle, id: String, title: String, description: Option<String>, acceptance_criteria: Option<String>, status: String) -> Result<QueryResult, String> {
    let query = "UPDATE stories SET title = ?, description = ?, acceptance_criteria = ?, status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(acceptance_criteria).unwrap(),
        serde_json::to_value(status).unwrap(),
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
pub async fn get_tasks_by_story_id(app: AppHandle, story_id: String, project_id: String) -> Result<Vec<Task>, String> {
    let query = "SELECT * FROM tasks WHERE story_id = ? AND archived = 0 AND project_id = ? ORDER BY created_at ASC";
    let values = vec![
        serde_json::to_value(story_id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
    ];
    select_query::<Task>(&app, query, values).await
}

#[tauri::command]
pub async fn add_task(app: AppHandle, id: String, project_id: String, story_id: String, title: String, description: Option<String>, status: String, assignee_type: Option<String>) -> Result<QueryResult, String> {
    let query = "INSERT INTO tasks (id, project_id, story_id, title, description, status, assignee_type) VALUES (?, ?, ?, ?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
        serde_json::to_value(story_id).unwrap(),
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(assignee_type).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn update_task_status(app: AppHandle, id: String, status: String) -> Result<QueryResult, String> {
    let query = "UPDATE tasks SET status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn update_task(app: AppHandle, id: String, title: String, description: Option<String>, status: String, assignee_type: Option<String>) -> Result<QueryResult, String> {
    let query = "UPDATE tasks SET title = ?, description = ?, status = ?, assignee_type = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        serde_json::to_value(title).unwrap(),
        serde_json::to_value(description).unwrap(),
        serde_json::to_value(status).unwrap(),
        serde_json::to_value(assignee_type).unwrap(),
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
// Team Chat Messages CRUD (AI Team Leader)
// ------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn get_team_chat_messages(app: AppHandle, project_id: String) -> Result<Vec<TeamChatMessage>, String> {
    let query = "SELECT * FROM team_chat_messages WHERE project_id = ? ORDER BY created_at ASC";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    select_query::<TeamChatMessage>(&app, query, values).await
}

#[tauri::command]
pub async fn add_team_chat_message(app: AppHandle, id: String, project_id: String, role: String, content: String) -> Result<QueryResult, String> {
    let query = "INSERT INTO team_chat_messages (id, project_id, role, content) VALUES (?, ?, ?, ?)";
    let values = vec![
        serde_json::to_value(id).unwrap(),
        serde_json::to_value(project_id).unwrap(),
        serde_json::to_value(role).unwrap(),
        serde_json::to_value(content).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn clear_team_chat_messages(app: AppHandle, project_id: String) -> Result<QueryResult, String> {
    let query = "DELETE FROM team_chat_messages WHERE project_id = ?";
    let values = vec![serde_json::to_value(project_id).unwrap()];
    execute_query(&app, query, values).await
}

// ------------------------------------------------------------------------------------------------
// Sprints CRUD
// ------------------------------------------------------------------------------------------------

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
    sprints.pop().ok_or("Failed to fetch created sprint".to_string())
}

#[tauri::command]
pub async fn start_sprint(app: AppHandle, sprint_id: String, duration_ms: i64) -> Result<QueryResult, String> {
    let started_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let query = "UPDATE sprints SET status = 'Active', started_at = ?, duration_ms = ? WHERE id = ?";
    let values = vec![
        serde_json::to_value(started_at).unwrap(),
        serde_json::to_value(duration_ms).unwrap(),
        serde_json::to_value(sprint_id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

#[tauri::command]
pub async fn complete_sprint(app: AppHandle, sprint_id: String, project_id: String, completed_at: i64) -> Result<bool, String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances.get(DB_STRING).ok_or("Database instance not found")?;
    
    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    // 1. Update sprint status to Completed
    let query_sprint = "UPDATE sprints SET status = 'Completed', completed_at = ? WHERE id = ?";
    let _ = sqlx::query(query_sprint)
        .bind(completed_at).bind(&sprint_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    // 2. Archive Done tasks belonging to this sprint
    let query_tasks_done = "UPDATE tasks SET archived = 1, updated_at = CURRENT_TIMESTAMP WHERE status = 'Done' AND archived = 0 AND sprint_id = ?";
    let _ = sqlx::query(query_tasks_done)
        .bind(&sprint_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    // 3. Find or Create a Planned Sprint for rollover
    let query_planned = "SELECT id FROM sprints WHERE project_id = ? AND status = 'Planned' LIMIT 1";
    let planned_sprint: Option<(String,)> = sqlx::query_as(query_planned)
        .bind(&project_id)
        .fetch_optional(&mut *tx).await.map_err(|e| e.to_string())?;

    let rollover_sprint_id = match planned_sprint {
        Some((id,)) => id,
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            let query_create = "INSERT INTO sprints (id, project_id, status) VALUES (?, ?, 'Planned')";
            let _ = sqlx::query(query_create)
                .bind(&new_id).bind(&project_id)
                .execute(&mut *tx).await.map_err(|e| e.to_string())?;
            new_id
        }
    };

    // 4. Move not-Done tasks to the Planned Sprint (rollover)
    let query_tasks_undone = "UPDATE tasks SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE status != 'Done' AND archived = 0 AND sprint_id = ?";
    let _ = sqlx::query(query_tasks_undone)
        .bind(&rollover_sprint_id).bind(&sprint_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

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
         .execute(&mut *tx).await.map_err(|e| e.to_string())?;
         
    // 6. Move unarchived stories to the Planned Sprint (rollover)
    let query_stories_undone = "UPDATE stories SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE archived = 0 AND sprint_id = ?";
    let _ = sqlx::query(query_stories_undone)
         .bind(&rollover_sprint_id).bind(&sprint_id)
         .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub async fn assign_story_to_sprint(app: AppHandle, story_id: String, sprint_id: Option<String>) -> Result<bool, String> {
    let instances = app.state::<DbInstances>();
    let db_instances = instances.0.read().await;
    let wrapper = db_instances.get(DB_STRING).ok_or("Database instance not found")?;
    
    #[allow(unreachable_patterns)]
    let pool = match wrapper {
        DbPool::Sqlite(p) => p,
        _ => return Err("Not an sqlite database".to_string()),
    };

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

    let q1 = "UPDATE stories SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let _ = sqlx::query(q1)
        .bind(&sprint_id).bind(&story_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;

    let q2 = "UPDATE tasks SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE story_id = ? AND archived = 0";
    let _ = sqlx::query(q2)
        .bind(&sprint_id).bind(&story_id)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
        
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn assign_task_to_sprint(app: AppHandle, task_id: String, sprint_id: Option<String>) -> Result<QueryResult, String> {
    let query = "UPDATE tasks SET sprint_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
    let values = vec![
        sprint_id.map(|s| serde_json::to_value(s).unwrap()).unwrap_or(serde_json::Value::Null),
        serde_json::to_value(task_id).unwrap(),
    ];
    execute_query(&app, query, values).await
}

pub async fn build_project_context(app: &AppHandle, project_id: &str) -> Result<String, String> {
    let query_project = "SELECT * FROM projects WHERE id = ?";
    let projects = select_query::<Project>(app, query_project, vec![serde_json::to_value(project_id).unwrap()]).await?;
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
    let query_sprints = "SELECT * FROM sprints WHERE project_id = ? AND status != 'Completed' ORDER BY status ASC";
    let sprints = select_query::<Sprint>(app, query_sprints, vec![serde_json::to_value(project_id).unwrap()]).await?;

    let active_sprint_id = sprints.iter().find(|s| s.status == "Active").map(|s| s.id.clone());
    let planned_sprint_ids: Vec<String> = sprints.iter().filter(|s| s.status == "Planned").map(|s| s.id.clone()).collect();

    // 3. ストーリーとタスクの取得
    let query_stories = "SELECT * FROM stories WHERE archived = 0 AND project_id = ? ORDER BY created_at ASC";
    let stories = select_query::<Story>(app, query_stories, vec![serde_json::to_value(project_id).unwrap()]).await?;
    
    let query_tasks = "SELECT * FROM tasks WHERE archived = 0 AND project_id = ? ORDER BY created_at ASC";
    let tasks = select_query::<Task>(app, query_tasks, vec![serde_json::to_value(project_id).unwrap()]).await?;
    
    if stories.is_empty() && tasks.is_empty() && md.is_empty() {
        return Ok(String::new());
    }

    // ストーリーとタスクの描画ヘルパー
    let render_stories = |stories_filtered: Vec<&Story>, tasks_all: &[Task]| -> String {
        let mut out = String::new();
        for story in stories_filtered {
            let desc = story.description.clone().unwrap_or_default().replace('\n', " ");
            let short_desc = if desc.chars().count() > 50 {
                format!("{}...", desc.chars().take(50).collect::<String>())
            } else {
                desc
            };
            let desc_str = if short_desc.is_empty() { String::new() } else { format!(": {}", short_desc) };
            out.push_str(&format!("- Story: {}{} (Status: {})\n", story.title, desc_str, story.status));
            
            for task in tasks_all.iter().filter(|t| t.story_id == story.id) {
                let status_icon = match task.status.as_str() {
                    "Done" => " ✅",
                    "In Progress" => " 🔄",
                    _ => "",
                };
                out.push_str(&format!("  - Task: {} (Status: {}){}\n", task.title, task.status, status_icon));
            }
        }
        out
    };

    // 4. プロダクトバックログ（sprint_id IS NULL）
    let backlog_stories: Vec<&Story> = stories.iter().filter(|s| s.sprint_id.is_none()).collect();
    if !backlog_stories.is_empty() {
        md.push_str("\n【プロダクトバックログ（未着手）】\n");
        md.push_str(&render_stories(backlog_stories, &tasks));
    }

    // 5. アクティブスプリント
    if let Some(ref active_id) = active_sprint_id {
        let active_stories: Vec<&Story> = stories.iter().filter(|s| s.sprint_id.as_deref() == Some(active_id.as_str())).collect();
        if !active_stories.is_empty() {
            md.push_str(&format!("\n【アクティブスプリント（進行中）】Sprint ID: {}\n", active_id));
            md.push_str(&render_stories(active_stories, &tasks));
        }
    }

    // 6. 計画中スプリント (Planned)
    for planned_id in &planned_sprint_ids {
        let planned_stories: Vec<&Story> = stories.iter().filter(|s| s.sprint_id.as_deref() == Some(planned_id.as_str())).collect();
        if !planned_stories.is_empty() {
            md.push_str(&format!("\n【計画中スプリント (Planned)】Sprint ID: {}\n", planned_id));
            md.push_str(&render_stories(planned_stories, &tasks));
        }
    }
    
    Ok(md)
}

