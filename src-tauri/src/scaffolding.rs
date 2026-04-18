use std::path::{Component, Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tauri_plugin_store::StoreExt;

use crate::{
    node_dependencies::{install_node_dependencies, NodeInstallOutputTarget},
    pty_manager::PtyManager,
};

// ---------------------------------------------------------------------------
// 型定義
// ---------------------------------------------------------------------------

/// 技術スタック検出結果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TechStackInfo {
    pub language: Option<String>,
    pub framework: Option<String>,
    pub meta_framework: Option<String>,
    pub raw_content: String,
}

/// スキャフォールド戦略
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ScaffoldStrategy {
    CliScaffold { command: String, args: Vec<String> },
    AiGenerated { prompt: String },
}

/// 技術スタック検出 + 戦略の組み合わせ
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TechStackDetection {
    pub tech_stack: TechStackInfo,
    pub strategy: ScaffoldStrategy,
}

/// スキャフォールド状態チェック結果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScaffoldStatus {
    pub has_agent_md: bool,
    pub has_claude_settings: bool,
    pub has_extra_files: bool,
    pub extra_files: Vec<String>,
}

/// スキャフォールドイベントペイロード
#[derive(Clone, serde::Serialize)]
struct ScaffoldOutputPayload {
    output: String,
}

#[derive(Clone, serde::Serialize)]
struct ScaffoldExitPayload {
    success: bool,
    reason: String,
}

#[derive(Debug, Clone)]
enum ScaffoldAiTransport {
    Cli {
        cli_type: crate::cli_runner::CliType,
        model: String,
        cwd: String,
    },
    Api {
        provider: crate::rig_provider::AiProvider,
        api_key: String,
        model: String,
    },
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ApiScaffoldPlan {
    #[serde(default)]
    directories: Vec<String>,
    #[serde(default)]
    files: Vec<ApiScaffoldFile>,
    #[serde(default)]
    summary: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ApiScaffoldFile {
    path: String,
    content: String,
}

// ---------------------------------------------------------------------------
// Inception ファイル以外を検出するためのフィルタ
// ---------------------------------------------------------------------------

const INCEPTION_FILES: &[&str] = &[
    "PRODUCT_CONTEXT.md",
    "ARCHITECTURE.md",
    "Rule.md",
    "AGENT.md",
    ".claude",
];

const PO_ASSISTANT_TRANSPORT_KEY: &str = "po-assistant-transport";
const PO_ASSISTANT_CLI_TYPE_KEY: &str = "po-assistant-cli-type";
const PO_ASSISTANT_CLI_MODEL_KEY: &str = "po-assistant-cli-model";

fn is_inception_or_scaffold_file(name: &str) -> bool {
    INCEPTION_FILES.iter().any(|f| name == *f)
}

fn extract_store_string_value(value: serde_json::Value) -> Option<String> {
    if let Some(obj) = value.as_object() {
        obj.get("value")
            .and_then(|inner| inner.as_str())
            .map(|inner| inner.to_string())
    } else {
        value.as_str().map(|inner| inner.to_string())
    }
}

fn emit_scaffold_output(app_handle: &AppHandle, output: impl Into<String>) {
    let _ = app_handle.emit(
        "scaffold_output",
        ScaffoldOutputPayload {
            output: output.into(),
        },
    );
}

fn emit_scaffold_exit(app_handle: &AppHandle, success: bool, reason: impl Into<String>) {
    let _ = app_handle.emit(
        "scaffold_exit",
        ScaffoldExitPayload {
            success,
            reason: reason.into(),
        },
    );
}

fn scaffold_provider_label(provider: &crate::rig_provider::AiProvider) -> &'static str {
    match provider {
        crate::rig_provider::AiProvider::Anthropic => "anthropic",
        crate::rig_provider::AiProvider::Gemini => "gemini",
        crate::rig_provider::AiProvider::OpenAI => "openai",
        crate::rig_provider::AiProvider::Ollama => "ollama",
    }
}

async fn resolve_scaffold_ai_transport(
    app_handle: &AppHandle,
    local_path: &str,
) -> Result<ScaffoldAiTransport, String> {
    let store = app_handle
        .store("settings.json")
        .map_err(|e| format!("Failed to access store: {}", e))?;
    let transport_kind = store
        .get(PO_ASSISTANT_TRANSPORT_KEY)
        .and_then(extract_store_string_value)
        .unwrap_or_else(|| "api".to_string());

    if transport_kind.trim().eq_ignore_ascii_case("cli") {
        let cli_type = crate::cli_runner::CliType::from_str(
            &store
                .get(PO_ASSISTANT_CLI_TYPE_KEY)
                .and_then(extract_store_string_value)
                .unwrap_or_else(|| "claude".to_string()),
        );
        let runner = crate::cli_runner::create_runner(&cli_type)?;
        let model = runner.resolve_model(
            &store
                .get(PO_ASSISTANT_CLI_MODEL_KEY)
                .and_then(extract_store_string_value)
                .unwrap_or_default(),
        );

        Ok(ScaffoldAiTransport::Cli {
            cli_type,
            model,
            cwd: local_path.to_string(),
        })
    } else {
        let (provider, api_key, model) =
            crate::rig_provider::resolve_provider_and_key(app_handle, None).await?;
        Ok(ScaffoldAiTransport::Api {
            provider,
            api_key,
            model,
        })
    }
}

fn strip_json_code_fence(content: &str) -> String {
    let trimmed = content.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let mut lines = trimmed.lines();
    let _ = lines.next();
    let mut body = lines.collect::<Vec<_>>();
    if body
        .last()
        .map(|line| line.trim() == "```")
        .unwrap_or(false)
    {
        body.pop();
    }
    body.join("\n").trim().to_string()
}

async fn install_scaffold_node_dependencies(
    app_handle: &AppHandle,
    project_root: &Path,
) -> Result<Vec<String>, String> {
    install_node_dependencies(
        app_handle,
        project_root,
        NodeInstallOutputTarget::Scaffold,
        "Scaffolding 後",
    )
    .await
}

fn parse_api_scaffold_plan(content: &str) -> Result<ApiScaffoldPlan, String> {
    let normalized = strip_json_code_fence(content);
    serde_json::from_str::<ApiScaffoldPlan>(&normalized).map_err(|error| {
        format!(
            "AI スキャフォールド応答を JSON として解釈できませんでした: {}",
            error
        )
    })
}

fn normalize_scaffold_relative_path(relative_path: &str) -> Result<PathBuf, String> {
    let candidate = relative_path.trim();
    if candidate.is_empty() {
        return Err("空のパスは使用できません。".to_string());
    }

    let candidate = candidate.replace('\\', "/");
    let path = Path::new(&candidate);
    if path.is_absolute() {
        return Err(format!("絶対パスは使用できません: {}", relative_path));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!(
                    "親ディレクトリ参照を含むパスは使用できません: {}",
                    relative_path
                ));
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(format!("無効なパスです: {}", relative_path));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(format!("無効なパスです: {}", relative_path));
    }

