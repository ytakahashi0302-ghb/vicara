use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneratedTask {
    pub title: String,
    pub description: String,
    pub priority: Option<i32>,
    pub blocked_by_indices: Option<Vec<usize>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoryDraft {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RefinedIdeaResponse {
    pub reply: String,
    pub story_draft: StoryDraft,
}

// generated_document を廃止し patch_target + patch_content 方式に移行
// フロントエンドは patch_target に指定されたファイルへ patch_content を書き込む
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatInceptionResponse {
    pub reply: String,
    pub is_finished: bool,
    pub patch_target: Option<String>, // 書き込み先ファイル名 (e.g. "PRODUCT_CONTEXT.md")
    pub patch_content: Option<String>, // 書き込む内容（そのフェーズの差分のみ）
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTaskResponse {
    pub reply: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TeamLeaderExecutionPlan {
    pub reply: Option<String>,
    pub operations: Vec<crate::ai_tools::CreateStoryAndTasksArgs>,
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

fn looks_like_backlog_mutation_request(message: &str) -> bool {
    let normalized = message.to_lowercase();
    let has_action = [
        "追加", "作成", "登録", "生成", "append", "create", "add", "register",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));
    let has_target = [
        "バックログ",
        "ストーリー",
        "story",
        "stories",
        "タスク",
        "task",
        "tasks",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword));

    has_action && has_target
}

async fn execute_fallback_team_leader_plan(
    app: &AppHandle,
    provider: &crate::rig_provider::AiProvider,
    api_key: &str,
    model: &str,
    project_id: &str,
    context_md: &str,
    user_request: &str,
    before_counts: ProjectBacklogCounts,
) -> Result<Option<ChatTaskResponse>, String> {
    let fallback_system_prompt = format!(
        "あなたはバックログ登録計画を JSON で返すプランナーです。ユーザー依頼に対して、実行すべき `create_story_and_tasks` 相当の操作を JSON のみで返してください。\n\nルール:\n- 既存ストーリーにタスクを追加する場合は、必ず context 内に存在する story ID を `target_story_id` に設定する\n- 新規ストーリーを作る場合のみ `target_story_id` を null にし、`story_title` を必須で入れる\n- `tasks` は必ず1件以上含める\n- 各 task には `title`, `description`, `priority`, `blocked_by_indices` を入れる\n- priority は整数 1〜5\n- 実行不要なら `operations` は空配列にする\n- 出力は必ず JSON オブジェクトのみ\n\n返却形式:\n{{\"reply\":\"ユーザー向け要約\",\"operations\":[{{\"target_story_id\":null,\"story_title\":\"...\",\"story_description\":\"...\",\"acceptance_criteria\":\"...\",\"story_priority\":3,\"tasks\":[{{\"title\":\"...\",\"description\":\"...\",\"priority\":2,\"blocked_by_indices\":[0]}}]}}]}}\n\n【既存バックログ】\n{}",
        context_md
    );

    let raw_plan = crate::rig_provider::chat_with_history(
        provider,
        api_key,
        model,
        &fallback_system_prompt,
        user_request,
        vec![],
    )
    .await?;

    let re = regex::Regex::new(r"(?s)\{.*\}").map_err(|e| e.to_string())?;
    let json_str = if let Some(caps) = re.captures(&raw_plan) {
        caps.get(0).map(|m| m.as_str()).unwrap_or(raw_plan.as_str())
    } else {
        raw_plan.as_str()
    };

    let plan: TeamLeaderExecutionPlan = match serde_json::from_str(json_str) {
        Ok(plan) => plan,
        Err(_) => return Ok(None),
    };

    if plan.operations.is_empty() {
        return Ok(None);
    }

    for operation in plan.operations {
        if operation.tasks.is_empty() {
            continue;
        }

        let story_draft = crate::db::StoryDraftInput {
            target_story_id: operation.target_story_id.clone(),
            title: operation
                .story_title
                .clone()
                .unwrap_or_else(|| "Untitled Story".to_string()),
            description: operation.story_description.clone(),
            acceptance_criteria: operation.acceptance_criteria.clone(),
            priority: operation.story_priority,
        };

        crate::db::insert_story_with_tasks(app, project_id, story_draft, operation.tasks).await?;
    }

    let after_counts = get_project_backlog_counts(app, project_id).await?;
    let added_stories = after_counts.stories.saturating_sub(before_counts.stories);
    let added_tasks = after_counts.tasks.saturating_sub(before_counts.tasks);
    let added_dependencies = after_counts
        .dependencies
        .saturating_sub(before_counts.dependencies);

    if added_stories == 0 && added_tasks == 0 && added_dependencies == 0 {
        return Ok(None);
    }

    let _ = app.emit("kanban-updated", ());

    let reply_prefix = plan
        .reply
        .unwrap_or_else(|| "バックログ登録を実行しました。".to_string());
    Ok(Some(ChatTaskResponse {
        reply: format!(
            "{}\n\n追加結果: stories +{}, tasks +{}, dependencies +{}",
            reply_prefix, added_stories, added_tasks, added_dependencies
        ),
    }))
}

#[tauri::command]
pub async fn generate_tasks_from_story(
    app: AppHandle,
    title: String,
    description: String,
    acceptance_criteria: String,
    provider: String,
    project_id: String,
) -> Result<Vec<GeneratedTask>, String> {
    let (provider_enum, api_key, model) =
        crate::rig_provider::resolve_provider_and_key(&app, Some(provider)).await?;
    let _context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let prompt = format!(
        "Context: {}\nStory: {}\nDesc: {}\nAC: {}\nJSON Array Output Please.",
        _context_md, title, description, acceptance_criteria
    );

    let system_prompt = r#"You are a task decomposition expert for agile software development.
Given a user story, generate a JSON array of subtasks. Each task object must include:
- "title": string (concise, action-oriented)
- "description": string (implementation details)
- "priority": integer 1-5 (REQUIRED; lower number = higher priority)
- "blocked_by_indices": number[] (zero-based indices of prerequisite tasks in this array; omit or use [] if none)

Priority guidelines (integer 1-5, lower = more urgent):
- 1: Most critical — architecture foundation, blocking everything else
- 2: High priority — core functionality on the critical path
- 3: Medium — important feature work, not blocking others (default)
- 4: Low — supporting tasks, tests, minor improvements
- 5: Lowest — documentation, polish, optional enhancements

Dependency guidelines:
- Use blocked_by_indices to express "this task cannot start until task N is done"
- Example: If task[2] requires the API from task[0], set task[2].blocked_by_indices = [0]
- Keep dependency chains short and avoid circular references

Output ONLY a valid JSON array, no explanation or markdown."#;
    let response = crate::rig_provider::chat_with_history(
        &provider_enum,
        &api_key,
        &model,
        system_prompt,
        &prompt,
        vec![],
    )
    .await?;

    let re = regex::Regex::new(r"(?s)\[.*?\]").map_err(|e| e.to_string())?;
    let json_str = re
        .captures(&response)
        .and_then(|caps| caps.get(0))
        .map_or(response.as_str(), |m| m.as_str());
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refine_idea(
    app: AppHandle,
    idea_seed: String,
    previous_context: Option<Vec<Message>>,
    project_id: String,
) -> Result<RefinedIdeaResponse, String> {
    let (provider, api_key, model) =
        crate::rig_provider::resolve_provider_and_key(&app, None).await?;
    let _context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();

    let chat_history = if let Some(ctx) = previous_context {
        crate::rig_provider::convert_messages(&ctx)
    } else {
        vec![]
    };

    let system_prompt = "PO Assist";
    let content = crate::rig_provider::chat_with_history(
        &provider,
        &api_key,
        &model,
        system_prompt,
        &idea_seed,
        chat_history,
    )
    .await?;

    let re = regex::Regex::new(r"(?s)\{.*?\}").unwrap();
    let json_str = re
        .captures(&content)
        .and_then(|caps| caps.get(0))
        .map_or(content.as_str(), |m| m.as_str());
    serde_json::from_str(json_str).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Inception Deck システムプロンプト構築
// 各フェーズで「何をヒアリングし、どのファイルの差分を生成するか」を定義する
// ---------------------------------------------------------------------------
fn build_inception_system_prompt(phase: u32, context_md: &str) -> String {
    let phase_instruction = match phase {
        1 => {
            r#"## Phase 1: コア価値とターゲット (Why)

**ヒアリング目標** (2〜3往復で引き出す):
- 解決したい課題 / ターゲットユーザー / コアバリュー / プロダクトの目的

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (新規作成)
**出力テンプレート** — 箇条書き・20行以内で厳守:
```
# PRODUCT_CONTEXT.md — {プロダクト名}
> 【AIへの絶対指示】本ファイルはシステムプロンプトとして機能する。

## 0. 課題とコアバリュー
- 課題: {1行}
- 解決策: {1行}

## 1. プロダクト定義
- 対象: {ターゲット}
- 目標: {目標}

## 2. 役割分担
- 人間(PO): What の意思決定のみ
- AI: How の実行（タスク分解・実装・改善）
```"#
        }

        2 => {
            r#"## Phase 2: やらないことリスト (Not List)

**ヒアリング目標** (2〜3往復):
- スコープ外にすること / 絶対やってはならないこと

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (末尾に追記)
**追記テンプレート** — Section 3〜5のみ・15行以内:
```
## 3. 運用ルール
- {スプリント方針を1行}

## 4. やらないこと (Not To Do)
- {項目1}
- {項目2}

## 5. コンテキスト管理
- Layer 1 (本ファイル + Rule.md): 不変のコア原則
- Layer 2 (handoff.md): スプリントごとの揮発性コンテキスト
```"#
        }

        3 => {
            r#"## Phase 3: 技術スタック・アーキテクチャ (What)

**ヒアリング目標** (2〜3往復):
- 言語 / FW / DB / アーキテクチャ上の制約

**生成ファイル**: patch_target = "ARCHITECTURE.md" (新規作成)
**出力テンプレート** — 全体20行以内・箇条書きのみ:
```
# ARCHITECTURE.md — {プロダクト名}
> 技術水準と設計方針のまとめ

## 技術スタック
- 言語: {選定}
- FW: {選定}
- DB: {選定}

## アーキテクチャ方針
- {方針1}
- {方針2}

## 設計の制約
- {注意点}
```"#
        }

        4 => {
            r#"## Phase 4: 開発ルール・AIルール (How)

**ヒアリング目標** (1〜2往復):
- このプロダクト固有のコーディング規約 / AIへの特別指示

**生成ファイル**: patch_target = "Rule.md" (末尾に追記)
**追記テンプレート** — 既存内容は絶対に含めない・15行以内:
```
---
## {プロダクト名} 固有ルール

### 技術スタック固有の規約
- {規約1}

### AIへの追加指示
- {追加ルール1}
```"#
        }

        _ => "全フェーズ完了。ユーザーにお祝いの言葉を伝えてください。",
    };

    // 既存ドキュメントは先頭400文字のみを参考情報として渡す（転記禁止）
    let existing_docs = if context_md.is_empty() {
        "（生成済みドキュメントなし）".to_string()
    } else {
        let preview: String = context_md.chars().take(400).collect();
        let suffix = if context_md.chars().count() > 400 {
            "...(省略)"
        } else {
            ""
        };
        format!(
            "【既存ドキュメント概要（参考のみ・このフェーズ以外の内容を再出力しないこと）】\n{}{}",
            preview, suffix
        )
    };

    format!(
        r#"あなたは「Inception Deckファシリテーター」です。

## 役割
ユーザーのプロダクト構想をヒアリングし、Markdownドキュメントとして策定することが唯一の仕事。

## 絶対禁止
- コード・実装手順の提案（例: Pythonコード、コマンド等）
- 「作り方」を教えること（あなたは企画コンサルであり、エンジニアではない）
- 他フェーズで生成済みのドキュメント内容を patch_content に含めること

## 出力品質規約（厳守）
- **箇条書きのみ** — 長文解説・説明・挨拶は不要
- **1項目1行** — 無駄な装飾を省く
- **patch_content は20行以内** — トークン節約が最優先
- **reply は1文のみ** — 例:「PRODUCT_CONTEXT.md を生成しました」

{phase_instruction}

{existing_docs}

## 出力フォーマット（必ずこの形式のJSONのみを返すこと）

ヒアリング中:
{{"reply": "質問（1文）", "is_finished": false, "patch_target": null, "patch_content": null}}

ドキュメント生成時:
{{"reply": "〜を生成しました。", "is_finished": true, "patch_target": "ファイル名.md", "patch_content": "Markdownの差分（20行以内）"}}

patch_content にはこのフェーズで追加する部分のみを含め、他フェーズの内容は絶対に含めないこと。"#,
        phase_instruction = phase_instruction,
        existing_docs = existing_docs,
    )
}

#[tauri::command]
pub async fn chat_inception(
    app: AppHandle,
    project_id: String,
    phase: u32,
    messages_history: Vec<Message>,
) -> Result<ChatInceptionResponse, String> {
    let (provider, api_key, model) =
        crate::rig_provider::resolve_provider_and_key(&app, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();

    let chat_history = crate::rig_provider::convert_messages(&messages_history);
    let system_prompt = build_inception_system_prompt(phase, &context_md);

    let content = crate::rig_provider::chat_with_history(
        &provider,
        &api_key,
        &model,
        &system_prompt,
        "",
        chat_history,
    )
    .await?;

    // Markdownコードフェンス (```json ... ```) を除去してからパース
    let stripped = {
        let fence_re = regex::Regex::new(r"(?s)```(?:json)?\s*\n?(.*?)\n?\s*```").unwrap();
        if let Some(caps) = fence_re.captures(&content) {
            caps.get(1).unwrap().as_str().to_string()
        } else {
            content.clone()
        }
    };

    // Greedy match でネストした JSON を正確に抽出
    let re = regex::Regex::new(r"(?s)\{.*\}").unwrap();
    let json_str = if let Some(caps) = re.captures(&stripped) {
        caps.get(0).unwrap().as_str()
    } else {
        &stripped
    };

    let resp: ChatInceptionResponse = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(_) => ChatInceptionResponse {
            reply: content,
            is_finished: false,
            patch_target: None,
            patch_content: None,
        },
    };

    Ok(resp)
}

#[tauri::command]
pub async fn chat_with_team_leader(
    app: AppHandle,
    project_id: String,
    messages_history: Vec<Message>,
) -> Result<ChatTaskResponse, String> {
    let (provider, api_key, model) =
        crate::rig_provider::resolve_provider_and_key(&app, None).await?;
    let _context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let before_counts = get_project_backlog_counts(&app, &project_id).await?;
    let latest_user_index = messages_history
        .iter()
        .rposition(|message| message.role == "user");
    let (latest_user_message, prior_messages) = if let Some(index) = latest_user_index {
        let latest = messages_history[index].content.clone();
        let prior = messages_history[..index].to_vec();
        (latest, prior)
    } else {
        (String::new(), messages_history.clone())
    };

    let chat_history = crate::rig_provider::convert_messages(&prior_messages);
    let system_prompt = format!(
        "あなたはScrum TeamのAI Team Leaderです。ユーザーから機能要件や追加タスクの要望があった場合、自身が持つツール (`create_story_and_tasks`) を必ず呼び出して、ストーリーとサブタスク群をデータベースに自動登録してください。\n\n【最重要ルール】\n- ユーザーがストーリーやタスクの作成・追加・登録を求めた場合、説明だけで終わらせず `create_story_and_tasks` を使うこと\n- 既存ストーリーにタスクを追加する依頼では、コンテキスト中の story ID を読んで `target_story_id` を必ず指定すること\n- ツールを呼んでいないのに「追加しました」「登録しました」と断定してはいけない\n- ツールが失敗した場合は、成功を装わずエラー内容を簡潔に伝えること\n\n【現在のプロダクトの状況（既存バックログ等）】\n{}\n\n【優先度と依存関係の設定ルール】\nストーリーとタスクを作成する際は、必ず以下のフィールドを設定してください：\n- story_priority: 整数 1〜5（小さいほど優先度が高い）\n- 各タスクの priority: 整数 1〜5（小さいほど優先度が高い）\n- 各タスクの blocked_by_indices: 先行タスクの配列インデックス（0始まり）を指定。依存がなければ省略か空配列\n\n優先度の判断基準（1〜5、数値が小さいほど重要）:\n- 1: 最重要 — アーキテクチャの根幹、他の全タスクをブロックする基盤作業\n- 2: 高優先 — クリティカルパス上のコア機能\n- 3: 中優先 — 重要な機能実装だが他をブロックしない（デフォルト）\n- 4: 低優先 — サポートタスク、テスト、軽微な改善\n- 5: 最低優先 — ドキュメント、UIの微調整、オプション機能\n\n【重要】ツール実行に失敗した場合は、エラー内容を確認して原因をユーザーに報告、または代替策を考えてください。ツールが失敗したからといって、決してユーザーに手動での登録作業を丸投げしないでください。\n\n会話の返答は必ず以下の形式のJSONオブジェクトのみで返してください。\n\n{{\"reply\": \"ツール実行結果やユーザーへのメッセージ内容\"}}",
        _context_md
    );

    let raw_text = crate::rig_provider::chat_team_leader_with_tools(
        &app,
        &provider,
        &api_key,
        &model,
        &system_prompt,
        &latest_user_message,
        chat_history,
        &project_id,
    )
    .await?;
    let after_counts = get_project_backlog_counts(&app, &project_id).await?;
    let mutation_requested = looks_like_backlog_mutation_request(&latest_user_message);
    let data_changed = before_counts.stories != after_counts.stories
        || before_counts.tasks != after_counts.tasks
        || before_counts.dependencies != after_counts.dependencies;

    if mutation_requested && !data_changed {
        if let Some(fallback_response) = execute_fallback_team_leader_plan(
            &app,
            &provider,
            &api_key,
            &model,
            &project_id,
            &_context_md,
            &latest_user_message,
            before_counts,
        )
        .await?
        {
            return Ok(fallback_response);
        }

        return Ok(ChatTaskResponse {
            reply: "登録・追加系の依頼として解釈しましたが、実際にはバックログの件数変化を確認できませんでした。今回は成功扱いにせず停止します。`create_story_and_tasks` の未実行または失敗が疑われるため、再試行時は対象ストーリーIDを明示して実行してください。".to_string(),
        });
    }

    let re = regex::Regex::new(r"(?s)\{.*?\}").unwrap();
    let json_str = if let Some(caps) = re.captures(&raw_text) {
        caps.get(0).unwrap().as_str()
    } else {
        &raw_text
    };

    let resp: ChatTaskResponse = match serde_json::from_str(json_str) {
        Ok(r) => r,
        Err(_) => ChatTaskResponse { reply: raw_text },
    };

    Ok(resp)
}
