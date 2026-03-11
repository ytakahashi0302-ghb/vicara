mod ai;
mod db;
mod inception;
use tauri_plugin_sql::{Builder as SqlBuilder, Migration, MigrationKind};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let migrations = vec![
        Migration {
            version: 1,
            description: "create_initial_tables",
            sql: include_str!("../migrations/1_init.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "add_sprints",
            sql: include_str!("../migrations/2_add_sprints.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "add_projects",
            sql: include_str!("../migrations/3_add_projects.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "add_archived_column",
            sql: include_str!("../migrations/4_add_archived_column.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "add_local_path",
            sql: include_str!("../migrations/5_add_local_path.sql"),
            kind: MigrationKind::Up,
        }
    ];

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            SqlBuilder::default()
                .add_migrations("sqlite:ai-scrum.db", migrations)
                .build(),
        )
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait slightly to ensure DB is initialized
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let _ = db::execute_query(&app_handle, "PRAGMA foreign_keys = ON;", vec![]).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            ai::generate_tasks_from_story,
            ai::refine_idea,
            db::get_projects,
            db::create_project,
            db::update_project,
            db::update_project_path,
            db::delete_project,
            db::get_stories,
            db::get_archived_stories,
            db::add_story,
            db::update_story,
            db::delete_story,
            db::get_tasks,
            db::get_archived_tasks,
            db::get_tasks_by_story_id,
            db::add_task,
            db::update_task_status,
            db::update_task,
            db::delete_task,
            db::get_sprints,
            db::add_sprint,
            db::archive_sprint,
            inception::generate_base_rule,
            inception::read_inception_file,
            inception::write_inception_file,
            ai::chat_inception
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