    Ok(normalized)
}

fn is_reserved_scaffold_target(relative_path: &Path) -> bool {
    let normalized = relative_path
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();

    let top_level = normalized.split('/').next().unwrap_or_default();
    matches!(
        normalized.as_str(),
        "PRODUCT_CONTEXT.md" | "ARCHITECTURE.md" | "Rule.md" | "AGENT.md"
    ) || top_level.eq_ignore_ascii_case(".claude")
}

fn resolve_scaffold_target_path(base_dir: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let normalized = normalize_scaffold_relative_path(relative_path)?;
    if is_reserved_scaffold_target(&normalized) {
        return Err(format!(
            "Scaffolding では保護対象ファイルを変更できません: {}",
            normalized.display()
        ));
    }

    Ok(base_dir.join(normalized))
}

async fn record_scaffold_provider_usage(
    app_handle: &AppHandle,
    project_id: Option<&str>,
    response: &crate::rig_provider::LlmTextResponse,
) {
    let Some(project_id) = project_id else {
        return;
    };

    if let Err(error) = crate::llm_observability::record_llm_usage(
        app_handle,
        crate::llm_observability::RecordLlmUsageInput {
            project_id: project_id.to_string(),
            task_id: None,
            sprint_id: None,
            source_kind: "scaffold_ai".to_string(),
            transport_kind: "provider_api".to_string(),
            provider: response.provider.clone(),
            model: response.model.clone(),
            usage: response.usage,
            measurement_status: None,
            request_started_at: Some(response.started_at),
            request_completed_at: Some(response.completed_at),
            success: true,
            error_message: None,
            raw_usage_json: Some(response.raw_usage_json.clone()),
        },
    )
    .await
    {
        log::warn!(
            "Failed to record scaffold provider usage for project_id={}: {}",
            project_id,
            error
        );
    }
}

