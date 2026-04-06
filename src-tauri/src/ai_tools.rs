use crate::db::{insert_story_with_tasks, StoryDraftInput, TaskDraft};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateStoryAndTasksArgs {
    pub target_story_id: Option<String>,
    pub story_title: Option<String>,
    pub story_description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub story_priority: Option<i32>,
    pub tasks: Vec<TaskDraft>,
}

#[derive(Debug)]
pub struct CustomToolError(pub String);

impl fmt::Display for CustomToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tool error: {}", self.0)
    }
}

impl std::error::Error for CustomToolError {}

pub struct CreateStoryAndTasksTool {
    pub app: AppHandle,
    pub project_id: String,
}

#[derive(Debug, Clone, Copy)]
struct ProjectBacklogCounts {
    stories: i64,
    tasks: i64,
    dependencies: i64,
}

async fn get_project_backlog_counts(
    app: &AppHandle,
    project_id: &str,
) -> Result<ProjectBacklogCounts, String> {
    let stories = crate::db::select_query::<(i64,)>(
        app,
        "SELECT COUNT(*) as count FROM stories WHERE project_id = ?",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?
    .first()
    .map(|row| row.0)
    .unwrap_or(0);

    let tasks = crate::db::select_query::<(i64,)>(
        app,
        "SELECT COUNT(*) as count FROM tasks WHERE project_id = ?",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?
    .first()
    .map(|row| row.0)
    .unwrap_or(0);

    let dependencies = crate::db::select_query::<(i64,)>(
        app,
        "SELECT COUNT(*) as count FROM task_dependencies td JOIN tasks t ON td.task_id = t.id WHERE t.project_id = ?",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?
    .first()
    .map(|row| row.0)
    .unwrap_or(0);

    Ok(ProjectBacklogCounts {
        stories,
        tasks,
        dependencies,
    })
}

impl Tool for CreateStoryAndTasksTool {
    const NAME: &'static str = "create_story_and_tasks";

    type Error = CustomToolError;
    type Args = CreateStoryAndTasksArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "カンバンバックログに新しいストーリーとサプタスク群を登録する、または既存のストーリーにタスクを追加するツール。既存のストーリーにタスクを追加する場合は、事前情報から対象のストーリーIDを推測して target_story_id に指定すること。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "target_story_id": {
                        "type": "string",
                        "description": "既存のストーリーにタスクを追加する場合の対象ストーリーID。新規作成の場合は指定しない(null)。",
                        "nullable": true
                    },
                    "story_title": {
                        "type": "string",
                        "description": "新規生成するストーリーの要約タイトル（新規作成時のみ指定）",
                        "nullable": true
                    },
                    "story_description": {
                        "type": "string",
                        "description": "ストーリーの詳細な説明",
                        "nullable": true
                    },
                    "acceptance_criteria": {
                        "type": "string",
                        "description": "ストーリーの受け入れ条件（マークダウンのリスト推奨）",
                        "nullable": true
                    },
                    "story_priority": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 5,
                        "description": "ストーリーの優先度（整数1〜5、小さいほど優先度高）: 1=最重要, 2=高, 3=中(デフォルト), 4=低, 5=最低",
                        "nullable": true
                    },
                    "tasks": {
                        "type": "array",
                        "minItems": 1,
                        "description": "作成するサブタスクのリスト",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "description": { "type": "string", "nullable": true },
                                "priority": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "maximum": 5,
                                    "description": "タスクの優先度（整数1〜5、小さいほど優先度高）: 1=最重要, 2=高, 3=中(デフォルト), 4=低, 5=最低"
                                },
                                "blocked_by_indices": {
                                    "type": "array",
                                    "items": { "type": "integer" },
                                    "description": "このタスクの先行タスクの配列インデックス（0始まり）。依存がなければ省略"
                                }
                            },
                            "required": ["title"]
                        }
                    }
                },
                "required": ["tasks"]
            })
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if args.tasks.is_empty() {
            return Err(CustomToolError(
                "少なくとも1件以上のタスクが必要です。tasks を空配列にせず、作成対象タスクを含めて再実行してください。".to_string(),
            ));
        }

        let before = get_project_backlog_counts(&self.app, &self.project_id)
            .await
            .map_err(CustomToolError)?;

        let story_draft = StoryDraftInput {
            target_story_id: args.target_story_id.clone(),
            title: args
                .story_title
                .clone()
                .unwrap_or_else(|| "Untitled Story".to_string()),
            description: args.story_description.clone(),
            acceptance_criteria: args.acceptance_criteria.clone(),
            priority: args.story_priority.clone(),
        };

        match insert_story_with_tasks(&self.app, &self.project_id, story_draft, args.tasks).await {
            Ok(story_id) => {
                let after = get_project_backlog_counts(&self.app, &self.project_id)
                    .await
                    .map_err(CustomToolError)?;
                let added_stories = after.stories.saturating_sub(before.stories);
                let added_tasks = after.tasks.saturating_sub(before.tasks);
                let added_dependencies = after.dependencies.saturating_sub(before.dependencies);

                if added_tasks <= 0 {
                    return Err(CustomToolError(
                        "ストーリー登録後も tasks テーブルの件数が増えていません。タスク追加は完了していないため、成功として扱えません。".to_string(),
                    ));
                }

                let _ = self.app.emit("kanban-updated", ());
                let target_msg = if let Some(id) = args.target_story_id {
                    format!("既存のストーリー(ID: {})", id)
                } else {
                    format!(
                        "新規ストーリー「{}」(ID: {})",
                        args.story_title.unwrap_or_default(),
                        story_id
                    )
                };

                Ok(format!(
                    "正常に{}へ反映しました。追加結果: stories +{}, tasks +{}, dependencies +{}。この結果だけを根拠にユーザーへ報告してください。",
                    target_msg, added_stories, added_tasks, added_dependencies
                ))
            }
            Err(e) => {
                eprintln!("CreateStoryAndTasksTool Execution Error: {:?}", e);
                Err(CustomToolError(e))
            }
        }
    }
}
