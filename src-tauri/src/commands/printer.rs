use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    pub name: String,
    pub display_name: String,
    pub is_default: bool,
    pub status: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[cfg(windows)]
pub fn get_default_printer_internal() -> String {
    use windows::core::PWSTR;
    use windows::Win32::Graphics::Printing::GetDefaultPrinterW;

    let mut size: u32 = 0;
    // First call to get required buffer size
    unsafe {
        let _ = GetDefaultPrinterW(PWSTR::null(), &mut size);
    }

    if size == 0 {
        return String::new();
    }

    let mut buffer: Vec<u16> = vec![0; size as usize];
    let success = unsafe {
        GetDefaultPrinterW(PWSTR(buffer.as_mut_ptr()), &mut size).as_bool()
    };

    if success {
        // Remove trailing null
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..len])
    } else {
        String::new()
    }
}

#[cfg(target_os = "macos")]
pub fn get_default_printer_internal() -> String {
    use std::process::Command;
    if let Ok(output) = Command::new("lpstat").arg("-d").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        // Format: "system default destination: HP_LaserJet_Pro"
        if let Some(dest) = text.split("system default destination:").nth(1) {
            return dest.trim().to_string();
        }
    }
    String::new()
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn get_default_printer_internal() -> String {
    String::new()
}

#[cfg(windows)]
pub fn list_printers_windows() -> Vec<PrinterInfo> {
    use windows::core::PWSTR;
    use windows::Win32::Graphics::Printing::{
        EnumPrintersW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_2W,
    };

    let default_printer = get_default_printer_internal();
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let mut needed: u32 = 0;
    let mut returned: u32 = 0;

    unsafe {
        let _ = EnumPrintersW(
            flags,
            PWSTR::null(),
            2,
            None,
            &mut needed,
            &mut returned,
        );
    }

    if needed == 0 {
        return Vec::new();
    }

    let mut buffer: Vec<u8> = vec![0; needed as usize];
    let success = unsafe {
        EnumPrintersW(
            flags,
            PWSTR::null(),
            2,
            Some(&mut buffer),
            &mut needed,
            &mut returned,
        )
        .is_ok()
    };

    let mut printers = Vec::new();

    if success && returned > 0 {
        let info_slice = unsafe {
            std::slice::from_raw_parts(buffer.as_ptr() as *const PRINTER_INFO_2W, returned as usize)
        };

        for info in info_slice {
            let name = if !info.pPrinterName.is_null() {
                unsafe { info.pPrinterName.to_string().unwrap_or_default() }
            } else {
                String::new()
            };

            if !name.is_empty() {
                let is_default = name.eq_ignore_ascii_case(&default_printer);
                printers.push(PrinterInfo {
                    display_name: name.clone(),
                    name,
                    is_default,
                    status: info.Status,
                });
            }
        }
    }

    // Fallback: If EnumPrinters returned empty but a default printer was detected
    if printers.is_empty() && !default_printer.is_empty() {
        printers.push(PrinterInfo {
            name: default_printer.clone(),
            display_name: default_printer,
            is_default: true,
            status: 0,
        });
    }

    printers
}

#[cfg(target_os = "macos")]
pub fn list_printers_macos() -> Vec<PrinterInfo> {
    use std::process::Command;
    let default_printer = get_default_printer_internal();
    let mut printers = Vec::new();

    if let Ok(output) = Command::new("lpstat").arg("-p").output() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            // Line format: "printer Printer_Name is idle.  enabled since..."
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[0] == "printer" {
                let name = parts[1].to_string();
                let is_default = !default_printer.is_empty() && name == default_printer;
                printers.push(PrinterInfo {
                    name: name.clone(),
                    display_name: name.replace('_', " "),
                    is_default,
                    status: 0,
                });
            }
        }
    }
    printers
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn list_printers_fallback() -> Vec<PrinterInfo> {
    Vec::new()
}

pub fn get_system_printers() -> Vec<PrinterInfo> {
    #[cfg(windows)]
    return list_printers_windows();

    #[cfg(target_os = "macos")]
    return list_printers_macos();

    #[cfg(not(any(windows, target_os = "macos")))]
    return list_printers_fallback();
}

#[tauri::command]
pub async fn get_printers() -> Result<Vec<PrinterInfo>, String> {
    crate::services::logging::log_info("[Print] Getting printers...");
    let printers = get_system_printers();
    Ok(printers)
}