fn apply_api_scaffold_plan(
    app_handle: &AppHandle,
    base_dir: &Path,
    plan: ApiScaffoldPlan,
) -> Result<String, String> {
    let mut created_dirs = 0usize;
    let mut created_files = 0usize;
    let mut skipped_files = 0usize;

    for directory in plan.directories {
        let target_dir = resolve_scaffold_target_path(base_dir, &directory)?;
        if target_dir.exists() {
            continue;
        }
        std::fs::create_dir_all(&target_dir).map_err(|error| {
            format!(
                "ディレクトリ作成に失敗しました ({}): {}",
                target_dir.display(),
                error
            )
        })?;
        created_dirs += 1;
        emit_scaffold_output(
            app_handle,
            format!(
                "created dir: {}",
                target_dir
                    .strip_prefix(base_dir)
                    .unwrap_or(&target_dir)
                    .display()
            ),
        );
    }

    for file in plan.files {
        let target_file = resolve_scaffold_target_path(base_dir, &file.path)?;
        if target_file.exists() {
            skipped_files += 1;
            emit_scaffold_output(
                app_handle,
                format!(
                    "skip existing file: {}",
                    target_file
                        .strip_prefix(base_dir)
                        .unwrap_or(&target_file)
                        .display()
                ),
            );
            continue;
        }

        if let Some(parent) = target_file.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "親ディレクトリ作成に失敗しました ({}): {}",
                    parent.display(),
                    error
                )
            })?;
        }

        std::fs::write(&target_file, file.content).map_err(|error| {
            format!(
                "ファイル作成に失敗しました ({}): {}",
                target_file.display(),
                error
            )
        })?;
        created_files += 1;
        emit_scaffold_output(
            app_handle,
            format!(
                "created file: {}",
                target_file
                    .strip_prefix(base_dir)
                    .unwrap_or(&target_file)
                    .display()
            ),
        );
    }

    if created_dirs == 0 && created_files == 0 {
        return Err(
            "AI スキャフォールド結果から新規ディレクトリ・ファイルを作成できませんでした。"
                .to_string(),
        );
    }

    let mut summary = format!(
        "AI スキャフォールド完了: dirs +{}, files +{}",
        created_dirs, created_files
    );
    if skipped_files > 0 {
        summary.push_str(&format!(", skipped {}", skipped_files));
    }
    if !plan.summary.trim().is_empty() {
        summary.push_str(&format!(" / {}", plan.summary.trim()));
    }

    Ok(summary)
}

fn scaffold_cli_requires_temporary_workspace(args: &[String]) -> bool {
    args.iter().any(|arg| arg.trim() == ".")
}

fn create_cli_scaffold_temp_project_dir(local_path: &Path) -> Result<(PathBuf, PathBuf), String> {
    let temp_root = std::env::temp_dir().join(format!("vicara-scaffold-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_root).map_err(|error| {
        format!(
            "一時ディレクトリの作成に失敗しました ({}): {}",
            temp_root.display(),
            error
        )
    })?;
    let project_dir_name = local_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "project".to_string());
    let temp_project_dir = temp_root.join(project_dir_name);
    std::fs::create_dir_all(&temp_project_dir).map_err(|error| {
        format!(
            "一時プロジェクトディレクトリの作成に失敗しました ({}): {}",
            temp_project_dir.display(),
            error
        )
    })?;

    Ok((temp_root, temp_project_dir))
}

fn copy_scaffold_entry(source: &Path, destination: &Path) -> Result<(), String> {
    if source.is_dir() {
        std::fs::create_dir_all(destination).map_err(|error| {
            format!(
                "ディレクトリ作成に失敗しました ({}): {}",
                destination.display(),
                error
            )
        })?;

        for entry in std::fs::read_dir(source).map_err(|error| {
            format!(
                "ディレクトリの読み取りに失敗しました ({}): {}",
                source.display(),
                error
            )
        })? {
            let entry = entry.map_err(|error| {
                format!(
                    "ディレクトリエントリの読み取りに失敗しました ({}): {}",
                    source.display(),
                    error
                )
            })?;
            let child_source = entry.path();
            let child_destination = destination.join(entry.file_name());
            copy_scaffold_entry(&child_source, &child_destination)?;
        }
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "親ディレクトリの作成に失敗しました ({}): {}",
                parent.display(),
                error
            )
        })?;
    }

    std::fs::copy(source, destination).map_err(|error| {
        format!(
            "ファイルコピーに失敗しました ({} -> {}): {}",
            source.display(),
            destination.display(),
            error
        )
    })?;

    Ok(())
}

fn import_cli_scaffold_output(
    generated_root: &Path,
    project_root: &Path,
) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(generated_root).map_err(|error| {
        format!(
            "CLI スキャフォールド結果の読み取りに失敗しました ({}): {}",
            generated_root.display(),
            error
        )
    })? {
        let entry = entry.map_err(|error| {
            format!(
                "CLI スキャフォールド結果のエントリ読み取りに失敗しました ({}): {}",
                generated_root.display(),
                error
            )
        })?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name == ".git" {
            continue;
        }
        entries.push((file_name, entry.path()));
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    if entries.is_empty() {
        return Err("CLI スキャフォールド結果に取り込めるファイルがありませんでした。".to_string());
    }

    let conflicts: Vec<String> = entries
        .iter()
        .filter_map(|(file_name, _)| {
            let target = project_root.join(file_name);
            target.exists().then(|| file_name.clone())
        })
        .collect();
    if !conflicts.is_empty() {
        return Err(format!(
            "CLI スキャフォールド結果を取り込めませんでした。既存のファイル/ディレクトリと衝突しています: {}",
            conflicts.join(", ")
        ));
    }

    let mut imported_entries = Vec::new();
    for (file_name, source_path) in entries {
        let destination_path = project_root.join(&file_name);
        copy_scaffold_entry(&source_path, &destination_path)?;
        imported_entries.push(file_name);
    }

    Ok(imported_entries)
}

