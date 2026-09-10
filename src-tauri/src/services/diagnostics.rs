use chrono::{Timelike, Utc};
use chrono_tz::Asia::Kolkata;
use std::collections::HashSet;
use std::fs;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tauri::{AppHandle, Manager};

const RELAUNCH_START_HOUR: u32 = 3;
const RELAUNCH_END_HOUR: u32 = 4;
const TRIM_MEMORY_THRESHOLD_MB: u64 = 1000;    // Auto-trim RAM cache at 1000 MB without restart
const WARNING_BANNER_THRESHOLD_MB: u64 = 1800; // Show Warning Banner if user is working and RAM >= 1800 MB
const SOFT_RELOAD_IDLE_MB: u64 = 2200;         // Soft reload page if user is idle and RAM >= 2200 MB
const HARD_RESTART_CRITICAL_MB: u64 = 3500;    // Only full process restart if memory exceeds 3500 MB while idle
const IDLE_MINUTES_THRESHOLD: u64 = 15;
const CHECK_INTERVAL_SECS: u64 = 60;
const MIN_UPTIME_BEFORE_NIGHTLY_RESTART_SECS: u64 = 1800; // Require at least 30 min uptime before scheduled restart

#[cfg(windows)]
pub fn get_system_idle_seconds() -> u64 {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

    let mut lii = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    let success = unsafe { GetLastInputInfo(&mut lii).as_bool() };
    if success {
        let tick = unsafe { GetTickCount() };
        let elapsed_ms = tick.saturating_sub(lii.dwTime);
        (elapsed_ms / 1000) as u64
    } else {
        0
    }
}

#[cfg(target_os = "macos")]
pub fn get_system_idle_seconds() -> u64 {
    use std::process::Command;
    if let Ok(output) = Command::new("ioreg").args(["-c", "IOHIDSystem", "-r"]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("\"HIDIdleTime\"") {
                if let Some(val_str) = line.split('=').nth(1) {
                    if let Ok(nanos) = val_str.trim().parse::<u64>() {
                        return nanos / 1_000_000_000;
                    }
                }
            }
        }
    }
    0
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn get_system_idle_seconds() -> u64 {
    0
}

/// Trims Windows process working set (RAM) across all app and WebView2 processes without closing the app.
#[cfg(windows)]
pub fn trim_process_tree_memory(pids: &[u32]) {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA,
    };

    let access = PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION;
    for &pid in pids {
        unsafe {
            if let Ok(handle) = OpenProcess(access, false, pid) {
                let _ = EmptyWorkingSet(handle);
                let _ = CloseHandle(handle);
            }
        }
    }
}

#[cfg(not(windows))]
pub fn trim_process_tree_memory(_pids: &[u32]) {}

/// Calculates total memory consumed by the Rust main process AND all WebView2 child processes (renderers, GPU).
pub fn get_app_tree_memory_mb(sys: &System, root_pid: Pid) -> (u64, Vec<u32>) {
    let mut total_bytes = 0u64;
    let mut pids_to_check = vec![root_pid];
    let mut all_pids = HashSet::new();
    all_pids.insert(root_pid);

    while let Some(current) = pids_to_check.pop() {
        if let Some(proc) = sys.process(current) {
            total_bytes += proc.memory();
        }

        for (pid, proc) in sys.processes() {
            if proc.parent() == Some(current) && all_pids.insert(*pid) {
                pids_to_check.push(*pid);
            }
        }
    }

    let pids: Vec<u32> = all_pids.into_iter().map(|p| p.as_u32()).collect();
    (total_bytes / (1024 * 1024), pids)
}

