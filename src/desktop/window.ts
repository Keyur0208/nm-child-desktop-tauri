import { invoke } from '@tauri-apps/api/core';

/**
 * Prompts native exit confirmation dialog, clears sensitive session keys,
 * and cleanly closes the application.
 */
export async function confirmAndCloseApp(): Promise<void> {
    return invoke<void>('confirm_and_close_app');
}

/**
 * Clears sensitive hospital session keys from localStorage.
 */
export async function clearSessionKeys(): Promise<void> {
    return invoke<void>('clear_session_keys');
}

/**
 * Brings window to front, restores if minimized, and flashes taskbar.
 */
export async function restoreWindow(): Promise<void> {
    return invoke<void>('restore_window');
}

/**
 * Reloads the main application window.
 */
export async function reloadWindow(): Promise<void> {
    return invoke<void>('reload_window');
}

/**
 * Cleanly restarts / relaunches the desktop application process.
 */
export async function relaunchApp(): Promise<void> {
    const { relaunch } = await import('@tauri-apps/plugin-process');
    return relaunch();
}