// ---------------------------------------------------------------------------
// 技術スタック検出
// ---------------------------------------------------------------------------

fn detect_stack_from_content(content: &str) -> TechStackInfo {
    let lower = content.to_lowercase();

    let language = if lower.contains("typescript") || lower.contains("ts") {
        Some("TypeScript".to_string())
    } else if lower.contains("javascript") || lower.contains("js") {
        Some("JavaScript".to_string())
    } else if lower.contains("rust") {
        Some("Rust".to_string())
    } else if lower.contains("python") {
        Some("Python".to_string())
    } else if lower.contains("go") && (lower.contains("golang") || lower.contains("go ")) {
        Some("Go".to_string())
    } else {
        None
    };

    let framework = if lower.contains("react") {
        Some("React".to_string())
    } else if lower.contains("vue") {
        Some("Vue".to_string())
    } else if lower.contains("svelte") {
        Some("Svelte".to_string())
    } else if lower.contains("fastapi") {
        Some("FastAPI".to_string())
    } else if lower.contains("express") {
        Some("Express".to_string())
    } else {
        None
    };

    let meta_framework =
        if lower.contains("next.js") || lower.contains("nextjs") || lower.contains("next js") {
            Some("Next.js".to_string())
        } else if lower.contains("nuxt") {
            Some("Nuxt".to_string())
        } else if lower.contains("vite") {
            Some("Vite".to_string())
        } else if lower.contains("tauri") {
            Some("Tauri".to_string())
        } else {
            None
        };

    TechStackInfo {
        language,
        framework,
        meta_framework,
        raw_content: content.to_string(),
    }
}

fn determine_strategy(stack: &TechStackInfo) -> ScaffoldStrategy {
    // メタフレームワーク優先
    if let Some(meta) = &stack.meta_framework {
        match meta.as_str() {
            "Next.js" => {
                return ScaffoldStrategy::CliScaffold {
                    command: "npx".to_string(),
                    args: vec![
                        "create-next-app@latest".to_string(),
                        ".".to_string(),
                        "--ts".to_string(),
                        "--app".to_string(),
                        "--use-npm".to_string(),
                        "--yes".to_string(),
                        "--skip-install".to_string(),
                        "--disable-git".to_string(),
                    ],
                };
            }
            "Nuxt" => {
                return ScaffoldStrategy::CliScaffold {
                    command: "npx".to_string(),
                    args: vec![
                        "nuxi@latest".to_string(),
                        "init".to_string(),
                        ".".to_string(),
                    ],
                };
            }
            "Vite" => {
                let template = match stack.framework.as_deref() {
                    Some("Vue") => "vue-ts",
                    Some("Svelte") => "svelte-ts",
                    _ => "react-ts", // デフォルトは React
                };
                return ScaffoldStrategy::CliScaffold {
                    command: "npx".to_string(),
                    args: vec![
                        "create-vite@latest".to_string(),
                        ".".to_string(),
                        "--template".to_string(),
                        template.to_string(),
                    ],
                };
            }
            "Tauri" => {
                return ScaffoldStrategy::CliScaffold {
                    command: "npx".to_string(),
                    args: vec!["create-tauri-app@latest".to_string(), ".".to_string()],
                };
            }
            _ => {}
        }
    }

    // フレームワーク + 言語ベースのフォールバック
    if let Some(lang) = &stack.language {
        match lang.as_str() {
            "Rust" => {
                return ScaffoldStrategy::CliScaffold {
                    command: "cargo".to_string(),
                    args: vec!["init".to_string(), ".".to_string()],
                };
            }
            _ => {}
        }
    }

    // どれにも該当しない場合は AI 生成
    ScaffoldStrategy::AiGenerated {
        prompt: build_ai_scaffold_prompt(stack),
    }
}

