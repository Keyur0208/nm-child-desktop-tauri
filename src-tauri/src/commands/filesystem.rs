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

#[tauri::command]
pub async fn save_download_bytes(
    app: AppHandle,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    // Sanitize filename
    let sanitized_filename = filename
        .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let mut clean_filename = if sanitized_filename.trim().is_empty() {
        "download".to_string()
    } else {
        sanitized_filename.trim().to_string()
    };

    // Auto-detect file format from magic bytes to guarantee Excel files are accurately named
    let has_ext = std::path::Path::new(&clean_filename).extension().is_some();
    if !has_ext || clean_filename.ends_with(".pdf") {
        if bytes.starts_with(b"PK\x03\x04") {
            // Modern Excel XLSX
            clean_filename = if clean_filename.ends_with(".pdf") {
                format!("{}.xlsx", &clean_filename[..clean_filename.len() - 4])
            } else {
                format!("{}.xlsx", clean_filename)
            };
        } else if bytes.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1") {
            // Legacy Excel XLS (BIFF8)
            clean_filename = if clean_filename.ends_with(".pdf") {
                format!("{}.xls", &clean_filename[..clean_filename.len() - 4])
            } else {
                format!("{}.xls", clean_filename)
            };
        } else if bytes.starts_with(b"%PDF-") {
            if !clean_filename.ends_with(".pdf") {
                clean_filename = format!("{}.pdf", clean_filename);
            }
        } else if !has_ext {
            clean_filename = format!("{}.xlsx", clean_filename);
        }
    }

    let download_dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().home_dir().map(|h| h.join("Downloads")))
        .unwrap_or_else(|_| std::env::temp_dir());

    if !download_dir.exists() {
        let _ = std::fs::create_dir_all(&download_dir);
    }

    #[cfg(target_os = "macos")]
    {
        let mut target = download_dir.join(&clean_filename);
        if target.exists() {
            let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("download");
            let ext = target.extension().and_then(|e| e.to_str()).unwrap_or("");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let new_name = if ext.is_empty() {
                format!("{}_{}", stem, now)
            } else {
                format!("{}_{}.{}", stem, now, ext)
            };
            target = download_dir.join(new_name);
        }

        fs::write(&target, &bytes)
            .map_err(|e| format!("Failed to save downloaded file: {}", e))?;

        crate::services::logging::log_info(&format!(
            "[Download] macOS saved file directly to Downloads: {:?}",
            target
        ));

        Ok(target.to_string_lossy().to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        use tauri_plugin_dialog::DialogExt;

        let mut builder = app
            .dialog()
            .file()
            .set_title("Save File")
            .set_file_name(&clean_filename)
            .set_directory(&download_dir);

        if let Some(ext) = std::path::Path::new(&clean_filename).extension().and_then(|e| e.to_str()) {
            builder = builder.add_filter(format!("{} File (*.{})", ext.to_uppercase(), ext), &[ext]);
        }

        // Open native OS file selection dialog (runs on blocking thread so UI remains fully responsive)
        let chosen = tauri::async_runtime::spawn_blocking(move || {
            builder.blocking_save_file()
        })
        .await
        .map_err(|e| format!("Dialog execution error: {}", e))?;

        let selected_path = match chosen {
            Some(p) => match p.into_path() {
                Ok(path) => path,
                Err(_) => return Err("Invalid selected file path".to_string()),
            },
            None => {
                crate::services::logging::log_info("[Download] User cancelled file selection dialog");
                return Ok("CANCELLED".to_string());
            }
        };

        fs::write(&selected_path, &bytes)
            .map_err(|e| format!("Failed to save downloaded file: {}", e))?;

        crate::services::logging::log_info(&format!(
            "[Download] Successfully saved file to user-selected location: {:?}",
            selected_path
        ));

        Ok(selected_path.to_string_lossy().to_string())
    }
}
