import { injectSystemInfoIntoStorage } from './desktop/system';
import { logDiagnostic } from './desktop/diagnostics';
import { SERVER } from './config/index';

// Initialize desktop machine info into localStorage
injectSystemInfoIntoStorage()
    .then(() => {
        logDiagnostic('INFO', '[App] Desktop system info initialized successfully.');
    })
    .catch((err) => {
        console.error('[App] Initialization error:', err);
    });

const statusTitle = document.getElementById('status-title');
const statusSubtitle = document.getElementById('status-subtitle');
const serverUrl = document.getElementById('server-url');
const statusPill = document.getElementById('status-pill');
const statusDot = document.getElementById('status-dot');
const statusText = document.getElementById('status-text');
const btnRetry = document.getElementById('btn-retry') as HTMLButtonElement | null;
const countdown = document.getElementById('countdown');
const timerVal = document.getElementById('timer-val');
const iconContainer = document.getElementById('icon-container');

// Immediately render dynamic server URL from SERVER.localUrl config
if (serverUrl) {
    serverUrl.textContent = SERVER.localUrl;
}

let retrySeconds = 5;
let retryInterval: number | null = null;
let isChecking = false;

function setOfflineState(message: string) {
    if (statusTitle) statusTitle.textContent = 'Server Connection Lost';
    if (statusSubtitle) {
        statusSubtitle.innerHTML =
            `Unable to reach Nilkanth Medico ERP server at <code>${SERVER.localUrl}</code>.<br />` +
            `The hospital server may be starting up or temporarily unavailable. (${message})`;
    }
    if (statusPill) {
        statusPill.className = 'status-pill';
    }
    if (statusDot) {
        statusDot.className = 'dot';
    }
    if (statusText) {
        statusText.textContent = 'Waiting for server…';
    }
    if (iconContainer) {
        iconContainer.className = 'icon-wrap';
        iconContainer.innerHTML = `
            <svg viewBox="0 0 24 24" fill="none" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="9" y="2" width="6" height="20" rx="1" />
                <rect x="2" y="9" width="20" height="6" rx="1" />
                <line x1="4" y1="4" x2="20" y2="20" stroke-width="2.5" />
            </svg>
        `;
    }
    if (btnRetry) btnRetry.style.display = 'inline-flex';
    if (countdown) countdown.style.display = 'block';

    startCountdown();
}

function setConnectingState() {
    if (statusTitle) statusTitle.textContent = 'Connecting to Hospital ERP…';
    if (statusSubtitle) {
        statusSubtitle.innerHTML = `Establishing connection to Nilkanth Medico ERP server at <code>${SERVER.localUrl}</code>`;
    }
    if (statusPill) {
        statusPill.className = 'status-pill connecting';
    }
    if (statusDot) {
        statusDot.className = 'dot blue';
    }
    if (statusText) {
        statusText.textContent = 'Connecting…';
    }
    if (iconContainer) {
        iconContainer.className = 'icon-wrap connecting';
        iconContainer.innerHTML = `
            <svg viewBox="0 0 24 24" fill="none" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <path d="M12 6v6l4 2"></path>
            </svg>
        `;
    }
    if (btnRetry) btnRetry.style.display = 'none';
    if (countdown) countdown.style.display = 'none';
}

function startCountdown() {
    if (retryInterval) clearInterval(retryInterval);
    retrySeconds = 5;
    if (timerVal) timerVal.textContent = String(retrySeconds);

    retryInterval = window.setInterval(() => {
        retrySeconds -= 1;
        if (timerVal) timerVal.textContent = String(retrySeconds);

        if (retrySeconds <= 0) {
            if (retryInterval) clearInterval(retryInterval);
            checkServerConnection();
        }
    }, 1000);
}

async function checkServerConnection() {
    if (isChecking) return;
    isChecking = true;
    setConnectingState();

    try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 4000);

        // Ping the hospital ERP server
        await fetch(SERVER.localUrl, {
            method: 'GET',
            mode: 'no-cors',
            cache: 'no-store',
            signal: controller.signal,
        });

        clearTimeout(timeoutId);
        logDiagnostic('INFO', `[App] ERP server online at ${SERVER.localUrl}. Navigating session.`);

        // Connected successfully: Check if there was a saved screen from before relaunch
        const lastActiveUrl = localStorage.getItem('__nm_last_active_url');
        if (lastActiveUrl && lastActiveUrl.startsWith(SERVER.localUrl)) {
            localStorage.removeItem('__nm_last_active_url');
            logDiagnostic('INFO', `[App] Restoring previous active screen: ${lastActiveUrl}`);
            window.location.href = lastActiveUrl;
        } else {
            window.location.href = SERVER.localUrl;
        }
    } catch (err: unknown) {
        isChecking = false;
        const msg = err instanceof Error ? err.message : 'Timeout or unreachable';
        logDiagnostic('WARN', `[App] ERP connection check failed: ${msg}`);
        setOfflineState(msg);
    }
}

if (btnRetry) {
    btnRetry.addEventListener('click', () => {
        if (retryInterval) clearInterval(retryInterval);
        checkServerConnection();
    });
}

window.addEventListener('online', () => {
    logDiagnostic('INFO', '[App] Network online event detected. Checking server connection.');
    checkServerConnection();
});

window.addEventListener('offline', () => {
    logDiagnostic('WARN', '[App] Network offline event detected.');
    setOfflineState('No network connection');
});

// Initial connection check on launch
checkServerConnection();
