use tauri::{AppHandle, WebviewWindow};

const SENSITIVE_STORAGE_KEYS: &[&str] = &[
    "indoorId",
    "billNo",
    "patientBillId",
    "isUpdate",
    "dischargeCardId",
    "receiptId",
    "endoLaproImageId",
    "navType",
];

#[tauri::command]
pub async fn confirm_and_close_app(window: WebviewWindow, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let answer = app
        .dialog()
        .message("Are you sure you want to close the application?")
        .title("Exit Application")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNo)
        .blocking_show();

    if answer {
        crate::services::logging::log_info("[Window] User confirmed exit — clearing session storage...");

        // Clear sensitive session storage keys
        let clear_script = SENSITIVE_STORAGE_KEYS
            .iter()
            .map(|k| format!("try {{ localStorage.removeItem('{}'); }} catch(e) {{}}", k))
            .collect::<Vec<_>>()
            .join("\n");

        let _ = window.eval(&clear_script);

        crate::services::logging::log_info("[Window] Closing application window and exiting process");
        let _ = window.destroy();
        app.exit(0);
    }

    Ok(())
}

#[tauri::command]
pub async fn clear_session_keys(window: WebviewWindow) -> Result<(), String> {
    let clear_script = SENSITIVE_STORAGE_KEYS
        .iter()
        .map(|k| format!("try {{ localStorage.removeItem('{}'); }} catch(e) {{}}", k))
        .collect::<Vec<_>>()
        .join("\n");

    window.eval(&clear_script).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_window(window: WebviewWindow) -> Result<(), String> {
    if window.is_minimized().unwrap_or(false) {
        let _ = window.unminimize();
    }
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
    Ok(())
}

#[tauri::command]
pub async fn reload_window(window: WebviewWindow) -> Result<(), String> {
    crate::services::logging::log_info("[Window] Reloading window...");
    crate::services::zoom::apply_current_zoom(&window);
    window.eval("window.location.reload();").map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trim_memory() -> Result<u64, String> {
    let (before, after) = crate::services::diagnostics::trim_memory_now();
    crate::services::logging::log_info(&format!(
        "[Window] User / Banner initiated memory trim: {} MB -> {} MB",
        before, after
    ));
    Ok(after)
}