/// Manually triggers a Win32 EmptyWorkingSet memory trim across the process tree and returns (before_mb, after_mb).
pub fn trim_memory_now() -> (u64, u64) {
    let mut sys = System::new_all();
    sys.refresh_all();
    let root_pid = Pid::from_u32(std::process::id());
    let (before_mb, pids) = get_app_tree_memory_mb(&sys, root_pid);
    trim_process_tree_memory(&pids);

    thread::sleep(Duration::from_millis(150));
    sys.refresh_all();
    let (after_mb, _) = get_app_tree_memory_mb(&sys, root_pid);
    crate::services::logging::log_info(&format!(
        "[Watchdog] Win32 EmptyWorkingSet executed: {} MB -> {} MB across {} processes",
        before_mb, after_mb, pids.len()
    ));
    (before_mb, after_mb)
}

/// Soft Reloads the web page without killing the .exe, while preserving login session and route URL.
pub fn soft_reload_page(window: &tauri::WebviewWindow, reason: &str) {
    crate::services::logging::log_warn(&format!("[Watchdog] Executing Soft Refresh: {}", reason));
    let script = r#"
        try {
            if (window.location && window.location.href && !window.location.href.includes('tauri://') && !window.location.href.includes('localhost:5173')) {
                localStorage.setItem('__nm_last_active_url', window.location.href);
            }
            window.location.reload();
        } catch(e) {
            window.location.reload();
        }
    "#;
    let _ = window.eval(script);
}

/// Displays an elegant, non-intrusive memory notification banner (like VS Code / Slack).
pub fn show_memory_warning_banner(window: &tauri::WebviewWindow, mem_mb: u64) {
    let script = format!(
        r#"
        (function() {{
            if (document.getElementById('nm-memory-banner')) {{
                const textEl = document.getElementById('nm-memory-text');
                if (textEl) textEl.textContent = 'High Memory Usage ({mem_mb} MB): Please save your work.';
                return;
            }}
            const banner = document.createElement('div');
            banner.id = 'nm-memory-banner';
            banner.setAttribute('style', 'position:fixed;top:16px;right:16px;z-index:9999999;background:rgba(15,23,42,0.94);color:#f8fafc;padding:10px 16px;border-radius:10px;box-shadow:0 10px 25px -5px rgba(0,0,0,0.5);border:1px solid rgba(234,179,8,0.7);font-family:system-ui,-apple-system,sans-serif;display:flex;align-items:center;gap:12px;font-size:13px;backdrop-filter:blur(8px);animation:fadeIn 0.3s ease;');
            banner.innerHTML = `
                <div style="display:flex;align-items:center;gap:8px;">
                    <span style="font-size:16px;">⚠️</span>
                    <span id="nm-memory-text" style="font-weight:500;">High RAM ({mem_mb} MB): Please save work.</span>
                </div>
                <div style="display:flex;align-items:center;gap:6px;margin-left:6px;">
                    <button id="nm-btn-freeram" style="background:#0284c7;color:#fff;border:none;padding:5px 10px;border-radius:6px;cursor:pointer;font-weight:600;font-size:11px;transition:background 0.2s;">Free RAM</button>
                    <button id="nm-btn-softreload" style="background:#059669;color:#fff;border:none;padding:5px 10px;border-radius:6px;cursor:pointer;font-weight:600;font-size:11px;transition:background 0.2s;">Save & Reload</button>
                    <button id="nm-btn-close-banner" style="background:transparent;color:#94a3b8;border:none;cursor:pointer;font-size:16px;padding:0 4px;line-height:1;">✕</button>
                </div>
            `;
            document.body.appendChild(banner);

            const btnFree = document.getElementById('nm-btn-freeram');
            if (btnFree) {{
                btnFree.onclick = function() {{
                    btnFree.textContent = 'Freeing...';
                    try {{
                        if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke) {{
                            window.__TAURI_INTERNALS__.invoke('trim_memory').then(() => {{
                                btnFree.textContent = '✅ Freed!';
                                setTimeout(() => banner.remove(), 2000);
                            }}).catch(() => {{ banner.remove(); }});
                        }} else if (window.__TAURI__ && window.__TAURI__.core) {{
                            window.__TAURI__.core.invoke('trim_memory').then(() => {{
                                btnFree.textContent = '✅ Freed!';
                                setTimeout(() => banner.remove(), 2000);
                            }}).catch(() => {{ banner.remove(); }});
                        }} else {{
                            banner.remove();
                        }}
                    }} catch(e) {{
                        banner.remove();
                    }}
                }};
            }}

            const btnReload = document.getElementById('nm-btn-softreload');
            if (btnReload) {{
                btnReload.onclick = function() {{
                    try {{
                        if (window.location && window.location.href) {{
                            localStorage.setItem('__nm_last_active_url', window.location.href);
                        }}
                    }} catch(e) {{}}
                    window.location.reload();
                }};
            }}

            const btnClose = document.getElementById('nm-btn-close-banner');
            if (btnClose) {{
                btnClose.onclick = function() {{
                    banner.remove();
                }};
            }}
        }})();
        "#,
        mem_mb = mem_mb
    );
    let _ = window.eval(&script);
}

