use std::path::Path;
use tauri::{AppHandle, Emitter};

use crate::pty_manager::PtyManager;

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

fn is_inception_or_scaffold_file(name: &str) -> bool {
    INCEPTION_FILES.iter().any(|f| name == *f)
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
4. 既存の PRODUCT_CONTEXT.md, ARCHITECTURE.md, Rule.md は絶対に変更・削除しないこと"#,
        lang, fw, meta, stack.raw_content
    )
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

    // PTY セッションを生成
    let session_id = state.spawn_session(&local_path).await?;

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

    match result {
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

            let success = exec_result.exit_code == 0;
            let _ = app_handle.emit(
                "scaffold_exit",
                ScaffoldExitPayload {
                    success,
                    reason: if success {
                        "スキャフォールド完了".to_string()
                    } else {
                        format!("exit code: {}", exec_result.exit_code)
                    },
                },
            );
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
    }
}

/// Claude CLI 経由で AI にディレクトリ構造を生成させる。
/// claude_runner の execute_claude_task と同じイベント（claude_cli_output / claude_cli_exit）で出力される。
#[tauri::command]
pub async fn execute_scaffold_ai(
    app_handle: AppHandle,
    state: tauri::State<'_, crate::claude_runner::ClaudeState>,
    local_path: String,
    tech_stack_info: String,
) -> Result<(), String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("ディレクトリが存在しません。".to_string());
    }

    let task_id = format!("scaffold-ai-{}", uuid::Uuid::new_v4());

    // Claude CLI に直接委譲
    crate::claude_runner::execute_claude_prompt_task(
        app_handle,
        state,
        task_id,
        tech_stack_info,
        local_path,
    )
    .await
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