fn build_ai_scaffold_prompt(stack: &TechStackInfo) -> String {
    let lang = stack.language.as_deref().unwrap_or("不明");
    let fw = stack.framework.as_deref().unwrap_or("なし");
    let meta = stack.meta_framework.as_deref().unwrap_or("なし");

    format!(
        r#"以下の技術スタックに基づいて、プロジェクトの初期ディレクトリ構造を作成してください。

言語: {}
フレームワーク: {}
メタフレームワーク: {}

ARCHITECTURE.md の内容:
{}

要件:
1. 適切なディレクトリ構造を作成すること（src/, tests/, docs/ など）
2. 必要な設定ファイルを生成すること（.gitignore, README.md など）
3. エントリポイントとなるファイルを最低限作成すること
4. 既存の PRODUCT_CONTEXT.md, ARCHITECTURE.md, Rule.md は絶対に変更・削除しないこと
5. 【フルスタック規約】バックエンド（API / サーバー）とフロントエンド（Vite / Next.js / Nuxt など SPA/SSR）を同時に含む構成を生成する場合は、以下を必ず満たすこと:
   - ルート直下の `package.json` に `concurrently` を devDependencies として追加する
   - ルートの scripts に以下を定義し、`npm run dev` 単発で両方が起動するようにする:
     * `"dev": "concurrently -n api,web -c blue,green \"npm:dev:api\" \"npm:dev:web\""`
     * `"dev:api"`: バックエンドの起動コマンド（例: `"tsx watch src/index.ts"` や `"node --watch src/server.js"`）
     * `"dev:web"`: フロントエンドの起動コマンド（例: `"npm --prefix frontend run dev"` や `"npm run dev --workspace=frontend"`）
   - フロントエンド側のディレクトリ（例: `frontend/`）にも独自の `package.json` と `dev` スクリプト（Vite 等）を配置する
   - バックエンドは **development モードでは `frontend/dist` を serve しない**こと。開発時は Vite 等の dev サーバーに任せ、必要なら CORS 許可 or プロキシ設定（`vite.config.ts` の `server.proxy`）で API 呼び出しを中継する
   - これらを怠ると `npm run dev` がバックエンドしか起動せず、Vicara のプレビュー（Vite の `Local: http://...:PORT/` 出力を待つ）が必ずタイムアウトする"#,
        lang, fw, meta, stack.raw_content
    )
}

async fn execute_api_scaffold_generation(
    app_handle: &AppHandle,
    local_path: &str,
    project_id: Option<&str>,
    provider: crate::rig_provider::AiProvider,
    api_key: &str,
    model: &str,
    tech_stack_info: &str,
) -> Result<String, String> {
    let system_prompt = r#"あなたはソフトウェアプロジェクトの初期構成を作る scaffolding アシスタントです。
プロジェクト直下に作成すべき最小限のディレクトリと UTF-8 テキストファイルを JSON で返してください。

出力ルール:
- 出力は JSON オブジェクトのみ
- 形式は必ず {"directories": string[], "files": [{"path": string, "content": string}], "summary": string}
- path は必ずプロジェクト直下からの相対パス
- 絶対パス、..、.claude 配下、PRODUCT_CONTEXT.md、ARCHITECTURE.md、Rule.md、AGENT.md は絶対に含めない
- 既存の Inception ドキュメントを参照しつつ、それら自体は変更しない
- content に markdown code fence を含めない
- 実行可能な最小構成を優先し、不要に大量のファイルを作らない

フルスタック規約（バックエンド + フロントエンドを同時に含む構成の場合は必ず遵守）:
- ルート直下 `package.json` の devDependencies に `concurrently` を追加する
- ルート `package.json` の scripts には必ず以下を含める:
    "dev":     "concurrently -n api,web -c blue,green \"npm:dev:api\" \"npm:dev:web\""
    "dev:api": バックエンド起動コマンド（例: "tsx watch src/index.ts"）
    "dev:web": フロントエンド起動コマンド（例: "npm --prefix frontend run dev"）
- フロントエンド側ディレクトリ（例: frontend/）には独自の package.json と Vite/Next.js 等の `dev` スクリプトを配置する
- バックエンドは **development モードでは frontend/dist を serve しない**。開発時は Vite の dev サーバーに委譲し、必要なら vite.config.ts の server.proxy で API を中継する
- この規約を守らないと `npm run dev` がバックエンドのみ起動し、Vicara のプレビュー（Vite の `Local: http://...:PORT/` ログ検出）がタイムアウトする"#;

    let user_prompt = format!(
        r#"以下の技術スタック情報に基づき、初期 scaffolding 用の JSON を生成してください。

対象ディレクトリ: {local_path}

技術スタック / ARCHITECTURE.md 抜粋:
{tech_stack_info}"#
    );

    emit_scaffold_output(
        app_handle,
        format!(
            "AI scaffolding via API: provider={}, model={}",
            scaffold_provider_label(&provider),
            model
        ),
    );

    let response = crate::rig_provider::chat_with_history(
        &provider,
        api_key,
        model,
        system_prompt,
        &user_prompt,
        vec![],
    )
    .await?;
    record_scaffold_provider_usage(app_handle, project_id, &response).await;

    let plan = parse_api_scaffold_plan(&response.content)?;
    let summary = apply_api_scaffold_plan(app_handle, Path::new(local_path), plan)?;
    let installed = install_scaffold_node_dependencies(app_handle, Path::new(local_path)).await?;
    if installed.is_empty() {
        Ok(summary)
    } else {
        Ok(format!("{} / 依存導入: {}", summary, installed.join(", ")))
    }
}

