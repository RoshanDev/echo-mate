import { execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

const PROJECT_ROOT = path.resolve(new URL('..', import.meta.url).pathname);
const ORIGINAL_HOME = process.env.HOME || os.homedir();
const USE_RUNNING_APP = process.env.ECHOMATE_E2E_USE_RUNNING === '1';
const OUT_DIR = process.env.ECHOMATE_E2E_OUT
  || path.join(os.tmpdir(), 'echomate-macos-smoke');
const CLIPBOARD_TEXT = process.env.ECHOMATE_E2E_CLIPBOARD_TEXT || '我明天面试，有点紧张';
const APP_PROCESS = 'echo-mate';
const E2E_ACCOUNT = {
  id: process.env.ECHOMATE_E2E_ACCOUNT_ID || 'echomate-e2e-account',
  alias: process.env.ECHOMATE_E2E_CONTACT_ALIAS || 'EchoMate E2E 测试账号',
  channel: process.env.ECHOMATE_E2E_CONTACT_CHANNEL || 'wechat',
};

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function runAppleScript(script, timeoutMs = 15000) {
  return execFileSync('osascript', [], {
    input: script,
    encoding: 'utf8',
    timeout: timeoutMs,
  }).trim();
}

function appleString(value) {
  return JSON.stringify(String(value));
}

function getClipboardText() {
  try {
    return runAppleScript('try\n  the clipboard as text\non error\n  return ""\nend try', 5000);
  } catch {
    return '';
  }
}

function setClipboardText(text) {
  runAppleScript(`set the clipboard to ${appleString(text)}`, 5000);
}

function echoMateRunning() {
  try {
    execFileSync('pgrep', ['-x', APP_PROCESS], { stdio: 'ignore', timeout: 5000 });
    return true;
  } catch {
    return false;
  }
}

function windowTitle() {
  return runAppleScript(`
tell application "System Events"
  if not (exists process "${APP_PROCESS}") then return ""
  tell process "${APP_PROCESS}"
    if not (exists window 1) then return ""
    return name of window 1
  end tell
end tell
`, 20000);
}

function clickGenerateButton() {
  runAppleScript(`
tell application "System Events"
  tell process "${APP_PROCESS}"
    set frontmost to true
    delay 0.2
    click button "⚡ 生成回复" of UI element 1 of scroll area 1 of group 1 of group 1 of window 1
  end tell
end tell
`, 10000);
}

function accessibilityText() {
  return runAppleScript(`
on collectText(e, depth, maxDepth)
  tell application "System Events"
    set textOut to ""
    try
      set n to name of e
      if n is not missing value then set textOut to textOut & (n as text) & linefeed
    end try
    try
      set v to value of e
      if v is not missing value then set textOut to textOut & (v as text) & linefeed
    end try
    if depth >= maxDepth then return textOut
    try
      repeat with child in UI elements of e
        set textOut to textOut & my collectText(child, depth + 1, maxDepth)
      end repeat
    end try
    return textOut
  end tell
end collectText

tell application "System Events"
  if not (exists process "${APP_PROCESS}") then return ""
  tell process "${APP_PROCESS}"
    if not (exists window 1) then return ""
    return my collectText(window 1, 0, 6)
  end tell
end tell
`, 30000);
}

async function waitFor(label, predicate, timeoutMs = 60000, intervalMs = 500) {
  const started = Date.now();
  let lastError;
  while (Date.now() - started < timeoutMs) {
    try {
      const value = await predicate();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await sleep(intervalMs);
  }
  const suffix = lastError ? ` Last error: ${lastError.message || String(lastError)}` : '';
  throw new Error(`Timed out waiting for ${label}.${suffix}`);
}

function screenshot(name) {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  const outPath = path.join(OUT_DIR, name);
  execFileSync('screencapture', ['-x', outPath], { timeout: 15000 });
  return outPath;
}

function sqliteValue(dbPath, sql) {
  return execFileSync('sqlite3', [dbPath, sql], {
    encoding: 'utf8',
    timeout: 5000,
  }).trim();
}

function e2eDbEvidence(homeDir) {
  const dbPath = path.join(homeDir, '.echomate-e2e', 'echomate.db');
  if (!fs.existsSync(dbPath)) return null;
  const accountCount = Number(sqliteValue(
    dbPath,
    `SELECT COUNT(*) FROM contacts WHERE id='${E2E_ACCOUNT.id}' AND alias='${E2E_ACCOUNT.alias}' AND channel='${E2E_ACCOUNT.channel}' AND is_allowlisted=1;`
  ));
  const suggestionRuns = Number(sqliteValue(
    dbPath,
    `SELECT COUNT(*) FROM suggestion_runs WHERE contact_id='${E2E_ACCOUNT.id}';`
  ));
  const sourceContexts = Number(sqliteValue(
    dbPath,
    `SELECT COUNT(*) FROM source_contexts WHERE contact_id='${E2E_ACCOUNT.id}' AND source_excerpt LIKE '%明天面试%';`
  ));
  const memoryCandidates = Number(sqliteValue(
    dbPath,
    `SELECT COUNT(*) FROM memory_candidates WHERE contact_id='${E2E_ACCOUNT.id}' AND source_ref='e2e-mock';`
  ));
  return { dbPath, accountCount, suggestionRuns, sourceContexts, memoryCandidates };
}

function spawnTauriDev(homeDir) {
  const env = {
    ...process.env,
    HOME: homeDir,
    RUSTUP_HOME: process.env.RUSTUP_HOME || path.join(ORIGINAL_HOME, '.rustup'),
    CARGO_HOME: process.env.CARGO_HOME || path.join(ORIGINAL_HOME, '.cargo'),
    ECHOMATE_E2E_MOCK_PROVIDER: '1',
    ECHOMATE_E2E_PROFILE_DIR: path.join(homeDir, '.echomate-e2e'),
    ECHOMATE_E2E_ACCOUNT_ID: E2E_ACCOUNT.id,
    ECHOMATE_E2E_CONTACT_ALIAS: E2E_ACCOUNT.alias,
    ECHOMATE_E2E_CONTACT_CHANNEL: E2E_ACCOUNT.channel,
    RUST_LOG: process.env.RUST_LOG || 'info',
  };
  const child = spawn('npx', ['tauri', 'dev'], {
    cwd: PROJECT_ROOT,
    env,
    detached: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  child.stdout.setEncoding('utf8');
  child.stderr.setEncoding('utf8');
  let output = '';
  child.stdout.on('data', (chunk) => {
    output += chunk;
  });
  child.stderr.on('data', (chunk) => {
    output += chunk;
  });
  child.on('exit', (code, signal) => {
    output += `\n[tauri-dev-exit code=${code} signal=${signal}]\n`;
  });
  return { child, output: () => output };
}

function stopSpawned(processInfo) {
  if (!processInfo) return;
  try {
    process.kill(-processInfo.child.pid, 'SIGTERM');
  } catch {}
}

async function main() {
  if (process.platform !== 'darwin') {
    throw new Error('macOS smoke runner must be run on macOS.');
  }

  if (USE_RUNNING_APP) {
    throw new Error('Refusing to run e2e against an already running EchoMate app. e2e must launch a dev app with a temporary HOME.');
  }

  if (!USE_RUNNING_APP && echoMateRunning()) {
    throw new Error('EchoMate is already running. Stop it first; e2e must use a temporary HOME and must not target the real profile.');
  }

  fs.mkdirSync(OUT_DIR, { recursive: true });
  const originalClipboard = getClipboardText();
  const homeDir = fs.mkdtempSync(path.join(os.tmpdir(), 'echomate-macos-smoke-home-'));
  let tauriDev = null;

  try {
    if (!USE_RUNNING_APP) {
      tauriDev = spawnTauriDev(homeDir);
    }

    await waitFor('EchoMate process', () => echoMateRunning(), 90000);
    await waitFor('EchoMate window', () => windowTitle() === 'EchoMate', 90000);

    setClipboardText(CLIPBOARD_TEXT);
    screenshot('macos-smoke-before.png');
    clickGenerateButton();

    const evidence = await waitFor('generated candidates in temp e2e database', () => {
      const current = e2eDbEvidence(homeDir);
      if (!current) return false;
      return current.accountCount === 1
        && current.suggestionRuns >= 1
        && current.sourceContexts >= 1
        && current.memoryCandidates >= 1
        && current;
    }, 60000, 1000);

    const afterScreenshot = screenshot('macos-smoke-after.png');
    const logPath = path.join(homeDir, '.echomate-e2e', 'logs', `echomate.log.${new Date().toISOString().slice(0, 10)}`);
    const logTail = fs.existsSync(logPath)
      ? fs.readFileSync(logPath, 'utf8').split('\n').slice(-20).join('\n')
      : '';

    console.log(JSON.stringify({
      ok: true,
      mode: USE_RUNNING_APP ? 'running-app' : 'spawned-tauri-dev',
      window: windowTitle(),
      screenshot: afterScreenshot,
      dbEvidence: evidence,
      logPath: fs.existsSync(logPath) ? logPath : null,
      logTail,
    }, null, 2));
  } catch (error) {
    if (tauriDev) {
      console.error(tauriDev.output());
    }
    throw error;
  } finally {
    setClipboardText(originalClipboard);
    stopSpawned(tauriDev);
  }
}

main().catch((error) => {
  console.error(error.stack || error.message || String(error));
  process.exit(1);
});