/// Removes the memory warning banner when memory returns to safe levels.
pub fn remove_memory_warning_banner(window: &tauri::WebviewWindow) {
    let script = r#"
        (function() {
            const b = document.getElementById('nm-memory-banner');
            if (b) b.remove();
        })();
    "#;
    let _ = window.eval(script);
}

fn get_last_restart_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok().map(|p| p.join("last_nightly_restart.txt"))
}

fn get_last_restart_date(app: &AppHandle) -> Option<String> {
    if let Some(path) = get_last_restart_file(app) {
        fs::read_to_string(path).ok().map(|s| s.trim().to_string())
    } else {
        None
    }
}

fn set_last_restart_date(app: &AppHandle, date_str: &str) {
    if let Some(path) = get_last_restart_file(app) {
        let _ = fs::write(path, date_str);
    }
}

pub fn start_memory_watchdog(app: AppHandle) {
    thread::spawn(move || {
        let mut sys = System::new_all();
        let root_pid = Pid::from_u32(std::process::id());
        let start_time = Instant::now();
        let mut loop_counter: u64 = 0;
        let mut banner_visible = false;

        crate::services::logging::log_info(&format!(
            "[Watchdog] 24x7 Watchdog active | Daily window: {:02}:00 - {:02}:00 IST | Trim: {} MB | Warning Banner: {} MB | Soft Reload: {} MB | Idle: {} min",
            RELAUNCH_START_HOUR, RELAUNCH_END_HOUR, TRIM_MEMORY_THRESHOLD_MB, WARNING_BANNER_THRESHOLD_MB, SOFT_RELOAD_IDLE_MB, IDLE_MINUTES_THRESHOLD
        ));

        loop {
            thread::sleep(Duration::from_secs(CHECK_INTERVAL_SECS));
            loop_counter += 1;

            let idle_sec = get_system_idle_seconds();
            let idle_min = idle_sec / 60;
            let is_idle = idle_min >= IDLE_MINUTES_THRESHOLD;

            // Refresh all processes to catch newly spawned msedgewebview2.exe renderers
            sys.refresh_all();
            let (total_mem_mb, pids) = get_app_tree_memory_mb(&sys, root_pid);

            let now_kolkata = Utc::now().with_timezone(&Kolkata);
            let current_hour = now_kolkata.hour();
            let current_minute = now_kolkata.minute();
            let today_str = now_kolkata.format("%Y-%m-%d").to_string();

            // Periodic diagnostic memory log every 5 minutes or when RAM is elevated
            if loop_counter % 5 == 0 || total_mem_mb >= TRIM_MEMORY_THRESHOLD_MB {
                crate::services::logging::log_info(&format!(
                    "[Diagnostics] App Tree RAM: {} MB across {} processes | System Idle: {} min | Time: {:02}:{:02} IST",
                    total_mem_mb, pids.len(), idle_min, current_hour, current_minute
                ));
            }

            // Health check: verify WebView renderer is alive (detects "This page is having a problem" crash)
            if let Some(window) = app.get_webview_window("main") {
                let eval_result = window.eval("window.__nm_watchdog_ping = Date.now();");
                if let Err(e) = eval_result {
                    crate::services::logging::log_error(&format!(
                        "[Watchdog] Webview renderer error detected: {}. Attempting auto-recovery reload...",
                        e
                    ));
                    let _ = window.eval("window.location.reload();");
                }
            }

            // 1. Windows EmptyWorkingSet auto-trimming (RAM >= 1000 MB) — NO restart, NO logout
            if total_mem_mb >= TRIM_MEMORY_THRESHOLD_MB {
                crate::services::logging::log_info(&format!(
                    "[Watchdog] Elevated RAM ({} MB >= {} MB). Triggering Win32 EmptyWorkingSet trimming...",
                    total_mem_mb, TRIM_MEMORY_THRESHOLD_MB
                ));
                trim_process_tree_memory(&pids);
            }

            // 2. Warning Banner: If user is working actively and RAM >= 1800 MB, show non-intrusive banner
            if let Some(window) = app.get_webview_window("main") {
                if total_mem_mb >= WARNING_BANNER_THRESHOLD_MB && !is_idle {
                    show_memory_warning_banner(&window, total_mem_mb);
                    banner_visible = true;
                } else if banner_visible && total_mem_mb < 1400 {
                    remove_memory_warning_banner(&window);
                    banner_visible = false;
                }
            }

            // 3. Soft Refresh: If RAM >= 2200 MB AND user has been idle for 15+ min, reload page cleanly
            if total_mem_mb >= SOFT_RELOAD_IDLE_MB && is_idle {
                if let Some(window) = app.get_webview_window("main") {
                    soft_reload_page(&window, &format!("RAM elevated ({} MB) while idle for {} min", total_mem_mb, idle_min));
                    trim_process_tree_memory(&pids);
                    continue;
                }
            }

            // 4. Nightly scheduled 3:00 AM - 4:00 AM Soft Refresh while system is idle
            if current_hour >= RELAUNCH_START_HOUR && current_hour < RELAUNCH_END_HOUR && is_idle {
                let already_restarted_today = get_last_restart_date(&app).as_deref() == Some(&today_str);
                let has_minimum_uptime = start_time.elapsed().as_secs() >= MIN_UPTIME_BEFORE_NIGHTLY_RESTART_SECS;

                if !already_restarted_today && has_minimum_uptime {
                    set_last_restart_date(&app, &today_str);
                    if let Some(window) = app.get_webview_window("main") {
                        soft_reload_page(&window, &format!("Scheduled nightly 3 AM refresh while system idle for {} min", idle_min));
                        trim_process_tree_memory(&pids);
                        continue;
                    }
                }
            }

            // 5. Extreme catastrophic protection: Full process restart ONLY if RAM > 3500 MB while idle
            if total_mem_mb > HARD_RESTART_CRITICAL_MB && is_idle {
                let reason = format!(
                    "Critical memory exceeded ({} MB > {} MB) while idle for {} min",
                    total_mem_mb, HARD_RESTART_CRITICAL_MB, idle_min
                );
                restart_app(&app, &reason);
                return;
            }
        }
    });
}

fn restart_app(app: &AppHandle, reason: &str) {
    crate::services::logging::log_warn(&format!("[Watchdog] {}", reason));

    // Save current active URL before restarting so session and screen are restored seamlessly
    if let Some(window) = app.get_webview_window("main") {
        crate::services::logging::log_info("[Watchdog] Preserving active route and session state before restart...");
        let save_script = r#"
            try {
                if (window.location && window.location.href && !window.location.href.includes('tauri://') && !window.location.href.includes('localhost:5173')) {
                    localStorage.setItem('__nm_last_active_url', window.location.href);
                }
            } catch(e) {}
        "#;
        let _ = window.eval(save_script);
    }

    // Brief pause to allow storage write to finish in WebView
    thread::sleep(Duration::from_millis(250));

    crate::services::logging::log_info("[Watchdog] Restarting application now...");
    app.restart();
}
