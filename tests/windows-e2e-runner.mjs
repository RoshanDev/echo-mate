import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';

const CDP_URL = process.env.ECHOMATE_CDP_URL || 'http://127.0.0.1:9222/json';
const OUT_DIR = process.env.ECHOMATE_E2E_OUT
  || (process.env.USERPROFILE ? path.join(process.env.USERPROFILE, 'echo-mate') : 'C:\\Users\\pibao\\echo-mate');
const E2E_HOTKEY = process.env.ECHOMATE_E2E_HOTKEY || '';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

function assertTemporaryProfile() {
  const appData = process.env.APPDATA || '';
  const allowedRoot = process.env.ECHOMATE_E2E_PROFILE_DIR || os.tmpdir();
  const resolvedAppData = path.resolve(appData).toLowerCase();
  const resolvedAllowed = path.resolve(allowedRoot).toLowerCase();
  if (!resolvedAppData || !resolvedAppData.startsWith(resolvedAllowed)) {
    throw new Error(
      `Refusing to run Windows e2e against a non-temporary profile. APPDATA=${appData || '(empty)'}; expected it under ${allowedRoot}. Launch EchoMate with temp APPDATA/ECHOMATE_E2E_PROFILE_DIR first.`
    );
  }
}

function getJson(url) {
  return new Promise((resolve, reject) => {
    http
      .get(url, (res) => {
        let data = '';
        res.on('data', (chunk) => {
          data += chunk;
        });
        res.on('end', () => {
          try {
            resolve(JSON.parse(data));
          } catch (error) {
            reject(error);
          }
        });
      })
      .on('error', reject);
  });
}

function ps(command, sta = false, options = {}) {
  return execFileSync('powershell.exe', [
    ...(sta ? ['-STA'] : []),
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-Command',
    command,
  ], { encoding: 'utf8', timeout: options.timeoutMs ?? 30000 }).trim();
}

function setClipboardText(text) {
  const encoded = Buffer.from(text, 'utf16le').toString('base64');
  ps(`
    $text = [Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('${encoded}'))
    Set-Clipboard -Value $text
  `, true);
}

function setClipboardImage() {
  ps(`
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    $bmp = New-Object System.Drawing.Bitmap 480,180
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::White)
    $font = New-Object System.Drawing.Font('Microsoft YaHei', 18)
    $brush = [System.Drawing.Brushes]::Black
    $g.DrawString('她：我明天面试，有点紧张', $font, $brush, 20, 30)
    $g.DrawString('我：那你早点休息', $font, $brush, 20, 85)
    [System.Windows.Forms.Clipboard]::SetImage($bmp)
    $g.Dispose()
    $bmp.Dispose()
  `, true);
}

function configuredHotkey() {
  const hotkey = ps(`
    $path = Join-Path $env:APPDATA 'EchoMate\\config.json'
    if (Test-Path $path) {
      (Get-Content $path -Raw | ConvertFrom-Json).hotkey
    } else {
      'CmdOrCtrl+Shift+Space'
    }
  `);
  return hotkey || 'CmdOrCtrl+Shift+Space';
}

function hotkeyVirtualKey(hotkey) {
  const key = hotkey.split('+').at(-1).trim().toUpperCase();
  if (key === 'SPACE') return 0x20;
  if (/^[A-Z]$/.test(key)) return key.charCodeAt(0);
  throw new Error(`Unsupported E2E hotkey key: ${hotkey}`);
}