// ---------------------------------------------------------------------------
// AGENT.md テンプレート
// ---------------------------------------------------------------------------

fn build_agent_md(project_name: &str, directory_tree: &str) -> String {
    format!(
        r#"# AGENT.md — {}

> AIコーディングエージェントへの統合指示書。作業前に必ず本ファイルと参照先を読むこと。

## 必読ドキュメント

以下のファイルを必ず読んでからコーディングを開始してください。

- [PRODUCT_CONTEXT.md](./PRODUCT_CONTEXT.md) — プロジェクトの目的と方向性
- [ARCHITECTURE.md](./ARCHITECTURE.md) — システム構成と技術スタック
- [Rule.md](./Rule.md) — コーディング規約と開発ルール

## ディレクトリ構造ガイド

```
{}
```

## ワークフロー

- 実装前に上記3ファイルを必ず読むこと
- 変更完了時は walkthrough.md を出力すること
- セッション終了時は handoff.md を更新すること
"#,
        project_name, directory_tree
    )
}

fn build_claude_settings_json() -> String {
    serde_json::json!({
        "customInstructions": "必ず AGENT.md を読んでから作業を開始してください。AGENT.md にはプロジェクトの概要、アーキテクチャ、コーディング規約への参照が記載されています。"
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// Tauri コマンド
// ---------------------------------------------------------------------------

/// ARCHITECTURE.md を解析し、技術スタック情報とスキャフォールド戦略を返す。
#[tauri::command]
pub async fn detect_tech_stack(local_path: String) -> Result<TechStackDetection, String> {
    let p = Path::new(&local_path);
    let arch_path = p.join("ARCHITECTURE.md");

    if !arch_path.exists() {
        return Err(
            "ARCHITECTURE.md が見つかりません。先にインセプションデッキを完了してください。"
                .to_string(),
        );
    }

    let content = std::fs::read_to_string(&arch_path).map_err(|e| e.to_string())?;
    let tech_stack = detect_stack_from_content(&content);
    let strategy = determine_strategy(&tech_stack);

    Ok(TechStackDetection {
        tech_stack,
        strategy,
    })
}

/// スキャフォールドの状態（AGENT.md や .claude/settings.json の有無等）を確認する。
#[tauri::command]
pub async fn check_scaffold_status(local_path: String) -> Result<ScaffoldStatus, String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("ディレクトリが存在しません。".to_string());
    }

    let has_agent_md = p.join("AGENT.md").exists();
    let has_claude_settings = p.join(".claude").join("settings.json").exists();

    let mut extra_files: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(p) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_inception_or_scaffold_file(&name) && !name.starts_with('.') {
                extra_files.push(name);
            }
        }
    }
    // .claude 以外の隠しファイルは除外済み。.git 等も除外される。

    Ok(ScaffoldStatus {
        has_agent_md,
        has_claude_settings,
        has_extra_files: !extra_files.is_empty(),
        extra_files,
    })
}

/// PTY 経由で CLI スキャフォールドコマンドを実行する。
/// 出力は `scaffold_output` イベントでストリーミングされる。
#[tauri::command]
pub async fn execute_scaffold_cli(
    state: tauri::State<'_, PtyManager>,
    app_handle: AppHandle,
    local_path: String,
    command: String,
    args: Vec<String>,
) -> Result<bool, String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("ディレクトリが存在しません。".to_string());
    }

    let mut temp_workspace_root = None;
    let execution_dir = if scaffold_cli_requires_temporary_workspace(&args) {
        let (temp_root, temp_project_dir) = create_cli_scaffold_temp_project_dir(p)?;
        emit_scaffold_output(
            &app_handle,
            format!(
                "既存の Inception ドキュメントと衝突しないよう、一時ディレクトリで CLI scaffold を実行します: {}",
                temp_project_dir.display()
            ),
        );
        temp_workspace_root = Some(temp_root);
        temp_project_dir
    } else {
        p.to_path_buf()
    };

    // PTY セッションを生成
    let session_id = state
        .spawn_session(&execution_dir.to_string_lossy())
        .await?;

    // コマンド文字列を組み立て
    let full_command = if args.is_empty() {
        command.clone()
    } else {
        format!("{} {}", command, args.join(" "))
    };

    let _ = app_handle.emit(
        "scaffold_output",
        ScaffoldOutputPayload {
            output: format!("$ {}\r\n", full_command),
        },
    );

    // コマンド実行
    let result = state.execute_command(&session_id, &full_command).await;

    // セッション解放
    let _ = state.kill_session(&session_id).await;

    let response = match result {
        Ok(exec_result) => {
            // 出力をイベントとして送信
            if !exec_result.stdout.is_empty() {
                let _ = app_handle.emit(
                    "scaffold_output",
                    ScaffoldOutputPayload {
                        output: exec_result.stdout,
                    },
                );
            }
            if !exec_result.stderr.is_empty() {
                let _ = app_handle.emit(
                    "scaffold_output",
                    ScaffoldOutputPayload {
                        output: exec_result.stderr,
                    },
                );
            }

            let mut success = exec_result.exit_code == 0;
            let mut reason = if success {
                "スキャフォールド完了".to_string()
            } else {
                format!("exit code: {}", exec_result.exit_code)
            };
            if success {
                if temp_workspace_root.is_some() {
                    match import_cli_scaffold_output(&execution_dir, p) {
                        Ok(imported_entries) => {
                            for imported_entry in &imported_entries {
                                emit_scaffold_output(
                                    &app_handle,
                                    format!("imported: {}", imported_entry),
                                );
                            }
                            reason = format!(
                                "CLI スキャフォールド完了: {} 件を取り込みました",
                                imported_entries.len()
                            );
                        }
                        Err(error) => {
                            success = false;
                            reason = error;
                        }
                    }
                }
            }
            if success {
                match install_scaffold_node_dependencies(&app_handle, p).await {
                    Ok(installed_dirs) => {
                        if !installed_dirs.is_empty() {
                            reason =
                                format!("{} / 依存導入: {}", reason, installed_dirs.join(", "));
                        }
                    }
                    Err(error) => {
                        success = false;
                        reason = error;
                    }
                }
            }
            let _ = app_handle.emit("scaffold_exit", ScaffoldExitPayload { success, reason });
            Ok(success)
        }
        Err(e) => {
            let _ = app_handle.emit(
                "scaffold_exit",
                ScaffoldExitPayload {
                    success: false,
                    reason: format!("実行失敗: {}", e),
                },
            );
            Err(e)
        }
    };

    if let Some(temp_root) = temp_workspace_root {
        let _ = std::fs::remove_dir_all(temp_root);
    }

    response
}

