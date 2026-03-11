use std::path::Path;
use tauri::AppHandle;

pub const BASE_RULE_CONTENT: &str = r#"# コーディング規約・エージェントルール
- AIはTypeScriptで厳格な型定義を利用し、React関数コンポーネントとHooksを用いること。
- DBやStoreの更新時には、必ずUIへの即時反映（リアクティビティ確保）を行うこと。
- アプローチを決定する際は必ずPO（人間）の承認を得ること。
- 日本語で自然なUIを構築すること。
"#;

#[tauri::command]
pub async fn generate_base_rule(_app: AppHandle, local_path: String) -> Result<bool, String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("Directory does not exist".to_string());
    }
    
    let rule_path = p.join("Rule.md");
    if !rule_path.exists() {
        std::fs::write(&rule_path, BASE_RULE_CONTENT).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

#[tauri::command]
pub async fn read_inception_file(_app: AppHandle, local_path: String, filename: String) -> Result<Option<String>, String> {
    let p = Path::new(&local_path);
    let file_path = p.join(filename);
    if file_path.exists() {
        let content = std::fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn write_inception_file(_app: AppHandle, local_path: String, filename: String, content: String, append: bool) -> Result<bool, String> {
    let p = Path::new(&local_path);
    if !p.exists() || !p.is_dir() {
        return Err("Directory does not exist".to_string());
    }
    
    let file_path = p.join(filename);
    if append {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&file_path)
            .map_err(|e| e.to_string())?;
        file.write_all(b"\n").map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes()).map_err(|e| e.to_string())?;
    } else {
        std::fs::write(&file_path, content).map_err(|e| e.to_string())?;
    }
    Ok(true)
}
