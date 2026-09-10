use chrono::Utc;
use chrono_tz::Asia::Kolkata;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init_logger(app: &AppHandle) {
    if let Ok(base) = app.path().app_data_dir() {
        let logs_dir = base.join("logs");
        let _ = std::fs::create_dir_all(&logs_dir);
        let mut guard = LOG_PATH.lock().unwrap();
        *guard = Some(logs_dir);
    }
}

fn get_current_log_file() -> Option<PathBuf> {
    let guard = LOG_PATH.lock().unwrap();
    if let Some(ref dir) = *guard {
        let now_kolkata = Utc::now().with_timezone(&Kolkata);
        let date_str = now_kolkata.format("%d-%m-%Y").to_string();
        Some(dir.join(format!("diagnostic-{}.log", date_str)))
    } else {
        None
    }
}

fn write_log(level: &str, text: &str) {
    let now_kolkata = Utc::now().with_timezone(&Kolkata);
    let timestamp = now_kolkata.format("%d-%m-%Y %H:%M:%S%.3f").to_string();
    let line = format!("[{}] [{}] {}\n", timestamp, level, text);

    print!("{}", line);

    if let Some(path) = get_current_log_file() {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
}

pub fn log_info(text: &str) {
    write_log("INFO", text);
}

pub fn log_warn(text: &str) {
    write_log("WARN", text);
}

pub fn log_error(text: &str) {
    write_log("ERROR", text);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthLogDetails {
    pub url: Option<String>,
    pub status: Option<i32>,
    pub has_token: Option<bool>,
    pub token_expiry: Option<String>,
    pub message: Option<String>,
}

#[tauri::command]
pub async fn log_diagnostic(level: String, message: String) -> Result<(), String> {
    match level.to_uppercase().as_str() {
        "ERROR" => log_error(&message),
        "WARN" => log_warn(&message),
        _ => log_info(&message),
    }
    Ok(())
}

#[tauri::command]
pub async fn log_auth_event(event_name: String, details: AuthLogDetails) -> Result<(), String> {
    let now_kolkata = Utc::now().with_timezone(&Kolkata);
    let timestamp = now_kolkata.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

    let text = format!(
        "\n============================================================\n\
        [AUTH DIAGNOSTIC] Event: {}\n\
        Timestamp: {}\n\
        URL: {}\n\
        HTTP Status: {}\n\
        Token Present in Storage: {}\n\
        Token Expiry Info: {}\n\
        Message: {}\n\
        ============================================================",
        event_name,
        timestamp,
        details.url.unwrap_or_else(|| "N/A".to_string()),
        details.status.map(|s| s.to_string()).unwrap_or_else(|| "N/A".to_string()),
        if details.has_token.unwrap_or(false) { "YES" } else { "NO" },
        details.token_expiry.unwrap_or_else(|| "N/A".to_string()),
        details.message.unwrap_or_default()
    );

    log_info(&text);
    Ok(())
}
