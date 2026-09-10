import { invoke } from '@tauri-apps/api/core';

export interface AuthLogDetails {
    url?: string;
    status?: number;
    hasToken?: boolean;
    tokenExpiry?: string;
    message?: string;
}

/**
 * Sends a diagnostic log line to the native logging system (Asia/Kolkata timezone file).
 */
export async function logDiagnostic(
    level: 'INFO' | 'WARN' | 'ERROR',
    message: string,
): Promise<void> {
    return invoke<void>('log_diagnostic', { level, message });
}

/**
 * Logs authentication lifecycle events safely without leaking passwords or JWT secrets.
 */
export async function logAuthEvent(eventName: string, details: AuthLogDetails = {}): Promise<void> {
    return invoke<void>('log_auth_event', { eventName, details });
}
