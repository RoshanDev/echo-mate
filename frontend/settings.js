// EchoMate Settings Page Logic
import { invoke } from './lib/@tauri-apps/api/core.js';

async function safeInvoke(cmd, args) {
  try {
    return args !== undefined ? await invoke(cmd, args) : await invoke(cmd);
  } catch (err) {
    console.error('Invoke failed:', cmd, err);
    throw err;
  }
}

document.addEventListener('DOMContentLoaded', () => {
  loadSettings();
  setupSettingsButtons();
});

async function loadSettings() {
  try {
    const settings = await safeInvoke('get_settings');
    if (!settings) return;
    document.getElementById('setting-hotkey').value = settings.hotkey || 'CmdOrCtrl+Shift+Space';
    document.getElementById('setting-primary-provider').value = settings.primary_provider || 'codex';
    document.getElementById('setting-fallback-provider').value = settings.fallback_provider || 'claude';
    document.getElementById('setting-candidate-count').value = settings.candidate_count || 5;
    document.getElementById('setting-timeout').value = settings.timeout_seconds || 45;
    document.getElementById('setting-strict-privacy').checked = settings.strict_privacy !== false;
    document.getElementById('setting-sqlcipher').checked = settings.sqlcipher === true;
    document.getElementById('setting-tone').value = settings.tone || 'warm_calm';
    document.getElementById('setting-length').value = settings.length || 'short_to_medium';
    document.getElementById('setting-emoji').value = settings.emoji_level !== undefined ? settings.emoji_level : 0.2;
    document.getElementById('setting-humor').value = settings.humor_level !== undefined ? settings.humor_level : 0.3;
  } catch (err) {
    console.error('Failed to load settings:', err);
  }
}

function setupSettingsButtons() {
  document.getElementById('btn-back').addEventListener('click', () => {
    safeInvoke('show_popup').catch(() => {});
  });

  document.getElementById('btn-save-settings').addEventListener('click', async () => {
    const settings = {
      hotkey: document.getElementById('setting-hotkey').value,
      primary_provider: document.getElementById('setting-primary-provider').value,
      fallback_provider: document.getElementById('setting-fallback-provider').value,
      candidate_count: parseInt(document.getElementById('setting-candidate-count').value) || 5,
      timeout_seconds: parseInt(document.getElementById('setting-timeout').value) || 45,
      strict_privacy: document.getElementById('setting-strict-privacy').checked,
      sqlcipher: document.getElementById('setting-sqlcipher').checked,
      tone: document.getElementById('setting-tone').value,
      length: document.getElementById('setting-length').value,
      emoji_level: parseFloat(document.getElementById('setting-emoji').value) || 0.2,
      humor_level: parseFloat(document.getElementById('setting-humor').value) || 0.3,
    };
    try {
      await safeInvoke('save_settings', { settings });
      safeInvoke('show_popup').catch(() => {});
    } catch (err) {
      console.error('Failed to save settings:', err);
    }
  });

  document.getElementById('btn-reset-settings').addEventListener('click', async () => {
    try {
      await safeInvoke('reset_settings');
      await loadSettings();
    } catch (err) {
      console.error('Failed to reset settings:', err);
    }
  });

  document.getElementById('btn-record-hotkey').addEventListener('click', () => {
    const btn = document.getElementById('btn-record-hotkey');
    const input = document.getElementById('setting-hotkey');

    if (btn.dataset.recording === 'true') {
      // Cancel recording
      stopRecording(btn);
      return;
    }

    // Start recording
    btn.textContent = '按下组合键... (Esc 取消)';
    btn.style.background = 'var(--accent)';
    btn.style.color = '#fff';
    btn.dataset.recording = 'true';

    function onKeyDown(e) {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === 'Escape') {
        stopRecording(btn);
        return;
      }

      // Ignore lone modifier keys
      if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return;

      const parts = [];
      if (e.metaKey || e.ctrlKey) parts.push('CmdOrCtrl');
      if (e.altKey) parts.push('Alt');
      if (e.shiftKey) parts.push('Shift');

      // Normalize key name
      let key = e.key;
      if (key.length === 1) {
        key = key.toUpperCase();
      } else if (key === ' ') {
        key = 'Space';
      } else {
        // Capitalize first letter for named keys
        key = key.charAt(0).toUpperCase() + key.slice(1);
      }

      parts.push(key);
      const hotkey = parts.join('+');
      input.value = hotkey;

      stopRecording(btn);
    }

    function stopRecording(b) {
      b.textContent = '录制';
      b.style.background = '';
      b.style.color = '';
      b.dataset.recording = 'false';
      document.removeEventListener('keydown', onKeyDown, true);
    }

    btn._stopFn = () => stopRecording(btn);
    document.addEventListener('keydown', onKeyDown, true);
  });
}
