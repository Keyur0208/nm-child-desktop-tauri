import { invoke } from '@tauri-apps/api/core';

export interface PrinterInfo {
    name: string;
    displayName: string;
    isDefault: boolean;
    status: number;
}

export interface PrintResult {
    success: boolean;
    code?: string;
    message?: string;
}

/**
 * Discovers all Windows printers installed on the workstation.
 */
export async function getPrinters(): Promise<PrinterInfo[]> {
    return invoke<PrinterInfo[]>('get_printers');
}

/**
 * Silently prints a PDF to the specified Windows printer without user dialogs.
 * Accepts raw PDF bytes (Uint8Array, ArrayBuffer, or number array).
 */
export async function silentPrintPdf(
    pdfBytes: Uint8Array | ArrayBuffer | number[],
    printerName: string,
): Promise<PrintResult> {
    const bytesArray =
        pdfBytes instanceof Uint8Array
            ? Array.from(pdfBytes)
            : pdfBytes instanceof ArrayBuffer
              ? Array.from(new Uint8Array(pdfBytes))
              : pdfBytes;

    return invoke<PrintResult>('silent_print_pdf', {
        pdfBytes: bytesArray,
        printerName,
    });
}

/**
 * Triggers silent document/receipt printing on the specified printer.
 */
export async function silentPrint(printerName?: string): Promise<PrintResult> {
    return invoke<PrintResult>('silent_print', { printerName });
}
