mod ai;
mod ai_tools;
mod claude_runner;
mod db;
mod inception;
mod pty_commands;
mod pty_manager;
mod rig_provider;
mod scaffolding;
pub mod worktree;
use tauri_plugin_sql::{Builder as SqlBuilder, Migration, MigrationKind};

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
        },
        Migration {
            version: 9,
            description: "priority_dependencies",
            sql: include_str!("../migrations/9_priority_dependencies.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 10,
            description: "priority_integer",
            sql: include_str!("../migrations/10_priority_integer.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 11,
            description: "priority_column_to_integer",
            sql: include_str!("../migrations/11_priority_column_to_integer.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 12,
            description: "team_configuration",
            sql: include_str!("../migrations/12_team_configuration.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 13,
            description: "task_role_assignment",
            sql: include_str!("../migrations/13_task_role_assignment.sql"),
            kind: MigrationKind::Up,
        },
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
        .manage(claude_runner::ClaudeState::new())
        .manage(worktree::WorktreeState::new())
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
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(5 * 60));
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
            db::get_team_configuration,
            db::save_team_configuration,
            ai::chat_with_team_leader,
            rig_provider::get_available_models,
            pty_commands::pty_spawn,
            pty_commands::pty_execute,
            pty_commands::pty_kill,
            claude_runner::get_active_claude_sessions,
            claude_runner::execute_claude_task,
            claude_runner::kill_claude_process,
            scaffolding::detect_tech_stack,
            scaffolding::check_scaffold_status,
            scaffolding::execute_scaffold_cli,
            scaffolding::execute_scaffold_ai,
            scaffolding::generate_agent_md,
            scaffolding::generate_claude_settings,
            db::get_all_task_dependencies,
            db::set_task_dependencies,
            worktree::create_worktree,
            worktree::remove_worktree,
            worktree::merge_worktree,
            worktree::get_worktree_status,
            worktree::get_worktree_diff
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
