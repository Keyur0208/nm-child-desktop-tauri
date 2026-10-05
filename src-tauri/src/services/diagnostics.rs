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
const PERIODIC_TRIM_MINUTES: u64 = 30;         // Auto-trim RAM and DevTools cache every 30 minutes unconditionally
const CRITICAL_SYSTEM_FREE_RAM_MB: u64 = 500;  // Auto-trim if host PC has less than 500 MB free RAM
const HARD_RESTART_CRITICAL_MB: u64 = 3500;    // Only full process restart if memory exceeds 3500 MB while idle
const IDLE_MINUTES_THRESHOLD: u64 = 15;
const MICRO_IDLE_SECONDS_THRESHOLD: u64 = 90;  // 90s micro-gap for 24x7 hospital staff (between patient visits)
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
/// Also purges CacheStorage, Performance Resource Timings, and DevTools console protocol cache.
pub fn soft_reload_page(window: &tauri::WebviewWindow, reason: &str) {
    crate::services::logging::log_warn(&format!("[Watchdog] Executing Soft Refresh & Cache Eviction: {}", reason));
    let script = r#"
        try {
            // 1. Preserve active route URL so user returns to the exact same screen
            if (window.location && window.location.href && !window.location.href.includes('tauri://') && !window.location.href.includes('localhost:8081')) {
                localStorage.setItem('__nm_last_active_url', window.location.href);
            }
            // 2. Clear browser in-memory CacheStorage
            if (window.caches && caches.keys) {
                caches.keys().then(function(keys) {
                    keys.forEach(function(k) { caches.delete(k); });
                });
            }
            // 3. Clear Performance Resource Timings to free heap references
            if (window.performance && window.performance.clearResourceTimings) {
                window.performance.clearResourceTimings();
            }
            // 4. Auto-cleanup DevTools protocol & console cache
            if (window.console && typeof console.clear === 'function') {
                console.clear();
            }
            // 5. Fresh reload
            window.location.reload();
        } catch(e) {
            window.location.reload();
        }
    "#;
    let _ = window.eval(script);
}



/// Calculates dynamic RAM trim threshold based on host machine's total RAM.
/// - Low-end PC (<= 4 GB RAM): 350 MB threshold (keeps low-spec PCs snappy, avoids Windows pagefile thrashing)
/// - Mid-range PC (<= 8 GB RAM): 600 MB threshold (standard OPD/clinic PCs)
/// - High-end PC (> 8 GB RAM): 850 MB threshold (radiology/server workstations)
pub fn get_dynamic_ram_threshold_mb(total_system_ram_mb: u64) -> u64 {
    if total_system_ram_mb <= 4096 {
        350
    } else if total_system_ram_mb <= 8192 {
        600
    } else {
        850
    }
}

/// Calculates dynamic idle trim threshold based on host machine's total RAM.
pub fn get_dynamic_idle_trim_threshold_mb(total_system_ram_mb: u64) -> u64 {
    if total_system_ram_mb <= 4096 {
        250
    } else if total_system_ram_mb <= 8192 {
        400
    } else {
        550
    }
}

/// Calculates dynamic soft reload threshold (used ONLY when user is idle for 15+ min).
pub fn get_dynamic_soft_reload_threshold_mb(total_system_ram_mb: u64) -> u64 {
    if total_system_ram_mb <= 4096 {
        1200
    } else if total_system_ram_mb <= 8192 {
        2000
    } else {
        2800
    }
}

