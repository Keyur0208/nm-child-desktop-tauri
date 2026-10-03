use std::fs;
use std::thread;
use std::time::{Duration, SystemTime};
use tauri::{AppHandle, Manager};

const RETENTION_DAYS: u64 = 30;
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

pub fn clean_old_logs(app: &AppHandle) {
    let logs_dir = match app.path().app_data_dir() {
        Ok(base) => base.join("logs"),
        Err(_) => return,
    };

    if !logs_dir.exists() {
        return;
    }

    crate::services::logging::log_info(&format!(
        "[LogCleaner] Log cleanup started (Retention: {} days)",
        RETENTION_DAYS
    ));

    let now = SystemTime::now();
    let max_age = Duration::from_secs(RETENTION_DAYS * SECONDS_PER_DAY);
    let mut deleted_count = 0;

    if let Ok(entries) = fs::read_dir(&logs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if let Some(ext) = path.extension() {
                if ext == "log" {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    if fs::remove_file(&path).is_ok() {
                                        deleted_count += 1;
                                        crate::services::logging::log_info(&format!(
                                            "[LogCleaner] Deleted 30-day-old log file: {:?}",
                                            path.file_name()
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    crate::services::logging::log_info(&format!(
        "[LogCleaner] Cleanup completed. Total deleted files: {}",
        deleted_count
    ));
}

/// Safely cleans WebView HTTP disk cache and code cache files older than 7 days
/// WITHOUT touching cookies, localStorage, or user sessions.
pub fn clean_browser_disk_cache(app: &AppHandle) {
    let base_dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => return,
    };

    // Candidate cache directories under WebView profile (e.g. EBWebView/Default/Cache, EBWebView/Default/Code Cache)
    let cache_dirs = [
        base_dir.join("EBWebView").join("Default").join("Cache"),
        base_dir.join("EBWebView").join("Default").join("Code Cache"),
        base_dir.join("EBWebView").join("Default").join("GPUCache"),
    ];

    let now = SystemTime::now();
    let max_age = Duration::from_secs(7 * SECONDS_PER_DAY); // 7-day-old cache cleanup
    let mut cleaned_files = 0;

    for dir in &cache_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(mod_time) = meta.modified() {
                            if let Ok(age) = now.duration_since(mod_time) {
                                if age > max_age && fs::remove_file(&path).is_ok() {
                                    cleaned_files += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if cleaned_files > 0 {
        crate::services::logging::log_info(&format!(
            "[Cleaner] Cleaned {} stale browser cache files older than 7 days",
            cleaned_files
        ));
    }
}

pub fn start_log_cleaner(app: AppHandle) {
    thread::spawn(move || {
        // Initial run on startup
        clean_old_logs(&app);
        clean_browser_disk_cache(&app);

        // Periodic run every 24 hours
        loop {
            thread::sleep(Duration::from_secs(SECONDS_PER_DAY));
            clean_old_logs(&app);
            clean_browser_disk_cache(&app);
        }
    });

    crate::services::logging::log_info("[LogCleaner] Daily log and browser cache cleanup background timer scheduled");
}
