use std::env;
use std::path::PathBuf;

fn main() {
    // Determine build profile: "debug" or "release"
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());

    // Determine target environment: prioritize explicitly set APP_ENV, otherwise infer from profile
    let app_env = env::var("APP_ENV").unwrap_or_else(|_| {
        if profile == "release" {
            "production".to_string()
        } else {
            "development".to_string()
        }
    });

    let target_env_file = match app_env.as_str() {
        "production" => ".env.production",
        _ => ".env",
    };

    // Candidates: root project directory (../), then current (./)
    let candidates = [
        PathBuf::from("..").join(target_env_file),
        PathBuf::from(target_env_file),
        PathBuf::from("..").join(".env"),
        PathBuf::from(".env"),
    ];

    for path in &candidates {
        if path.exists() {
            let _ = dotenvy::from_path(path);
            break;
        }
    }

    // Resolve final environment values with sensible defaults
    let resolved_app_env = env::var("VITE_APP_ENV")
        .or_else(|_| env::var("APP_ENV"))
        .unwrap_or(app_env);

    let default_api_url = if resolved_app_env == "production" {
        "http://app.nilkanthmedico.com".to_string()
    } else {
        "http://localhost:8081".to_string()
    };

    let api_url = env::var("VITE_API_URL")
        .or_else(|_| env::var("API_URL"))
        .unwrap_or(default_api_url);

    let default_log_level = if resolved_app_env == "production" {
        "info".to_string()
    } else {
        "debug".to_string()
    };

    let log_level = env::var("VITE_LOG_LEVEL")
        .or_else(|_| env::var("LOG_LEVEL"))
        .unwrap_or(default_log_level);

    // Embed variables into the compiled binary
    println!("cargo:rustc-env=APP_ENV={}", resolved_app_env);
    println!("cargo:rustc-env=API_URL={}", api_url);
    println!("cargo:rustc-env=LOG_LEVEL={}", log_level);

    // Watch env files in both root (..) and src-tauri (.) for re-compilation
    println!("cargo:rerun-if-changed=../.env.development");
    println!("cargo:rerun-if-changed=../.env.production");
    println!("cargo:rerun-if-changed=../.env");
    println!("cargo:rerun-if-changed=.env.development");
    println!("cargo:rerun-if-changed=.env.production");
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-env-changed=APP_ENV");
    println!("cargo:rerun-if-env-changed=VITE_APP_ENV");
    println!("cargo:rerun-if-env-changed=API_URL");
    println!("cargo:rerun-if-env-changed=VITE_API_URL");
    println!("cargo:rerun-if-env-changed=LOG_LEVEL");
    println!("cargo:rerun-if-env-changed=VITE_LOG_LEVEL");

    tauri_build::build();
}