function moveWindow() {
  if (process.env.ECHOMATE_E2E_SKIP_MOVE_WINDOW === '1') {
    console.warn('[e2e] Window positioning skipped by ECHOMATE_E2E_SKIP_MOVE_WINDOW=1');
    return;
  }

  try {
    ps(`
    Add-Type @"
    using System;
    using System.Runtime.InteropServices;
    public class E2EWin {
      [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
      [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr hWndInsertAfter, int X, int Y, int cx, int cy, uint uFlags);
      [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    }
"@
    [E2EWin]::SetProcessDPIAware() | Out-Null
    $deadline = (Get-Date).AddSeconds(4)
    do {
      $p = Get-Process -Name echo-mate -ErrorAction Stop | Select-Object -First 1
      if ($p.MainWindowHandle -ne 0) { break }
      Start-Sleep -Milliseconds 150
    } while ((Get-Date) -lt $deadline)
    if ($p.MainWindowHandle -eq 0) {
      throw 'EchoMate main window handle was not ready'
    }
    [E2EWin]::SetWindowPos($p.MainWindowHandle, [IntPtr]::Zero, 100, 50, 866, 1031, 0x0040) | Out-Null
    [E2EWin]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
  `, false, { timeoutMs: 6000 });
  } catch (error) {
    console.warn(`[e2e] Window positioning skipped: ${error.message || String(error)}`);
  }
}

function captureWindow(name) {
  const outPath = path.join(OUT_DIR, name);
  ps(`
    Add-Type -AssemblyName System.Drawing
    Add-Type @"
    using System;
    using System.Runtime.InteropServices;
    public class E2ECap {
      [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
      [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
      [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
      public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    }
"@
    [E2ECap]::SetProcessDPIAware() | Out-Null
    $p = Get-Process -Name echo-mate -ErrorAction Stop | Select-Object -First 1
    [E2ECap]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 250
    [E2ECap+RECT]$rect = New-Object E2ECap+RECT
    [E2ECap]::GetWindowRect($p.MainWindowHandle, [ref]$rect) | Out-Null
    $w = $rect.Right - $rect.Left
    $h = $rect.Bottom - $rect.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
    $bmp.Save('${outPath.replaceAll('\\', '\\\\')}', [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    $bmp.Dispose()
  `);
  return outPath;
}

function runSelectedHotkey(vk) {
  ps(`
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    Add-Type @"
    using System;
    using System.Runtime.InteropServices;
    public class E2EKeys {
      [DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
    }
"@
    $form = New-Object System.Windows.Forms.Form
    $form.Text = 'EchoMate E2E Selection Source'
    $form.Width = 640
    $form.Height = 220
    $form.TopMost = $true
    $text = New-Object System.Windows.Forms.TextBox
    $text.Multiline = $true
    $text.Dock = [System.Windows.Forms.DockStyle]::Fill
    $text.Font = New-Object System.Drawing.Font('Microsoft YaHei', 18)
    $text.Text = '我明天面试，有点紧张'
    $form.Controls.Add($text)
    $timer = New-Object System.Windows.Forms.Timer
    $timer.Interval = 700
    $timer.Add_Tick({
      $timer.Stop()
      $text.Focus()
      $text.SelectAll()
      [E2EKeys]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
      [E2EKeys]::keybd_event(0x10, 0, 0, [UIntPtr]::Zero)
      [E2EKeys]::keybd_event(${vk}, 0, 0, [UIntPtr]::Zero)
      Start-Sleep -Milliseconds 120
      [E2EKeys]::keybd_event(${vk}, 0, 2, [UIntPtr]::Zero)
      [E2EKeys]::keybd_event(0x10, 0, 2, [UIntPtr]::Zero)
      [E2EKeys]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)
    })
    $closeTimer = New-Object System.Windows.Forms.Timer
    $closeTimer.Interval = 5200
    $closeTimer.Add_Tick({
      $closeTimer.Stop()
      $form.Close()
    })
    $form.Add_Shown({
      $text.Focus()
      $text.SelectAll()
      $timer.Start()
      $closeTimer.Start()
    })
    [System.Windows.Forms.Application]::Run($form)
  `, true);
}