/// PO アシスタント設定に追従して AI にディレクトリ構造を生成させる。
/// CLI transport は agent_runner と同じイベント（agent_cli_output / agent_cli_exit）を流し、
/// API transport は scaffold_output / scaffold_exit を流す。
#[tauri::command]
pub async fn execute_scaffold_ai(
    app_handle: AppHandle,
    state: tauri::State<'_, crate::agent_runner::AgentState>,
    local_path: String,
    tech_stack_info: String,
) -> Result<(), String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("ディレクトリが存在しません。".to_string());
    }

    let task_id = format!("scaffold-ai-{}", uuid::Uuid::new_v4());
    let project_id = crate::db::get_project_by_local_path(&app_handle, &local_path)
        .await?
        .map(|project| project.id);

    match resolve_scaffold_ai_transport(&app_handle, &local_path).await? {
        ScaffoldAiTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            crate::agent_runner::execute_cli_prompt_task(
                app_handle,
                state,
                task_id,
                tech_stack_info,
                cwd,
                cli_type,
                model,
                project_id,
            )
            .await
        }
        ScaffoldAiTransport::Api {
            provider,
            api_key,
            model,
        } => {
            match execute_api_scaffold_generation(
                &app_handle,
                &local_path,
                project_id.as_deref(),
                provider,
                &api_key,
                &model,
                &tech_stack_info,
            )
            .await
            {
                Ok(summary) => emit_scaffold_exit(&app_handle, true, summary),
                Err(error) => emit_scaffold_exit(&app_handle, false, error),
            }
            Ok(())
        }
    }
}

/// AGENT.md を生成する（参照ポインタ方式）。
/// Inception ファイルの内容はコピーせず、リンクのみ配置する。
/// ディレクトリ構造ガイドは Rust の std::fs で自前生成する（Windows tree コマンドの文字化け回避）。
#[tauri::command]
pub async fn generate_agent_md(local_path: String, project_name: String) -> Result<String, String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("ディレクトリが存在しません。".to_string());
    }

    // ディレクトリツリーを Rust で生成（文字化けしない）
    let directory_tree = build_directory_tree(p, 3);

    let content = build_agent_md(&project_name, &directory_tree);

    // AGENT.md を書き込み
    let agent_path = p.join("AGENT.md");
    std::fs::write(&agent_path, &content).map_err(|e| e.to_string())?;

    Ok(content)
}

/// .claude/settings.json を生成する。
#[tauri::command]
pub async fn generate_claude_settings(local_path: String) -> Result<(), String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("ディレクトリが存在しません。".to_string());
    }

    let claude_dir = p.join(".claude");
    if !claude_dir.exists() {
        std::fs::create_dir_all(&claude_dir).map_err(|e| e.to_string())?;
    }

    let settings_path = claude_dir.join("settings.json");
    let content = build_claude_settings_json();
    std::fs::write(&settings_path, content).map_err(|e| e.to_string())?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ヘルパー
// ---------------------------------------------------------------------------