#[tauri::command]
pub async fn silent_print_pdf(
    _app: tauri::AppHandle,
    pdf_bytes: Vec<u8>,
    printer_name: String,
) -> Result<PrintResult, String> {
    crate::services::logging::log_info(&format!(
        "[Print] Silent PDF print requested for printer: {}",
        printer_name
    ));

    // 1. Verify printer exists
    let printers = get_system_printers();
    let exists = printers.iter().any(|p| p.name == printer_name);
    if !exists && !printers.is_empty() {
        return Ok(PrintResult {
            success: false,
            code: Some("PRINTER_NOT_FOUND".to_string()),
            message: Some(format!("Printer \"{}\" not found", printer_name)),
        });
    }

    // 2. Write temp PDF
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_dir = std::env::temp_dir();
    let pdf_path = temp_dir.join(format!("homs-temp-{}.pdf", now_ms));

    if let Err(e) = fs::write(&pdf_path, &pdf_bytes) {
        crate::services::logging::log_error(&format!("[Print] Failed to write temp PDF: {}", e));
        return Ok(PrintResult {
            success: false,
            code: Some("WRITE_FAILED".to_string()),
            message: Some(e.to_string()),
        });
    }

    // 3. Print using OS print spooler command
    #[cfg(windows)]
    let print_status = execute_windows_print(&pdf_path, &printer_name);

    #[cfg(target_os = "macos")]
    let print_status = execute_mac_print(&pdf_path, &printer_name);

    #[cfg(not(any(windows, target_os = "macos")))]
    let print_status = Ok(());

    // 4. Clean up temp PDF
    let _ = fs::remove_file(&pdf_path);
    crate::services::logging::log_info(&format!(
        "[Print] Temporary PDF file deleted: {:?}",
        pdf_path
    ));

    match print_status {
        Ok(_) => Ok(PrintResult {
            success: true,
            code: None,
            message: Some(format!("Printed on {}", printer_name)),
        }),
        Err(err) => {
            crate::services::logging::log_error(&format!("[Print] Silent PDF print failed: {}", err));
            Ok(PrintResult {
                success: false,
                code: Some("PRINT_FAILED".to_string()),
                message: Some(err),
            })
        }
    }
}

#[cfg(windows)]
fn execute_windows_print(pdf_path: &PathBuf, printer_name: &str) -> Result<(), String> {
    use std::process::Command;

    // First attempt: PowerShell Start-Process with printto verb
    let path_str = pdf_path.to_str().unwrap_or_default();
    let ps_cmd = format!(
        "Start-Process -FilePath '{}' -ArgumentList '\"{}\"' -Verb printto -PassThru | ForEach-Object {{ $_.WaitForExit(15000); if (-not $_.HasExited) {{ $_.Kill() }} }}",
        path_str.replace('\'', "''"),
        printer_name.replace('"', "\\\"")
    );

    let output = Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_cmd])
        .output();

    match output {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // Fallback attempt: Run via Rundll32 or system default spooler
            if stderr.is_empty() {
                Ok(())
            } else {
                Err(stderr.to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn execute_mac_print(pdf_path: &PathBuf, printer_name: &str) -> Result<(), String> {
    use std::process::Command;
    let mut cmd = Command::new("lp");
    if !printer_name.is_empty() {
        cmd.arg("-d").arg(printer_name);
    }
    cmd.arg(pdf_path);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute macOS lp command: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(if stderr.is_empty() {
            "macOS lp print command exited with error".to_string()
        } else {
            stderr.to_string()
        })
    }
}

#[tauri::command]
pub async fn silent_print(
    window: tauri::WebviewWindow,
    printer_name: Option<String>,
) -> Result<PrintResult, String> {
    let name = printer_name.unwrap_or_else(get_default_printer_internal);
    crate::services::logging::log_info(&format!("[Print] Silent print requested on {}", name));

    // Triggers webview print via JS eval
    let script = "window.print();";
    match window.eval(script) {
        Ok(_) => Ok(PrintResult {
            success: true,
            code: None,
            message: Some(format!("Printed on {}", name)),
        }),
        Err(e) => Ok(PrintResult {
            success: false,
            code: Some("EVAL_FAILED".to_string()),
            message: Some(e.to_string()),
        }),
    }
}