function runHotkeyWithoutSelection(vk) {
  ps(`
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    Add-Type @"
    using System;
    using System.Runtime.InteropServices;
    public class E2EBlankKeys {
      [DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
    }
"@
    $form = New-Object System.Windows.Forms.Form
    $form.Text = 'EchoMate E2E Blank Focus'
    $form.Width = 420
    $form.Height = 160
    $form.TopMost = $true
    $text = New-Object System.Windows.Forms.TextBox
    $text.Multiline = $true
    $text.Dock = [System.Windows.Forms.DockStyle]::Fill
    $text.Font = New-Object System.Drawing.Font('Microsoft YaHei', 18)
    $text.Text = 'No selected text'
    $form.Controls.Add($text)
    $timer = New-Object System.Windows.Forms.Timer
    $timer.Interval = 700
    $timer.Add_Tick({
      $timer.Stop()
      $form.Activate()
      $text.Focus()
      $text.SelectionStart = $text.TextLength
      $text.SelectionLength = 0
      [E2EBlankKeys]::keybd_event(0x11, 0, 0, [UIntPtr]::Zero)
      [E2EBlankKeys]::keybd_event(0x10, 0, 0, [UIntPtr]::Zero)
      [E2EBlankKeys]::keybd_event(${vk}, 0, 0, [UIntPtr]::Zero)
      Start-Sleep -Milliseconds 120
      [E2EBlankKeys]::keybd_event(${vk}, 0, 2, [UIntPtr]::Zero)
      [E2EBlankKeys]::keybd_event(0x10, 0, 2, [UIntPtr]::Zero)
      [E2EBlankKeys]::keybd_event(0x11, 0, 2, [UIntPtr]::Zero)
    })
    $closeTimer = New-Object System.Windows.Forms.Timer
    $closeTimer.Interval = 5200
    $closeTimer.Add_Tick({
      $closeTimer.Stop()
      $form.Close()
    })
    $form.Add_Shown({
      $form.Activate()
      $text.Focus()
      $text.SelectionStart = $text.TextLength
      $text.SelectionLength = 0
      $timer.Start()
      $closeTimer.Start()
    })
    [System.Windows.Forms.Application]::Run($form)
  `, true);
}

async function connect() {
  const pages = await getJson(CDP_URL);
  const page = pages.find((item) => item.type === 'page');
  if (!page) throw new Error('No WebView page found on CDP port 9222');

  const ws = new WebSocket(page.webSocketDebuggerUrl);
  const pending = new Map();
  let id = 0;
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      pending.get(message.id)(message);
      pending.delete(message.id);
    }
  };
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = reject;
  });
  const send = (method, params = {}) => new Promise((resolve) => {
    const callId = ++id;
    pending.set(callId, resolve);
    ws.send(JSON.stringify({ id: callId, method, params }));
  });
  await send('Runtime.enable');
  await send('Page.enable');
  return { ws, send };
}