/// 除外するディレクトリ名
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    ".next",
    "__pycache__",
];

/// std::fs でディレクトリツリーを生成する（文字化けしない）。
fn build_directory_tree(root: &Path, max_depth: usize) -> String {
    let mut lines = Vec::new();
    lines.push(".".to_string());
    collect_tree_entries(root, "", max_depth, 0, &mut lines);
    lines.join("\n")
}

fn collect_tree_entries(
    dir: &Path,
    prefix: &str,
    max_depth: usize,
    current_depth: usize,
    lines: &mut Vec<String>,
) {
    if current_depth >= max_depth {
        return;
    }

    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };

    // ソート: ディレクトリ優先、名前順
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_is_dir
            .cmp(&a_is_dir)
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });

    // 除外フィルタ
    let entries: Vec<_> = entries
        .into_iter()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            !IGNORED_DIRS.contains(&name.as_str())
        })
        .collect();

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_last = i == count - 1;
        let connector = if is_last { "`-- " } else { "|-- " };
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        let display_name = if is_dir { format!("{}/", name) } else { name };

        lines.push(format!("{}{}{}", prefix, connector, display_name));

        if is_dir {
            let child_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}|   ", prefix)
            };
            collect_tree_entries(
                &entry.path(),
                &child_prefix,
                max_depth,
                current_depth + 1,
                lines,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        import_cli_scaffold_output, is_reserved_scaffold_target, normalize_scaffold_relative_path,
        parse_api_scaffold_plan, scaffold_cli_requires_temporary_workspace,
    };
    use std::fs;
    use std::path::Path;

    #[test]
    fn parse_api_scaffold_plan_accepts_markdown_fenced_json() {
        let plan = parse_api_scaffold_plan(
            r#"```json
{
  "directories": ["src"],
  "files": [{ "path": "src/main.ts", "content": "console.log('ok');" }],
  "summary": "starter"
}
```"#,
        )
        .expect("fenced json should parse");

        assert_eq!(plan.directories, vec!["src"]);
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].path, "src/main.ts");
        assert_eq!(plan.summary, "starter");
    }

    #[test]
    fn normalize_scaffold_relative_path_rejects_parent_segments() {
        let error = normalize_scaffold_relative_path("../src/main.ts")
            .expect_err("parent segments should be rejected");

        assert!(error.contains("親ディレクトリ参照"));
    }

    #[test]
    fn is_reserved_scaffold_target_blocks_inception_files_and_dot_claude() {
        assert!(is_reserved_scaffold_target(Path::new("PRODUCT_CONTEXT.md")));
        assert!(is_reserved_scaffold_target(Path::new(
            ".claude/settings.json"
        )));
        assert!(!is_reserved_scaffold_target(Path::new("src/main.ts")));
    }

    #[test]
    fn scaffold_cli_requires_temp_workspace_when_target_is_current_directory() {
        assert!(scaffold_cli_requires_temporary_workspace(&[
            "create-next-app@latest".to_string(),
            ".".to_string(),
        ]));
        assert!(!scaffold_cli_requires_temporary_workspace(&[
            "create-next-app@latest".to_string(),
            "my-app".to_string(),
        ]));
    }

    #[test]
    fn import_cli_scaffold_output_merges_generated_files_without_touching_inception_docs() {
        let generated_dir = tempfile::tempdir().expect("generated tempdir should exist");
        let project_dir = tempfile::tempdir().expect("project tempdir should exist");

        fs::write(project_dir.path().join("PRODUCT_CONTEXT.md"), "# existing").expect("seed docs");
        fs::create_dir_all(generated_dir.path().join("src")).expect("src dir should exist");
        fs::write(generated_dir.path().join("package.json"), "{}")
            .expect("package file should exist");
        fs::write(
            generated_dir.path().join("src").join("main.ts"),
            "console.log('ok');",
        )
        .expect("main file should exist");

        let result = import_cli_scaffold_output(generated_dir.path(), project_dir.path())
            .expect("import should succeed");

        assert_eq!(result, vec!["package.json".to_string(), "src".to_string()]);
        assert!(project_dir.path().join("PRODUCT_CONTEXT.md").exists());
        assert!(project_dir.path().join("package.json").exists());
        assert!(project_dir.path().join("src").join("main.ts").exists());
    }

    #[test]
    fn import_cli_scaffold_output_rejects_top_level_conflicts() {
        let generated_dir = tempfile::tempdir().expect("generated tempdir should exist");
        let project_dir = tempfile::tempdir().expect("project tempdir should exist");

        fs::write(generated_dir.path().join("package.json"), "{}")
            .expect("generated package exists");
        fs::write(project_dir.path().join("package.json"), "{}").expect("project package exists");

        let error = import_cli_scaffold_output(generated_dir.path(), project_dir.path())
            .expect_err("conflicting top-level file should fail");

        assert!(error.contains("package.json"));
    }
}
