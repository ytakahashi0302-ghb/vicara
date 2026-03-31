mod ai;
mod db;
mod inception;
mod pty_commands;
mod pty_manager;
mod rig_provider;
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
        },
        Migration {
            version: 6,
            description: "ai_dev_team",
            sql: include_str!("../migrations/6_ai_dev_team.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "scrum_foundation",
            sql: include_str!("../migrations/7_scrum_foundation.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 8,
            description: "ai_team_leader",
            sql: include_str!("../migrations/8_ai_team_leader.sql"),
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
        .manage(pty_manager::PtyManager::new())
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Wait slightly to ensure DB is initialized
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let _ = db::execute_query(&app_handle, "PRAGMA foreign_keys = ON;", vec![]).await;
            });
            // PTY auto-cleanup: every 5 minutes, kill sessions idle for 30+ minutes
            let cleanup_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use tauri::Manager;
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(5 * 60));
                interval.tick().await; // skip immediate first tick
                loop {
                    interval.tick().await;
                    cleanup_handle
                        .state::<pty_manager::PtyManager>()
                        .cleanup_idle_sessions(std::time::Duration::from_secs(30 * 60))
                        .await;
                }
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
            db::create_planned_sprint,
            db::start_sprint,
            db::complete_sprint,
            db::assign_story_to_sprint,
            db::assign_task_to_sprint,
            inception::generate_base_rule,
            inception::read_inception_file,
            inception::write_inception_file,
            ai::chat_inception,
            db::get_team_chat_messages,
            db::add_team_chat_message,
            db::clear_team_chat_messages,
            ai::chat_with_team_leader,
            pty_commands::pty_spawn,
            pty_commands::pty_execute,
            pty_commands::pty_kill
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
