use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Webview, WebviewWindow};

/// Standard zoom levels matching desktop browser zoom steps:
/// 50%, 60%, 67%, 70%, 75%, 80%, 90%, 100%, 110%, 125%, 150%, 175%, 200%, 250%
pub const ZOOM_LEVELS: &[f64] = &[
    0.50, 0.60, 0.67, 0.70, 0.75, 0.80, 0.90, 1.00, 1.10, 1.25, 1.50, 1.75, 2.00, 2.50,
];

pub const DEFAULT_ZOOM: f64 = 1.00;
pub const MIN_ZOOM: f64 = 0.50;
pub const MAX_ZOOM: f64 = 2.50;

static CURRENT_ZOOM: Mutex<f64> = Mutex::new(DEFAULT_ZOOM);
static ZOOM_STORAGE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Initialize zoom service: sets storage path and loads persisted zoom level from disk
pub fn init_zoom(app: &AppHandle) {
    if let Ok(base) = app.path().app_data_dir() {
        if !base.exists() {
            let _ = fs::create_dir_all(&base);
        }
        let zoom_file = base.join("zoom.txt");

        // Load saved zoom if file exists
        if zoom_file.exists() {
            if let Ok(content) = fs::read_to_string(&zoom_file) {
                if let Ok(saved) = content.trim().parse::<f64>() {
                    let clamped = saved.clamp(MIN_ZOOM, MAX_ZOOM);
                    let rounded = (clamped * 100.0).round() / 100.0;
                    let mut guard = CURRENT_ZOOM.lock().unwrap_or_else(|p| p.into_inner());
                    *guard = rounded;
                    crate::services::logging::log_info(&format!(
                        "[Zoom] Loaded saved zoom level from disk: {:.2}",
                        rounded
                    ));
                }
            }
        }

        let mut path_guard = ZOOM_STORAGE_PATH.lock().unwrap_or_else(|p| p.into_inner());
        *path_guard = Some(zoom_file);
    }
}

/// Save current zoom to disk
fn persist_zoom(zoom: f64) {
    let path_guard = ZOOM_STORAGE_PATH.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(path) = path_guard.as_ref() {
        let _ = fs::write(path, format!("{:.2}", zoom));
    }
}

/// Get the current application-level zoom factor (e.g. 1.0 = 100%, 0.8 = 80%)
pub fn get_current_zoom() -> f64 {
    *CURRENT_ZOOM.lock().unwrap_or_else(|p| p.into_inner())
}

/// Set the current application-level zoom factor and persist it to disk
pub fn set_current_zoom(zoom: f64) {
    let clamped = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    let rounded = (clamped * 100.0).round() / 100.0;
    {
        let mut guard = CURRENT_ZOOM.lock().unwrap_or_else(|p| p.into_inner());
        *guard = rounded;
    }
    persist_zoom(rounded);
}

/// Calculate the next zoom step up
pub fn calculate_zoom_in(current: f64) -> f64 {
    for &level in ZOOM_LEVELS {
        if level > current + 0.005 {
            return level;
        }
    }
    MAX_ZOOM
}

/// Calculate the next zoom step down
pub fn calculate_zoom_out(current: f64) -> f64 {
    for &level in ZOOM_LEVELS.iter().rev() {
        if level < current - 0.005 {
            return level;
        }
    }
    MIN_ZOOM
}

/// Clear any lingering CSS zoom on document.body, persist to localStorage, and update window.__NM_DESKTOP_ZOOM__
pub fn generate_cleanup_script(zoom: f64) -> String {
    format!(
        r#"
        (() => {{
            try {{
                if (document.body && document.body.style.zoom) {{
                    document.body.style.zoom = '';
                }}
                localStorage.setItem('nm_desktop_zoom', '{zoom:.2}');
            }} catch (e) {{}}
            window.__NM_DESKTOP_ZOOM__ = {zoom:.2};
        }})();
        "#
    )
}