async function evaluate(send, expression) {
  const response = await send('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  const result = response.result?.result;
  if (result?.subtype === 'error') {
    throw new Error(result.description || result.value || 'Runtime.evaluate failed');
  }
  return result?.value;
}

async function waitFor(send, expression, label, timeoutMs = 12000) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const value = await evaluate(send, expression);
    if (value) return value;
    await sleep(250);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

async function pageScreenshot(send, name) {
  const response = await send('Page.captureScreenshot', { format: 'png', fromSurface: true });
  const outPath = path.join(OUT_DIR, name);
  fs.writeFileSync(outPath, Buffer.from(response.result.data, 'base64'));
  return outPath;
}

async function domSummary(send) {
  return evaluate(send, `(() => ({
    status: document.getElementById('status-text')?.textContent || '',
    candidates: document.querySelectorAll('.candidate-card').length,
    actionVisible: getComputedStyle(document.getElementById('action-section')).display !== 'none',
    contextVisible: getComputedStyle(document.getElementById('context-section')).display !== 'none',
    contextText: document.getElementById('context-card')?.textContent || '',
    memoryVisible: getComputedStyle(document.getElementById('memory-section')).display !== 'none',
    reminderVisible: getComputedStyle(document.getElementById('reminder-section')).display !== 'none',
    savedMemory: document.getElementById('memory-cards')?.textContent.includes('已保存') || false,
    reminderPanel: document.getElementById('reminder-panel')?.textContent || '',
    eventCounts: window.__e2eCounts || {},
  }))()`);
}

async function main() {
  assertTemporaryProfile();
  moveWindow();
  const { ws, send } = await connect();
  const hotkey = E2E_HOTKEY || configuredHotkey();
  const hotkeyVk = hotkeyVirtualKey(hotkey);

  await evaluate(send, `(() => {
    window.__e2eCounts = { candidates: 0, reminders: 0, inbound: 0 };
    window.__e2eEvents = [];
    const target = { kind: 'Any' };
    const add = (event, key) => window.__TAURI_INTERNALS__.invoke('plugin:event|listen', {
      event,
      target,
      handler: window.__TAURI_INTERNALS__.transformCallback((payload) => {
        window.__e2eCounts[key] += 1;
        window.__e2eEvents.push({ event, payload });
      })
    });
    return Promise.all([
      add('candidates-ready', 'candidates'),
      add('reminder-due', 'reminders'),
      add('inbound-signal', 'inbound')
    ]).then(() => true);
  })()`);

  await evaluate(send, `window.__TAURI_INTERNALS__.invoke('get_settings')
    .then((settings) => window.__TAURI_INTERNALS__.invoke('save_settings', {
      settings: {
        ...settings,
        hotkey: ${JSON.stringify(E2E_HOTKEY)} || settings.hotkey,
        strict_privacy: false,
        global_privacy_mode: false,
        windows_notification_helper_enabled: true,
        context_retention_days: 30
      }
    }))
    .then(() => window.__TAURI_INTERNALS__.invoke('upsert_contact', {
      contact: { id: null, alias: '测试联系人A', channel: 'wechat', is_allowlisted: true }
    }))
    .then((contact) => window.__TAURI_INTERNALS__.invoke('set_active_contact', { contactId: contact.id }))`);

  const beforeInboundCandidates = await evaluate(send, `window.__e2eCounts.candidates`);
  const inboundResult = await evaluate(send, `window.__TAURI_INTERNALS__.invoke('ingest_platform_signal', {
    signal: {
      contact_alias: '测试联系人A',
      channel: 'wechat',
      source: 'notification',
      text: '我明天面试，有点紧张',
      app_name: 'WeChat'
    }
  })`);
  if (!inboundResult?.allowed) {
    throw new Error(`Expected allowlisted inbound signal, got: ${inboundResult?.reason || 'unknown'}`);
  }
  await waitFor(send, `window.__e2eCounts.inbound >= 1`, 'inbound signal event');
  await sleep(800);
  const afterInboundCandidates = await evaluate(send, `window.__e2eCounts.candidates`);
  if (afterInboundCandidates !== beforeInboundCandidates) {
    throw new Error('Inbound signal auto-generated candidates');
  }

  setClipboardText('我明天面试，有点紧张');
  await evaluate(send, `document.getElementById('btn-test-generate').click(); true`);
  await waitFor(send, `document.querySelectorAll('.candidate-card').length === 5`, 'text candidates');
  await waitFor(send, `window.__e2eCounts.candidates >= 1`, 'text candidates event');
  const textDom = await pageScreenshot(send, 'e2e-text-dom.png');
  const textWindow = captureWindow('e2e-text-window.png');

  await evaluate(send, `document.querySelector('#memory-cards [data-action="save"]').click(); true`);
  await waitFor(send, `document.getElementById('memory-cards').textContent.includes('已保存')`, 'saved memory');
  await evaluate(send, `document.querySelector('#reminder-cards [data-action="create"]').click(); true`);
  await waitFor(send, `window.__e2eCounts.reminders >= 1 && document.getElementById('reminder-panel').textContent.includes('跟进提醒')`, 'reminder due panel');
  const reminderDom = await pageScreenshot(send, 'e2e-reminder-dom.png');
  const reminderWindow = captureWindow('e2e-reminder-window.png');

  await evaluate(send, `document.querySelector('#reminder-panel .copy-btn').click(); true`);
  await waitFor(send, `document.getElementById('status-text').textContent.includes('已复制候选')`, 'follow-up copy');
  const copied = ps('Get-Clipboard -Raw');

  const beforeHotkeyEvents = await evaluate(send, `window.__e2eCounts.candidates`);
  await evaluate(send, `document.getElementById('btn-close').click(); true`);
  await sleep(700);
  runSelectedHotkey(hotkeyVk);
  await waitFor(send, `window.__e2eCounts.candidates > ${beforeHotkeyEvents}`, 'selected-text hotkey event', 16000);
  const hotkeyWindow = captureWindow('e2e-hotkey-window.png');

  setClipboardImage();
  const beforeScreenshotEvents = await evaluate(send, `window.__e2eCounts.candidates`);
  await evaluate(send, `document.getElementById('btn-test-generate').click(); true`);
  await waitFor(send, `window.__e2eCounts.candidates > ${beforeScreenshotEvents} && document.getElementById('mode-label').textContent === 'screenshot'`, 'auto clipboard image generation', 16000);
  const screenshotDom = await pageScreenshot(send, 'e2e-screenshot-dom.png');
  const screenshotWindow = captureWindow('e2e-screenshot-window.png');

  const beforeScreenshotStyleEvents = await evaluate(send, `window.__e2eCounts.candidates`);
  await evaluate(send, `document.getElementById('btn-fun').click(); true`);
  await waitFor(send, `window.__e2eCounts.candidates > ${beforeScreenshotStyleEvents} && document.getElementById('mode-label').textContent === 'screenshot'`, 'screenshot style regeneration', 16000);
  const screenshotStyleWindow = captureWindow('e2e-screenshot-style-window.png');

  setClipboardImage();
  const beforeScreenshotHotkeyEvents = await evaluate(send, `window.__e2eCounts.candidates`);
  await evaluate(send, `document.getElementById('btn-close').click(); true`);
  await sleep(700);
  runHotkeyWithoutSelection(hotkeyVk);
  await waitFor(send, `window.__e2eCounts.candidates > ${beforeScreenshotHotkeyEvents} && document.getElementById('mode-label').textContent === 'screenshot'`, 'clipboard image hotkey fallback', 18000);
  const screenshotHotkeyWindow = captureWindow('e2e-screenshot-hotkey-window.png');

  const beforeTopicEvents = await evaluate(send, `window.__e2eCounts.candidates`);
  await evaluate(send, `document.getElementById('btn-topic-generate').click(); true`);
  await waitFor(send, `window.__e2eCounts.candidates > ${beforeTopicEvents} && document.getElementById('mode-label').textContent === 'topic'`, 'proactive topic generation', 16000);
  const topicWindow = captureWindow('e2e-topic-window.png');

  const summary = await domSummary(send);
  if (!summary.contextVisible || !summary.contextText.includes('白名单')) {
    throw new Error(`Expected merged allowlisted context, got: ${summary.contextText}`);
  }
  ws.close();
  console.log(JSON.stringify({
    ok: true,
    hotkey,
    copied: copied.trim(),
    inboundResult,
    summary,
    screenshots: {
      textDom,
      textWindow,
      reminderDom,
      reminderWindow,
      hotkeyWindow,
      screenshotDom,
      screenshotWindow,
      screenshotStyleWindow,
      screenshotHotkeyWindow,
      topicWindow,
    },
  }, null, 2));
}

main().catch((error) => {
  console.error(error.stack || error.message || String(error));
  process.exit(1);
});
