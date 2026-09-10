use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub pc_name: String,
    pub username: String,
    pub ip_address: String,
    pub mac_address: String,
    pub platform: String,
    pub arch: String,
    pub cpu_model: String,
    pub cpu_cores: usize,
    pub total_ram_mb: u64,
    pub is_printer_supported: bool,
    pub default_printer: String,
}

pub fn get_system_info_internal() -> SystemInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let pc_name = whoami::fallible::hostname().unwrap_or_else(|_| "Unknown".to_string());
    let username = whoami::fallible::username().unwrap_or_else(|_| "Unknown".to_string());
    let platform = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();

    let cpu = sys.cpus().first();
    let cpu_model = cpu
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let cpu_cores = sys.cpus().len();
    let total_ram_mb = sys.total_memory() / (1024 * 1024);

    let ip_address = crate::utils::get_local_ipv4();
    let mac_address = mac_address::get_mac_address()
        .ok()
        .flatten()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let default_printer = crate::commands::printer::get_default_printer_internal();

    SystemInfo {
        pc_name,
        username,
        ip_address,
        mac_address,
        platform,
        arch,
        cpu_model,
        cpu_cores,
        total_ram_mb,
        is_printer_supported: true,
        default_printer,
    }
}

pub fn generate_injection_script(info: &SystemInfo) -> String {
    let json_raw = serde_json::to_string(info).unwrap_or_else(|_| "{}".to_string());
    let escaped_js_str = serde_json::to_string(&json_raw).unwrap_or_else(|_| "\"{{}}\"".to_string());

    format!(
        r#"
        (function() {{
            try {{
                const infoStr = {};
                localStorage.setItem('resourceInfo', infoStr);
                const parsed = JSON.parse(infoStr);
                window.resourceInfo = parsed;
                window.__RESOURCE_INFO__ = parsed;
                console.log('[Tauri Native] Successfully injected resourceInfo for origin:', window.location.origin);
            }} catch (err) {{
                console.error('[Tauri Native] Failed to inject resourceInfo into localStorage:', err);
            }}
        }})();
        "#,
        escaped_js_str
    )
}

#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    Ok(get_system_info_internal())
}

#[tauri::command]
pub async fn inject_resource_info(window: tauri::WebviewWindow) -> Result<(), String> {
    let info = get_system_info_internal();
    let script = generate_injection_script(&info);
    window.eval(&script).map_err(|e| e.to_string())
}