/// Apply native WebView zoom to a WebviewWindow
pub fn apply_current_zoom(window: &WebviewWindow) {
    let zoom = get_current_zoom();
    if let Err(e) = window.set_zoom(zoom) {
        crate::services::logging::log_error(&format!("[Zoom] Failed to apply window zoom {:.2}: {}", zoom, e));
    } else {
        crate::services::logging::log_info(&format!("[Zoom] Native window zoom applied: {:.2}", zoom));
    }
    let script = generate_cleanup_script(zoom);
    let _ = window.eval(&script);
}

/// Apply native WebView zoom to a Webview instance (used in on_page_load)
pub fn apply_current_zoom_webview(webview: &Webview) {
    let zoom = get_current_zoom();
    if let Err(e) = webview.set_zoom(zoom) {
        crate::services::logging::log_error(&format!("[Zoom] Failed to apply webview zoom {:.2}: {}", zoom, e));
    } else {
        crate::services::logging::log_info(&format!("[Zoom] Native webview zoom applied: {:.2}", zoom));
    }
    let script = generate_cleanup_script(zoom);
    let _ = webview.eval(&script);
}

/// Handles zoom-in action from menu or keyboard shortcut
pub fn zoom_in(window: &WebviewWindow) -> f64 {
    let cur = get_current_zoom();
    let next = calculate_zoom_in(cur);
    set_current_zoom(next);
    apply_current_zoom(window);
    crate::services::logging::log_info(&format!("[Zoom] Zoom in: {:.2} -> {:.2}", cur, next));
    next
}

/// Handles zoom-out action from menu or keyboard shortcut
pub fn zoom_out(window: &WebviewWindow) -> f64 {
    let cur = get_current_zoom();
    let next = calculate_zoom_out(cur);
    set_current_zoom(next);
    apply_current_zoom(window);
    crate::services::logging::log_info(&format!("[Zoom] Zoom out: {:.2} -> {:.2}", cur, next));
    next
}

/// Handles zoom-reset action from menu or keyboard shortcut
pub fn zoom_reset(window: &WebviewWindow) -> f64 {
    let cur = get_current_zoom();
    set_current_zoom(DEFAULT_ZOOM);
    apply_current_zoom(window);
    crate::services::logging::log_info(&format!("[Zoom] Zoom reset: {:.2} -> {:.2}", cur, DEFAULT_ZOOM));
    DEFAULT_ZOOM
}

/// Returns the client-side preloading initialization script.
/// Ensures no conflicting CSS zoom is attached to document.body,
/// syncs localStorage 'nm_desktop_zoom' across restarts,
/// and preserves native Popper, MUI Menu, Popover, and Portal coordinate spaces.
pub fn get_zoom_init_script() -> &'static str {
    r#"
    (function() {
        function ensureNoCssZoom() {
            try {
                if (document.body && document.body.style.zoom) {
                    document.body.style.zoom = '';
                }
            } catch (e) {}
        }

        // On app/page load, check if there is a saved zoom level in localStorage
        try {
            var saved = localStorage.getItem('nm_desktop_zoom');
            if (saved) {
                var z = parseFloat(saved);
                if (!isNaN(z) && z >= 0.50 && z <= 2.50) {
                    window.__NM_DESKTOP_ZOOM__ = z;
                    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
                        window.__TAURI_INTERNALS__.invoke('set_zoom_level', { zoom: z });
                    }
                }
            }
        } catch (e) {}

        if (document.body) {
            ensureNoCssZoom();
        } else if (document.documentElement) {
            var observer = new MutationObserver(function() {
                if (document.body) {
                    ensureNoCssZoom();
                    observer.disconnect();
                }
            });
            observer.observe(document.documentElement, { childList: true });
        }

        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', ensureNoCssZoom);
        }
        window.addEventListener('pageshow', ensureNoCssZoom);
    })();
    "#
}

#[tauri::command]
pub async fn get_zoom_level() -> Result<f64, String> {
    Ok(get_current_zoom())
}

#[tauri::command]
pub async fn set_zoom_level(window: WebviewWindow, zoom: f64) -> Result<f64, String> {
    set_current_zoom(zoom);
    apply_current_zoom(&window);
    Ok(get_current_zoom())
}
