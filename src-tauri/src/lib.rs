pub mod commands;
pub mod services;
pub mod utils;

use tauri::webview::{DownloadEvent, PageLoadEvent};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            services::logging::log_info(&format!(
                "[App] Second instance blocked — argv: {}",
                argv.join(" ")
            ));
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.request_user_attention(Some(tauri::UserAttentionType::Critical));
                services::logging::log_info("[App] Existing window restored and focused");
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .on_menu_event(services::menu::handle_menu_event)
        .on_page_load(|webview, payload| {
            match payload.event() {
                PageLoadEvent::Started => {
                    services::zoom::apply_current_zoom_webview(webview);
                }
                PageLoadEvent::Finished => {
                    let info = commands::system::get_system_info_internal();
                    let script = commands::system::generate_injection_script(&info);
                    let _ = webview.eval(&script);

                    // Reinforce application-level native zoom level across page loads and reloads
                    services::zoom::apply_current_zoom_webview(webview);
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();

            // Initialize daily logging system
            services::logging::init_logger(&handle);
            services::logging::log_info("============================================================");
            services::logging::log_info("[App] Nilkanth Medico Hospital ERP Child — STARTING (Tauri 2.x)");
            services::logging::log_info(&format!("[App] OS PID: {}", std::process::id()));
            services::logging::log_info("============================================================");

            // Initialize app storage directories (images, documents, pdf, logs, database)
            let _ = commands::filesystem::get_app_storage_dir(&handle);

            // Initialize zoom persistence from saved storage
            services::zoom::init_zoom(&handle);

            // Start 30-day automatic log cleanup thread
            services::cleaner::start_log_cleaner(handle.clone());

            // Start 24x7 memory & nightly 3:00 AM idle watchdog
            services::diagnostics::start_memory_watchdog(handle.clone());

            // Build main window with native preloading initialization script
            let sys_info = commands::system::get_system_info_internal();
            let sys_script = commands::system::generate_injection_script(&sys_info);
            let zoom_script = services::zoom::get_zoom_init_script();
            let init_script = format!("{}\n{}", sys_script, zoom_script);
            services::logging::log_info("[App] Initializing main window with native resourceInfo and zoom injection scripts");

            let window = WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::default(),
            )
            .title("Nilkanth Medico - HOMS Child")
            .inner_size(1400.0, 900.0)
            .min_inner_size(1024.0, 700.0)
            .resizable(true)
            .maximized(true)
            .fullscreen(false)
            .decorations(true)
            .additional_browser_args("--enable-gpu-rasterization --enable-zero-copy --ignore-gpu-blocklist --disable-renderer-backgrounding --disk-cache-size=268435456 --js-flags=--max-old-space-size=4096")
            .initialization_script(&init_script)
            .on_download(|webview, event| {
    match event {
        DownloadEvent::Requested { url, destination } => {
            use tauri_plugin_dialog::DialogExt;

            let filename = destination
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "download".to_string());

            services::logging::log_info(&format!(
                "[Download] Requested: url={}, filename={}",
                url, filename
            ));

            let builder = webview
                .dialog()
                .file()
                .set_title("Save File")
                .set_file_name(&filename);

            match builder.blocking_save_file() {
                Some(selected_path) => {
                    let chosen_path = match selected_path.into_path() {
                        Ok(path) => path,
                        Err(_) => {
                            services::logging::log_error(
                                "[Download] Invalid selected path"
                            );
                            return false;
                        }
                    };

                    services::logging::log_info(&format!(
                        "[Download] Save path selected: {}",
                        chosen_path.display()
                    ));

                    *destination = chosen_path;

                    true
                }

                None => {
                    services::logging::log_info(
                        "[Download] User cancelled Save As"
                    );

                    false
                }
            }
        }

        DownloadEvent::Finished {
            url,
            path,
            success,
        } => {
            services::logging::log_info(&format!(
                "[Download] Finished: url={}, path={:?}, success={}",
                url, path, success
            ));

            true
        }

        _ => true,
    }
})
            .build()?;

            // Apply initial zoom level
            services::zoom::apply_current_zoom(&window);

            // Build and attach Electron-like native application menu bar
            match services::menu::create_app_menu(&handle) {
                Ok(menu) => {
                    #[cfg(target_os = "macos")]
                    let _ = handle.set_menu(menu.clone());

                    let _ = window.set_menu(menu);
                    services::logging::log_info("[App] Electron-style native menu bar attached successfully");
                }
                Err(err) => {
                    services::logging::log_error(&format!("[App] Failed to build native menu: {}", err));
                }
            }

            let app_handle = handle.clone();
            let win_clone = window.clone();

            // Intercept window close to prompt confirmation and clean sensitive keys
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let a_handle = app_handle.clone();
                    let w_clone = win_clone.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = commands::window::confirm_and_close_app(w_clone, a_handle).await;
                    });
                }
            });

            services::logging::log_info("[App] All Tauri native modules ready — ERP is RUNNING");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_system_info,
            commands::system::inject_resource_info,
            commands::printer::get_printers,
            commands::printer::silent_print,
            commands::printer::silent_print_pdf,
            commands::filesystem::get_app_storage_paths,
            commands::filesystem::save_document_file,
            commands::window::confirm_and_close_app,
            commands::window::clear_session_keys,
            commands::window::restore_window,
            commands::window::reload_window,
            commands::window::trim_memory,
            services::logging::log_diagnostic,
            services::logging::log_auth_event,
            services::zoom::get_zoom_level,
            services::zoom::set_zoom_level,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
