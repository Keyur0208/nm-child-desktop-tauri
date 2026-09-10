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
        .menu(services::menu::create_app_menu)
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
            let client_enhancements = get_client_enhancements_script();
            let init_script = format!("{}\n{}\n{}", sys_script, zoom_script, client_enhancements);
            services::logging::log_info("[App] Initializing main window with native resourceInfo, zoom, and download enhancement scripts");

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
                        let filename = destination
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_else(|| "download.pdf".to_string());

                        services::logging::log_info(&format!(
                            "[Download] Requested: url={}, filename={}",
                            url, filename
                        ));

                        #[cfg(target_os = "macos")]
                        {
                            let app = webview.app_handle();
                            let download_dir = app
                                .path()
                                .download_dir()
                                .or_else(|_| app.path().home_dir().map(|h| h.join("Downloads")))
                                .unwrap_or_else(|_| std::env::temp_dir());

                            if !download_dir.exists() {
                                let _ = std::fs::create_dir_all(&download_dir);
                            }

                            let mut target = download_dir.join(&filename);
                            if target.exists() {
                                let stem = target.file_stem().and_then(|s| s.to_str()).unwrap_or("download");
                                let ext = target.extension().and_then(|e| e.to_str()).unwrap_or("");
                                let now = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let new_name = if ext.is_empty() {
                                    format!("{}_{}", stem, now)
                                } else {
                                    format!("{}_{}.{}", stem, now, ext)
                                };
                                target = download_dir.join(new_name);
                            }

                            services::logging::log_info(&format!(
                                "[Download] macOS saving directly to Downloads: {:?}",
                                target
                            ));
                            *destination = target;
                            true
                        }

                        #[cfg(not(target_os = "macos"))]
                        {
                            use tauri_plugin_dialog::DialogExt;
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

            // Ensure macOS activation policy is Regular (enables native menu bar & Dock)
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            // On Windows / Linux, ensure window-level menu bar is explicitly attached
            #[cfg(not(target_os = "macos"))]
            if let Ok(menu) = services::menu::create_app_menu(&handle) {
                let _ = window.set_menu(menu);
            }
            services::logging::log_info("[App] Electron-style native menu bar initialized successfully");

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
            commands::filesystem::save_download_bytes,
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

/// Injected client-side script that handles macOS WKWebView blob downloads and exact print color reproduction
fn get_client_enhancements_script() -> &'static str {
    r#"
    (function() {
        // 1. Inject Print Exact Color CSS for macOS WKWebView & modern browsers
        function injectPrintStyles() {
            try {
                if (!document.getElementById('__nm_print_styles__')) {
                    var style = document.createElement('style');
                    style.id = '__nm_print_styles__';
                    style.textContent = [
                        '@media print {',
                        '    *, *::before, *::after {',
                        '        -webkit-print-color-adjust: exact !important;',
                        '        print-color-adjust: exact !important;',
                        '    }',
                        '    html, body {',
                        '        overflow: visible !important;',
                        '        height: auto !important;',
                        '    }',
                        '}'
                    ].join('\n');
                    (document.head || document.documentElement).appendChild(style);
                }
            } catch (e) {}
        }

        // 2. Download Feedback Toast
        function showDownloadToast(msg, isSuccess) {
            try {
                var toast = document.createElement('div');
                toast.textContent = msg;
                toast.style.cssText = 'position:fixed;bottom:24px;right:24px;background:' + (isSuccess ? '#10b981' : '#ef4444') + ';color:#ffffff;padding:12px 20px;border-radius:8px;font-family:system-ui,-apple-system,sans-serif;font-size:13px;font-weight:600;box-shadow:0 10px 25px rgba(0,0,0,0.3);z-index:999999;transition:opacity 0.3s ease,transform 0.3s ease;transform:translateY(0);opacity:1;pointer-events:none;';
                (document.body || document.documentElement).appendChild(toast);
                setTimeout(function() {
                    toast.style.opacity = '0';
                    toast.style.transform = 'translateY(10px)';
                    setTimeout(function() {
                        if (toast.parentElement) toast.parentElement.removeChild(toast);
                    }, 350);
                }, 4000);
            } catch (e) {}
        }

        // 3. Save Blob / Data URLs directly to Downloads via native Rust command
        function saveBlobNative(href, filename) {
            console.log('[Tauri Native] Intercepted blob download:', filename);
            fetch(href)
                .then(function(res) { return res.arrayBuffer(); })
                .then(function(buffer) {
                    var bytes = Array.from(new Uint8Array(buffer));
                    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
                        window.__TAURI_INTERNALS__.invoke('save_download_bytes', {
                            filename: filename,
                            bytes: bytes
                        }).then(function(savedPath) {
                            if (savedPath === 'CANCELLED') {
                                console.log('[Tauri Native] File download cancelled by user');
                                return;
                            }
                            console.log('[Tauri Native] File downloaded and saved successfully:', savedPath);
                            var baseName = savedPath ? (savedPath.split('/').pop().split('\\').pop() || filename) : filename;
                            showDownloadToast('✓ Saved: ' + baseName, true);
                        }).catch(function(err) {
                            console.error('[Tauri Native] Failed to save file:', err);
                            showDownloadToast('⚠ Failed to save: ' + (err || 'Unknown error'), false);
                        });
                    }
                })
                .catch(function(err) {
                    console.error('[Tauri Native] Failed to fetch blob for download:', err);
                    showDownloadToast('⚠ Download fetch error: ' + err, false);
                });
        }

        // 4. Intercept Blob / Data URL Downloads on macOS WKWebView
        function setupBlobDownloadInterceptor() {
            if (window.__nm_download_interceptor_set__) return;
            window.__nm_download_interceptor_set__ = true;

            // Intercept standard click events
            document.addEventListener('click', function(e) {
                var el = e.target;
                while (el && el.tagName !== 'A') {
                    el = el.parentElement;
                }
                if (!el || el.tagName !== 'A') return;

                var hasDownload = el.hasAttribute('download');
                var href = el.getAttribute('href') || el.href || '';

                if (hasDownload && (href.startsWith('blob:') || href.startsWith('data:'))) {
                    e.preventDefault();
                    e.stopPropagation();

                    var filename = el.getAttribute('download') || 'download.pdf';
                    saveBlobNative(href, filename);
                }
            }, true);

            // Intercept programmatic .click() on dynamically created unattached anchor elements
            try {
                var origClick = HTMLAnchorElement.prototype.click;
                HTMLAnchorElement.prototype.click = function() {
                    var hasDownload = this.hasAttribute('download');
                    var href = this.getAttribute('href') || this.href || '';
                    if (hasDownload && (href.startsWith('blob:') || href.startsWith('data:'))) {
                        var filename = this.getAttribute('download') || 'download.pdf';
                        saveBlobNative(href, filename);
                        return;
                    }
                    return origClick.apply(this, arguments);
                };
            } catch (e) {}
        }

        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', function() {
                injectPrintStyles();
                setupBlobDownloadInterceptor();
            });
        } else {
            injectPrintStyles();
            setupBlobDownloadInterceptor();
        }
        window.addEventListener('pageshow', function() {
            injectPrintStyles();
            setupBlobDownloadInterceptor();
        });
    })();
    "#
}
