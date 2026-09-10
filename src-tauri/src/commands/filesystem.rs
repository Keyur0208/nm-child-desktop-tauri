use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoragePaths {
    pub base_dir: String,
    pub images_dir: String,
    pub documents_dir: String,
    pub pdf_dir: String,
    pub cache_dir: String,
    pub logs_dir: String,
    pub database_dir: String,
}

pub fn get_app_storage_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app_data_dir: {}", e))?;

    let dirs = ["images", "documents", "pdf", "cache", "logs", "database"];
    for dir in dirs {
        let path = base.join(dir);
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
    }

    Ok(base)
}

#[tauri::command]
pub async fn get_app_storage_paths(app: AppHandle) -> Result<StoragePaths, String> {
    let base = get_app_storage_dir(&app)?;

    Ok(StoragePaths {
        base_dir: base.to_string_lossy().to_string(),
        images_dir: base.join("images").to_string_lossy().to_string(),
        documents_dir: base.join("documents").to_string_lossy().to_string(),
        pdf_dir: base.join("pdf").to_string_lossy().to_string(),
        cache_dir: base.join("cache").to_string_lossy().to_string(),
        logs_dir: base.join("logs").to_string_lossy().to_string(),
        database_dir: base.join("database").to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn save_document_file(
    app: AppHandle,
    category: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    // Prevent directory traversal attacks
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err("Invalid filename".to_string());
    }

    let base = get_app_storage_dir(&app)?;
    let sub_dir = match category.as_str() {
        "images" => base.join("images"),
        "documents" => base.join("documents"),
        "pdf" => base.join("pdf"),
        "cache" => base.join("cache"),
        _ => return Err("Invalid storage category".to_string()),
    };

    let target = sub_dir.join(filename);
    fs::write(&target, bytes).map_err(|e| e.to_string())?;

    Ok(target.to_string_lossy().to_string())
}
