import { invoke } from '@tauri-apps/api/core';

export interface StoragePaths {
    baseDir: string;
    imagesDir: string;
    documentsDir: string;
    pdfDir: string;
    cacheDir: string;
    logsDir: string;
    databaseDir: string;
}

export type StorageCategory = 'images' | 'documents' | 'pdf' | 'cache';

/**
 * Returns structured application storage directory paths.
 */
export async function getAppStoragePaths(): Promise<StoragePaths> {
    return invoke<StoragePaths>('get_app_storage_paths');
}

/**
 * Saves document/image bytes to scoped application storage.
 * Avoids storing large binaries as Base64 strings in memory or localStorage.
 */
export async function saveDocument(
    category: StorageCategory,
    filename: string,
    bytes: Uint8Array | number[],
): Promise<string> {
    const bytesArray = bytes instanceof Uint8Array ? Array.from(bytes) : bytes;
    return invoke<string>('save_document_file', {
        category,
        filename,
        bytes: bytesArray,
    });
}
