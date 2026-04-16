use crate::db::{insert_story_with_tasks, StoryDraftInput, TaskDraft};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::fmt;
use tauri::{AppHandle, Emitter};

const STORY_DUPLICATE_SIMILARITY_THRESHOLD: f64 = 0.88;

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

fn normalize_story_title(title: &str) -> String {
    title
        .chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn story_title_bigrams(title: &str) -> HashSet<String> {
    let chars = title.chars().collect::<Vec<_>>();
    match chars.len() {
        0 => HashSet::new(),
        1 => std::iter::once(title.to_string()).collect(),
        _ => chars
            .windows(2)
            .map(|window| window.iter().collect::<String>())
            .collect(),
    }
}

fn story_title_similarity(candidate: &str, existing: &str) -> f64 {
    let candidate = normalize_story_title(candidate);
    let existing = normalize_story_title(existing);

    if candidate.is_empty() || existing.is_empty() {
        return 0.0;
    }
    if candidate == existing {
        return 1.0;
    }

    let shorter_len = candidate.chars().count().min(existing.chars().count());
    if shorter_len >= 6 && (candidate.contains(&existing) || existing.contains(&candidate)) {
        return 0.96;
    }

    let candidate_bigrams = story_title_bigrams(&candidate);
    let existing_bigrams = story_title_bigrams(&existing);
    if candidate_bigrams.is_empty() || existing_bigrams.is_empty() {
        return 0.0;
    }

    let intersection = candidate_bigrams.intersection(&existing_bigrams).count() as f64;
    (2.0 * intersection) / ((candidate_bigrams.len() + existing_bigrams.len()) as f64)
}

fn build_duplicate_story_error(story: &crate::db::Story, similarity: f64) -> String {
    let status_label = if story.archived {
        "Completed / Archived".to_string()
    } else {
        story.status.clone()
    };

    format!(
        "既存 Story と重複する可能性が高いため、新規作成を停止しました。候補: \"{}\" (ID: {}, status: {})。類似度: {:.2}。既存 Story へ task を追加する場合は target_story_id を指定し、完了済み実装の派生作業なら差分が分かるタイトルへ具体化してください。",
        story.title, story.id, status_label, similarity
    )
}

pub async fn guard_story_creation_against_duplicates(
    app: &AppHandle,
    project_id: &str,
    target_story_id: Option<&str>,
    story_title: Option<&str>,
) -> Result<(), String> {
    if target_story_id
        .map(str::trim)
        .map(|value| !value.is_empty())
        .unwrap_or(false)
    {
        return Ok(());
    }

    let Some(candidate_title) = story_title.map(str::trim).filter(|title| !title.is_empty()) else {
        return Ok(());
    };

    let existing_stories = crate::db::select_query::<crate::db::Story>(
        app,
        "SELECT * FROM stories WHERE project_id = ? ORDER BY archived ASC, updated_at DESC, created_at DESC",
        vec![serde_json::to_value(project_id).unwrap()],
    )
    .await?;

    let duplicate = existing_stories
        .into_iter()
        .map(|story| {
            let similarity = story_title_similarity(candidate_title, &story.title);
            (story, similarity)
        })
        .filter(|(_, similarity)| *similarity >= STORY_DUPLICATE_SIMILARITY_THRESHOLD)
        .max_by(|(_, left), (_, right)| {
            left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
        });

    if let Some((story, similarity)) = duplicate {
        return Err(build_duplicate_story_error(&story, similarity));
    }

    Ok(())
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

        guard_story_creation_against_duplicates(
            &self.app,
            &self.project_id,
            args.target_story_id.as_deref(),
            args.story_title.as_deref(),
        )
        .await
        .map_err(CustomToolError)?;

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

// ─── AddProjectNoteTool ───

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AddProjectNoteArgs {
    pub title: String,
    pub content: String,
    pub sprint_id: Option<String>,
}

pub struct AddProjectNoteTool {
    pub app: AppHandle,
    pub project_id: String,
}

impl Tool for AddProjectNoteTool {
    const NAME: &'static str = "add_project_note";

    type Error = CustomToolError;
    type Args = AddProjectNoteArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "会話中に自然と出てきた気づき・懸念・メモを「ふせん」としてボードに残すツール。【重要】ユーザーが「PBIに追加して」「ストーリーを作って」「タスクを登録して」など明示的にバックログ作成を求めた場合は絶対にこのツールを使わず、`create_story_and_tasks` を使うこと。このツールはあくまで会話の副産物として生まれた気づきを記録する補助ツールである。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "ふせんのタイトル（簡潔に）" },
                    "content": { "type": "string", "description": "ふせんの内容（Markdown形式可）" },
                    "sprint_id": { "type": "string", "description": "関連するスプリントID（省略可）", "nullable": true }
                },
                "required": ["title", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        crate::db::add_project_note(
            self.app.clone(),
            self.project_id.clone(),
            args.sprint_id,
            args.title.clone(),
            args.content,
            Some("po_assistant".to_string()),
        )
        .await
        .map_err(|e| CustomToolError(e))?;

        let _ = self.app.emit("kanban-updated", ());
        Ok(format!("ふせん「{}」をボードに追加しました。", args.title))
    }
}

// ─── SuggestRetroItemTool ───

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SuggestRetroItemArgs {
    pub category: String,
    pub content: String,
}

pub struct SuggestRetroItemTool {
    pub app: AppHandle,
    pub project_id: String,
}

impl Tool for SuggestRetroItemTool {
    const NAME: &'static str = "suggest_retro_item";

    type Error = CustomToolError;
    type Args = SuggestRetroItemArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "レトロスペクティブボードにKPTアイテムを提案するツール。会話中に気づいた良かった点(Keep)、問題点(Problem)、改善提案(Try)をレトロボードに追加します。アクティブなレトロセッションが必要です。".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "enum": ["keep", "problem", "try"],
                        "description": "KPTカテゴリ: keep=継続すべき良い取り組み, problem=解決すべき課題, try=次回試したい改善案"
                    },
                    "content": { "type": "string", "description": "アイテムの内容" }
                },
                "required": ["category", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let sessions = crate::db::get_retro_sessions(self.app.clone(), self.project_id.clone())
            .await
            .map_err(|e| CustomToolError(e))?;

        let active_session = sessions
            .iter()
            .find(|s| s.status == "draft" || s.status == "in_progress")
            .ok_or_else(|| {
                CustomToolError(
                    "アクティブなレトロセッションがありません。レトロスペクティブを開始してから再度お試しください。ユーザーにレトロセッションの開始を案内してください。".to_string(),
                )
            })?;

        crate::db::add_retro_item(
            self.app.clone(),
            active_session.id.clone(),
            args.category.clone(),
            args.content.clone(),
            "po".to_string(),
            None,
            None,
        )
        .await
        .map_err(|e| CustomToolError(e))?;

        let _ = self.app.emit("kanban-updated", ());
        let category_label = match args.category.as_str() {
            "keep" => "Keep",
            "problem" => "Problem",
            "try" => "Try",
            _ => &args.category,
        };
        Ok(format!(
            "レトロの {} に「{}」を追加しました。",
            category_label, args.content
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_story_title, story_title_similarity};

    #[test]
    fn normalize_story_title_removes_spacing_and_symbols() {
        assert_eq!(
            normalize_story_title("  DB 一覧表示を追加!! "),
            "db一覧表示を追加"
        );
    }

    #[test]
    fn story_title_similarity_detects_exact_and_near_exact_titles() {
        assert_eq!(story_title_similarity("DB一覧表示", "db 一覧表示"), 1.0);
        assert!(story_title_similarity("ユーザー一覧APIを追加", "ユーザー一覧 API を追加") > 0.9);
        assert!(story_title_similarity("通知設定画面を追加", "売上CSVエクスポート") < 0.5);
    }
}
