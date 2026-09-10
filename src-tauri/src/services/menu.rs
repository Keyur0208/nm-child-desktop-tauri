use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Manager, Wry};

/// Creates the native desktop application menu with Electron-like File, Edit, View, Window, and Help menus.
pub fn create_app_menu(app: &AppHandle) -> Result<Menu<Wry>, tauri::Error> {
    // 0. macOS Application Menu
    #[cfg(target_os = "macos")]
    let app_menu = SubmenuBuilder::new(app, "Nilkanth Medico")
        .item(&PredefinedMenuItem::about(app, None, None)?)
        .separator()
        .item(&PredefinedMenuItem::services(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    // 1. File Menu
    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::with_id("reload", "Reload").accelerator("CmdOrCtrl+R").build(app)?)
        .item(&MenuItemBuilder::with_id("force_reload", "Force Reload").accelerator("CmdOrCtrl+Shift+R").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("print", "Print...").accelerator("CmdOrCtrl+P").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("exit", "Exit").accelerator("Alt+F4").build(app)?)
        .build()?;

    // 2. Edit Menu
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    // 3. View Menu
    let view_menu = SubmenuBuilder::new(app, "View")
        .item(&MenuItemBuilder::with_id("reload", "Reload").accelerator("CmdOrCtrl+R").build(app)?)
        .item(&MenuItemBuilder::with_id("force_reload", "Force Reload").accelerator("CmdOrCtrl+Shift+R").build(app)?)
        .item(&MenuItemBuilder::with_id("toggle_devtools", "Toggle Developer Tools").accelerator("CmdOrCtrl+Shift+I").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("zoom_reset", "Actual Size").accelerator("CmdOrCtrl+0").build(app)?)
        .item(&MenuItemBuilder::with_id("zoom_in", "Zoom In").accelerator("CmdOrCtrl+Plus").build(app)?)
        .item(&MenuItemBuilder::with_id("zoom_out", "Zoom Out").accelerator("CmdOrCtrl+-").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("toggle_fullscreen", "Toggle Full Screen").accelerator("F11").build(app)?)
        .item(&MenuItemBuilder::with_id("toggle_maximize", "Toggle Maximize").build(app)?)
        .build()?;

    // 4. Window Menu
    let window_menu = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&MenuItemBuilder::with_id("toggle_maximize", "Maximize / Restore").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;

    // 5. Help Menu
    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id("reconnect", "Check Connection / Reload").accelerator("F5").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("about", "About Nilkanth Medico HOMS").build(app)?)
        .build()?;

    // Main Menu Bar
    #[cfg(target_os = "macos")]
    let menu = MenuBuilder::new(app)
        .items(&[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu, &help_menu])
        .build();

    #[cfg(not(target_os = "macos"))]
    let menu = MenuBuilder::new(app)
        .items(&[&file_menu, &edit_menu, &view_menu, &window_menu, &help_menu])
        .build();

    menu
}

/// Handles all menu item click events and keyboard shortcuts
pub fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().as_ref();
    crate::services::logging::log_info(&format!("[Menu] Event clicked: {}", id));

    let main_win = match app.get_webview_window("main") {
        Some(w) => w,
        None => return,
    };

    match id {
        "exit" => {
            let a_handle = app.clone();
            let w_clone = main_win.clone();
            tauri::async_runtime::spawn(async move {
                let _ = crate::commands::window::confirm_and_close_app(w_clone, a_handle).await;
            });
        }
        "reload" => {
            crate::services::zoom::apply_current_zoom(&main_win);
            let _ = main_win.eval("window.location.reload();");
        }
        "force_reload" => {
            crate::services::zoom::apply_current_zoom(&main_win);
            let _ = main_win.eval("window.location.reload(true);");
        }
        "toggle_devtools" => {
            if main_win.is_devtools_open() {
                main_win.close_devtools();
            } else {
                main_win.open_devtools();
            }
        }
        "zoom_in" => {
            crate::services::zoom::zoom_in(&main_win);
        }
        "zoom_out" => {
            crate::services::zoom::zoom_out(&main_win);
        }
        "zoom_reset" => {
            crate::services::zoom::zoom_reset(&main_win);
        }
        "toggle_fullscreen" => {
            if let Ok(is_fs) = main_win.is_fullscreen() {
                let _ = main_win.set_fullscreen(!is_fs);
            }
        }
        "toggle_maximize" => {
            if let Ok(is_max) = main_win.is_maximized() {
                if is_max {
                    let _ = main_win.unmaximize();
                } else {
                    let _ = main_win.maximize();
                }
            }
        }
        "reconnect" => {
            crate::services::zoom::apply_current_zoom(&main_win);
            let _ = main_win.eval("window.location.reload();");
        }
        "about" => {
            use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
            app.dialog()
                .message("Nilkanth Medico - HOMS Child Desktop Application\nVersion: 0.0.1\n\nHospital ERP Client Terminal\nCopyright © Nilkanth Medico PVT LTD. All rights reserved.")
                .title("About Nilkanth Medico HOMS")
                .kind(MessageDialogKind::Info)
                .buttons(MessageDialogButtons::Ok)
                .show(|_| {});
        }
        _ => {}
    }
}
