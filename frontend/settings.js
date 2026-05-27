// EchoMate Settings Page Logic

const invoke = window.__TAURI__?.core?.invoke;

document.addEventListener('DOMContentLoaded', () => {
  loadSettings();
  setupSettingsButtons();
});

async function loadSettings() {
  try {
    const settings = await invoke('get_settings');
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
    document.getElementById('setting-emoji').value = settings.emoji_level ?? 0.2;
    document.getElementById('setting-humor').value = settings.humor_level ?? 0.3;
  } catch (err) {
    console.error('Failed to load settings:', err);
  }
}

function setupSettingsButtons() {
  document.getElementById('btn-back').addEventListener('click', () => {
    invoke('show_popup');
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
      await invoke('save_settings', { settings });
      invoke('show_popup');
    } catch (err) {
      console.error('Failed to save settings:', err);
    }
  });

  document.getElementById('btn-reset-settings').addEventListener('click', async () => {
    try {
      await invoke('reset_settings');
      await loadSettings();
    } catch (err) {
      console.error('Failed to reset settings:', err);
    }
  });

  document.getElementById('btn-record-hotkey').addEventListener('click', () => {
    const btn = document.getElementById('btn-record-hotkey');
    btn.textContent = '按下热键...';
    btn.style.background = 'var(--warning)';
    invoke('record_hotkey').then(hotkey => {
      if (hotkey) {
        document.getElementById('setting-hotkey').value = hotkey;
      }
      btn.textContent = '录制';
      btn.style.background = '';
    }).catch(() => {
      btn.textContent = '录制';
      btn.style.background = '';
    });
  });
}