/// Silently purges DevTools console buffer and Performance Resource Timings in WebView without page reload or flicker.
pub fn purge_webview_cache_and_timings(window: &tauri::WebviewWindow) {
    let script = r#"
        try {
            if (window.performance && window.performance.clearResourceTimings) {
                window.performance.clearResourceTimings();
            }
            if (window.console && typeof console.clear === 'function') {
                console.clear();
            }
        } catch(e) {}
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

        sys.refresh_all();
        let total_system_ram_mb = sys.total_memory() / (1024 * 1024);
        let dynamic_trim_threshold_mb = get_dynamic_ram_threshold_mb(total_system_ram_mb);
        let dynamic_idle_trim_mb = get_dynamic_idle_trim_threshold_mb(total_system_ram_mb);
        let dynamic_soft_reload_mb = get_dynamic_soft_reload_threshold_mb(total_system_ram_mb);

        crate::services::logging::log_info(&format!(
            "[Watchdog] 24x7 Dynamic Adaptive Watchdog active | Host PC RAM: {} MB | Trim Threshold: {} MB | Interval: {}m | Soft Reload (Idle 15m): {} MB",
            total_system_ram_mb, dynamic_trim_threshold_mb, PERIODIC_TRIM_MINUTES, dynamic_soft_reload_mb
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
            let available_system_ram_mb = sys.available_memory() / (1024 * 1024);

            let now_kolkata = Utc::now().with_timezone(&Kolkata);
            let current_hour = now_kolkata.hour();
            let current_minute = now_kolkata.minute();
            let today_str = now_kolkata.format("%Y-%m-%d").to_string();

            // Periodic diagnostic memory log every 5 minutes or when RAM is elevated
            if loop_counter % 5 == 0 || total_mem_mb >= dynamic_trim_threshold_mb {
                crate::services::logging::log_info(&format!(
                    "[Diagnostics] App RAM: {} MB across {} procs | PC Free RAM: {} MB / {} MB | Idle: {} min | Time: {:02}:{:02} IST",
                    total_mem_mb, pids.len(), available_system_ram_mb, total_system_ram_mb, idle_min, current_hour, current_minute
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

            // 1. Silent Adaptive RAM Trimming & DevTools Cache Purge (Zero user disruption)
            // Triggers automatically without ANY popup/banner or page reload:
            // - Rule A: App RAM >= dynamic_trim_threshold_mb (350MB for 4GB PC, 600MB for 8GB PC, 850MB for 16GB PC)
            // - Rule B: Entire Host PC is running out of memory (< 500 MB free RAM)
            // - Rule C: Fixed 30-minute interval (independent of user activity)
            // - Rule D: User is briefly idle (>= 1 min) and RAM >= dynamic_idle_trim_mb
            let is_threshold_crossed = total_mem_mb >= dynamic_trim_threshold_mb;
            let is_system_low_ram = available_system_ram_mb < CRITICAL_SYSTEM_FREE_RAM_MB;
            let is_periodic_tick = loop_counter % PERIODIC_TRIM_MINUTES == 0;
            let is_idle_trim = idle_sec >= 60 && total_mem_mb >= dynamic_idle_trim_mb;

            if is_threshold_crossed || is_system_low_ram || is_periodic_tick || is_idle_trim {
                let trigger_reason = if is_threshold_crossed {
                    format!("App RAM >= {} MB threshold (Current: {} MB)", dynamic_trim_threshold_mb, total_mem_mb)
                } else if is_system_low_ram {
                    format!("Host PC low free RAM ({} MB < {} MB)", available_system_ram_mb, CRITICAL_SYSTEM_FREE_RAM_MB)
                } else if is_periodic_tick {
                    format!("Scheduled {}m periodic interval", PERIODIC_TRIM_MINUTES)
                } else {
                    format!("Brief idle ({}s) and RAM elevated ({} MB)", idle_sec, total_mem_mb)
                };

                crate::services::logging::log_info(&format!(
                    "[Watchdog] Silent memory trim & DevTools cache purge triggered: {}",
                    trigger_reason
                ));

                // A. OS Working Set release across all processes
                trim_process_tree_memory(&pids);

                // B. DevTools console references & Performance Resource Timings cleanup
                if let Some(window) = app.get_webview_window("main") {
                    purge_webview_cache_and_timings(&window);
                }
            }

            // 2. Soft Refresh: If RAM >= dynamic_soft_reload_mb
            // In a 24x7 hospital environment, staff rarely idles for 15 minutes.
            // We safely trigger soft reload if staff has a brief micro-gap (>= 90 seconds without typing/moving mouse)
            // or full idle (>= 15 min), completely preserving active URL and login session.
            let is_micro_idle = idle_sec >= MICRO_IDLE_SECONDS_THRESHOLD;
            if total_mem_mb >= dynamic_soft_reload_mb && (is_idle || is_micro_idle) {
                if let Some(window) = app.get_webview_window("main") {
                    soft_reload_page(&window, &format!("RAM elevated ({} MB >= {} MB) during 24x7 staff pause ({}s)", total_mem_mb, dynamic_soft_reload_mb, idle_sec));
                    trim_process_tree_memory(&pids);
                    continue;
                }
            }

            // 3. Nightly scheduled 3:00 AM - 4:00 AM Soft Refresh while system is idle
            if current_hour >= RELAUNCH_START_HOUR && current_hour < RELAUNCH_END_HOUR && (is_idle || is_micro_idle) {
                let already_restarted_today = get_last_restart_date(&app).as_deref() == Some(&today_str);
                let has_minimum_uptime = start_time.elapsed().as_secs() >= MIN_UPTIME_BEFORE_NIGHTLY_RESTART_SECS;

                if !already_restarted_today && has_minimum_uptime {
                    set_last_restart_date(&app, &today_str);
                    crate::services::cleaner::clean_browser_disk_cache(&app);
                    if let Some(window) = app.get_webview_window("main") {
                        soft_reload_page(&window, &format!("Scheduled nightly 3 AM refresh during lull ({}s)", idle_sec));
                        trim_process_tree_memory(&pids);
                        continue;
                    }
                }
            }

            // 4. Extreme catastrophic protection: Full process restart ONLY if RAM > 3500 MB while idle
            if total_mem_mb > HARD_RESTART_CRITICAL_MB && (is_idle || is_micro_idle) {
                let reason = format!(
                    "Critical memory exceeded ({} MB > {} MB) during pause ({}s)",
                    total_mem_mb, HARD_RESTART_CRITICAL_MB, idle_sec
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
                if (window.location && window.location.href && !window.location.href.includes('tauri://') && !window.location.href.includes('localhost:8081')) {
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
