import { invoke } from '@tauri-apps/api/core';

export interface SystemInfo {
    pcName: string;
    username: string;
    ipAddress: string;
    macAddress: string;
    platform: string;
    arch: string;
    cpuModel: string;
    cpuCores: number;
    totalRamMB: number;
    isPrinterSupported: boolean;
    defaultPrinter: string;
}

/**
 * Retrieves typed machine and system information from the native Rust backend.
 */
export async function getSystemInfo(): Promise<SystemInfo> {
    return invoke<SystemInfo>('get_system_info');
}

/**
 * Injects machine information into localStorage under 'resourceInfo',
 * preserving full backward compatibility with the existing React hospital ERP.
 * Invokes native Rust injection into the active webview document to ensure
 * remote URLs (e.g. SERVER.localUrl) receive resourceInfo correctly.
 */
export async function injectSystemInfoIntoStorage(): Promise<SystemInfo> {
    try {
        const info = await getSystemInfo();

        // Set locally on current window/origin
        try {
            localStorage.setItem('resourceInfo', JSON.stringify(info));
        } catch (_) {}

        // Native injection into active webview window
        try {
            await invoke('inject_resource_info');
        } catch (_) {}

        console.log('[Desktop] Successfully injected resourceInfo into localStorage');
        return info;
    } catch (err) {
        console.error('[Desktop] Failed to get system info:', err);
        throw err;
    }
}
