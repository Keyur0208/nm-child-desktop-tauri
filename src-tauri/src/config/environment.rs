use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Application deployment environment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppEnv {
    Development,
    Production,
    #[serde(untagged)]
    Custom(String),
}

impl AppEnv {
    pub fn from_str_value(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "production" | "prod" => AppEnv::Production,
            "development" | "dev" => AppEnv::Development,
            other => AppEnv::Custom(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            AppEnv::Development => "development",
            AppEnv::Production => "production",
            AppEnv::Custom(ref s) => s.as_str(),
        }
    }
}

/// Consolidated runtime and build-time environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentConfig {
    pub app_env: AppEnv,
    pub api_url: String,
    pub log_level: String,
}

impl EnvironmentConfig {
    /// True if running in development mode
    pub fn is_dev(&self) -> bool {
        matches!(self.app_env, AppEnv::Development)
    }

    /// True if running in production mode
    pub fn is_prod(&self) -> bool {
        matches!(self.app_env, AppEnv::Production)
    }

    /// Determines whether DevTools should be allowed/enabled
    pub fn is_devtools_enabled(&self) -> bool {
        // 1. Allow explicit runtime override if set in environment or local .env
        for key in &["ENABLE_DEVTOOLS", "DEVTOOLS", "TAURI_DEVTOOLS"] {
            if let Ok(val) = std::env::var(key) {
                let v = val.trim().to_lowercase();
                if v == "true" || v == "1" || v == "yes" || v == "on" {
                    return true;
                }
                if v == "false" || v == "0" || v == "no" || v == "off" {
                    return false;
                }
            }
        }

        // 2. Default: Enabled in development, disabled in production
        self.is_dev()
    }
}

static CONFIG: OnceLock<EnvironmentConfig> = OnceLock::new();

/// Resolves environment configuration by checking:
/// 1. Optional runtime .env next to executable or working directory
/// 2. Active runtime environment variables
/// 3. Compile-time embedded values from build.rs (.env.development / .env.production)
fn init_config() -> EnvironmentConfig {
    // Attempt to load runtime .env if present (working dir, parent dir, or next to exe)
    dotenvy::dotenv().ok();
    let _ = dotenvy::from_filename("../.env");
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_env = exe_dir.join(".env");
            if exe_env.exists() {
                let _ = dotenvy::from_path(&exe_env);
            }
        }
    }

    // 1. Resolve APP_ENV (supports VITE_APP_ENV or APP_ENV)
    let env_str = std::env::var("VITE_APP_ENV")
        .or_else(|_| std::env::var("APP_ENV"))
        .ok()
        .or_else(|| option_env!("APP_ENV").map(|s| s.to_string()))
        .unwrap_or_else(|| {
            if cfg!(debug_assertions) {
                "development".to_string()
            } else {
                "production".to_string()
            }
        });
    let app_env = AppEnv::from_str_value(&env_str);

    // 2. Resolve API_URL (supports VITE_API_URL or API_URL)
    let default_api_url = match app_env {
        AppEnv::Production => "http://app.nilkanthmedico.com",
        _ => "http://localhost:8081",
    };
    let api_url = std::env::var("VITE_API_URL")
        .or_else(|_| std::env::var("API_URL"))
        .ok()
        .or_else(|| option_env!("API_URL").map(|s| s.to_string()))
        .unwrap_or_else(|| default_api_url.to_string());

    // 3. Resolve LOG_LEVEL (supports VITE_LOG_LEVEL or LOG_LEVEL)
    let default_log_level = match app_env {
        AppEnv::Production => "info",
        _ => "debug",
    };
    let log_level = std::env::var("VITE_LOG_LEVEL")
        .or_else(|_| std::env::var("LOG_LEVEL"))
        .ok()
        .or_else(|| option_env!("LOG_LEVEL").map(|s| s.to_string()))
        .unwrap_or_else(|| default_log_level.to_string());

    EnvironmentConfig {
        app_env,
        api_url,
        log_level,
    }
}

/// Returns a reference to the global lazily-initialized environment configuration
pub fn get_config() -> &'static EnvironmentConfig {
    CONFIG.get_or_init(init_config)
}

/// Tauri command to retrieve the current environment configuration from the frontend
#[tauri::command]
pub fn get_environment_info() -> EnvironmentConfig {
    get_config().clone()
}
