// ---------------------------------------------------------------------------
// APP  —  branding / identity
// ---------------------------------------------------------------------------
export const APP = {
    name: 'Nilkanth Medico ERP',
    productName: 'Nilkanth Medico',
    company: 'Nilkanth Medico Private Limited',
    website: 'https://nilkanthmedico.com',
    supportEmail: 'support@nilkanthmedico.com',
} as const;

// ---------------------------------------------------------------------------
// ASSETS  —  relative paths from project root
// ---------------------------------------------------------------------------
export const ASSETS = {
    logo: 'logo.ico',
    offlinePage: 'public/offline.html',
} as const;

// ---------------------------------------------------------------------------
// SERVER  —  hospital local ERP server endpoints
// ---------------------------------------------------------------------------
export const SERVER = {
    port: 8081,
    localUrl: 'http://192.168.1.76:8081',
    serverlUrl: 'http://192.168.1.76:1111',
} as const;

// ---------------------------------------------------------------------------
// WINDOW  —  Window defaults
// ---------------------------------------------------------------------------
export const WINDOW = {
    width: 1400,
    height: 900,
    whiteScreenMs: 3000,
} as const;

// ---------------------------------------------------------------------------
// WATCHDOG  —  nightly scheduled relaunch + memory limit guard
// ---------------------------------------------------------------------------
export const WATCHDOG = {
    relaunchHour: 3,
    relaunchMinute: 0,
    maxMemoryMB: 2000,
    idleMinutes: 15,
    checkIntervalMs: 60_000,
} as const;

// ---------------------------------------------------------------------------
// MESSAGES  —  user-visible dialog strings
// ---------------------------------------------------------------------------
export const MESSAGES = {
    crash: {
        relaunchLog: '[Crash] Critical failure — relaunching app now',
        reloadLog: (reason: string) => `[Crash] Renderer exited (${reason}) — reloading in 2 s`,
        unresponsive: '[Crash] Renderer is UNRESPONSIVE — will reload in 5 s if still frozen',
        responsive: '[Crash] Renderer is responsive again',
    },
    window: {
        whiteScreen: '[Window] WHITE SCREEN detected — reloading automatically',
        clearFail: '[Window] Could not clear localStorage on close',
        injectFail: '[Window] Could not inject PC info into localStorage',
    },
} as const;

// ---------------------------------------------------------------------------
// STORAGE_KEYS  —  localStorage keys holding sensitive session data
// Cleared automatically when the app quits.
// ---------------------------------------------------------------------------
export const STORAGE_KEYS = [
    'indoorId',
    'billNo',
    'patientBillId',
    'isUpdate',
    'dischargeCardId',
    'receiptId',
    'endoLaproImageId',
    'navType',
] as const;
