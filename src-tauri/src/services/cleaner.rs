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

pub fn start_log_cleaner(app: AppHandle) {
    thread::spawn(move || {
        // Initial run on startup
        clean_old_logs(&app);

        // Periodic run every 24 hours
        loop {
            thread::sleep(Duration::from_secs(SECONDS_PER_DAY));
            clean_old_logs(&app);
        }
    });

    crate::services::logging::log_info("[LogCleaner] Daily log cleanup background timer scheduled");
}
