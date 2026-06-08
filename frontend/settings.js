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
  loadContacts();
  loadPermissionStatus();
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
    document.getElementById('setting-global-privacy').checked = settings.global_privacy_mode === true;
    document.getElementById('setting-retention-days').value = settings.context_retention_days || 30;
    document.getElementById('setting-sqlcipher').checked = settings.sqlcipher === true;
    document.getElementById('setting-windows-helper').checked = settings.windows_notification_helper_enabled === true;
    document.getElementById('setting-macos-helper').checked = settings.macos_context_helper_enabled === true;
    document.getElementById('setting-macos-accessibility').checked = settings.macos_accessibility_enabled === true;
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
      global_privacy_mode: document.getElementById('setting-global-privacy').checked,
      context_retention_days: parseInt(document.getElementById('setting-retention-days').value) || 30,
      sqlcipher: document.getElementById('setting-sqlcipher').checked,
      windows_notification_helper_enabled: document.getElementById('setting-windows-helper').checked,
      macos_context_helper_enabled: document.getElementById('setting-macos-helper').checked,
      macos_accessibility_enabled: document.getElementById('setting-macos-accessibility').checked,
      tone: document.getElementById('setting-tone').value,
      length: document.getElementById('setting-length').value,
      emoji_level: parseFloat(document.getElementById('setting-emoji').value) || 0.2,
      humor_level: parseFloat(document.getElementById('setting-humor').value) || 0.3,
    };
    try {
      await safeInvoke('save_settings', { settings });
      await loadPermissionStatus();
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

  document.getElementById('btn-save-contact').addEventListener('click', saveContact);
}

async function loadContacts() {
  try {
    const contacts = await safeInvoke('list_contacts');
    renderContacts(contacts || []);
  } catch (err) {
    console.error('Failed to load contacts:', err);
  }
}

function renderContacts(contacts) {
  const list = document.getElementById('contacts-list');
  list.innerHTML = '';
  if (!contacts.length) {
    list.innerHTML = '<div class="empty-row">还没有联系人。未选择白名单联系人时，EchoMate 只生成候选，不保存上下文。</div>';
    return;
  }
  contacts.forEach((contact) => {
    const row = document.createElement('div');
    row.className = 'contact-row';
    row.innerHTML =
      '<div class="contact-main">' +
        '<div class="contact-name">' + escapeHtml(contact.alias) + '</div>' +
        '<div class="contact-meta">' + escapeHtml(contact.channel) + ' · ' + (contact.is_allowlisted ? '已启用' : '已停用') + '</div>' +
      '</div>' +
      '<div class="contact-actions">' +
        '<button class="small-btn" data-action="edit">编辑</button>' +
        '<button class="small-btn" data-action="clear">清空上下文</button>' +
        '<button class="small-btn danger" data-action="delete">删除</button>' +
      '</div>';
    list.appendChild(row);

    row.querySelector('[data-action="edit"]').addEventListener('click', () => {
      document.getElementById('contact-id').value = contact.id;
      document.getElementById('contact-alias').value = contact.alias;
      document.getElementById('contact-channel').value = contact.channel || 'wechat';
      document.getElementById('contact-allowlisted').checked = contact.is_allowlisted;
    });
    row.querySelector('[data-action="clear"]').addEventListener('click', async () => {
      await safeInvoke('clear_contact_context', { id: contact.id });
      row.querySelector('.contact-meta').textContent = (contact.channel || 'wechat') + ' · 上下文已清空';
    });
    row.querySelector('[data-action="delete"]').addEventListener('click', async () => {
      await safeInvoke('delete_contact', { id: contact.id });
      await loadContacts();
    });
  });
}

async function saveContact() {
  const alias = document.getElementById('contact-alias').value.trim();
  if (!alias) return;
  await safeInvoke('upsert_contact', {
    contact: {
      id: document.getElementById('contact-id').value || null,
      alias,
      channel: document.getElementById('contact-channel').value || 'wechat',
      is_allowlisted: document.getElementById('contact-allowlisted').checked,
    },
  });
  document.getElementById('contact-id').value = '';
  document.getElementById('contact-alias').value = '';
  document.getElementById('contact-allowlisted').checked = true;
  await loadContacts();
}

async function loadPermissionStatus() {
  try {
    const status = await safeInvoke('get_permission_status');
    const macosSnapshot = await safeInvoke('get_macos_context_snapshot').catch(() => null);
    const box = document.getElementById('permission-status');
    if (!status || !box) return;
    const macosLines = macosSnapshot ? (
      '<div><strong>macOS helper：</strong>' + escapeHtml(macosSnapshot.status) + '</div>' +
      '<div><strong>前台应用：</strong>' + escapeHtml(macosSnapshot.front_app || '未读取') + '</div>' +
      '<div><strong>窗口标题：</strong>' + escapeHtml(macosSnapshot.window_title || '未读取') + '</div>' +
      '<div><strong>选中文本：</strong>' + (macosSnapshot.selected_text_available ? '可读取' : '未读取或未授权') + '</div>' +
      '<div><strong>Pasteboard：</strong>' + (macosSnapshot.pasteboard_available ? '可读取' : '空或未读取') + '</div>'
    ) : '';
    box.innerHTML =
      '<div><strong>平台：</strong>' + escapeHtml(status.platform) + '</div>' +
      '<div><strong>Windows：</strong>' + escapeHtml(status.windows_notification_status) + '</div>' +
      '<div><strong>macOS：</strong>' + escapeHtml(status.macos_context_status) + '</div>' +
      macosLines +
      '<div><strong>降级路径：</strong>' + escapeHtml(status.fallback_path) + '</div>' +
      '<div class="permission-note">EchoMate 只生成候选并复制，不会自动发送、主动起聊、抓全量历史或监控群聊。</div>';
  } catch (err) {
    console.error('Failed to load permission status:', err);
  }
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}
