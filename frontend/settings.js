// EchoMate Settings Page Logic
import { invoke } from './lib/@tauri-apps/api/core.js';

let currentContacts = [];
let pendingManualFacts = [];

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
  loadPrivacyGuideStatus();
  loadStyleProfile();
  loadDataAuditReport();
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
    document.getElementById('setting-debug-log-body').checked = settings.debug_log_body_enabled === true;
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
      debug_log_body_enabled: document.getElementById('setting-debug-log-body').checked,
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

  const workspaceContactSelect = document.getElementById('workspace-contact-select');
  if (workspaceContactSelect) {
    workspaceContactSelect.addEventListener('change', () => refreshWorkspacePanels());
  }
  document.getElementById('btn-load-relationship')?.addEventListener('click', loadRelationshipCard);
  document.getElementById('btn-refresh-memory-inbox')?.addEventListener('click', loadMemoryInbox);
  document.getElementById('btn-refresh-reminders')?.addEventListener('click', loadReminderCenter);
  document.getElementById('btn-refresh-audit')?.addEventListener('click', loadDataAuditReport);
  document.getElementById('btn-export-data')?.addEventListener('click', exportDataSnapshot);
  document.getElementById('btn-clear-logs')?.addEventListener('click', clearLogs);
  document.getElementById('btn-clear-all-data')?.addEventListener('click', clearAllData);

  const factContactSelect = document.getElementById('fact-contact-select');
  if (factContactSelect) {
    factContactSelect.addEventListener('change', () => {
      pendingManualFacts = [];
      renderManualFactPreview(null);
      loadContactFacts(factContactSelect.value);
    });
  }

  const classifyBtn = document.getElementById('btn-classify-facts');
  if (classifyBtn) {
    classifyBtn.addEventListener('click', classifyManualFacts);
  }

  const saveFactsBtn = document.getElementById('btn-save-facts');
  if (saveFactsBtn) {
    saveFactsBtn.addEventListener('click', saveManualFacts);
  }

  document.getElementById('btn-refresh-style-profile').addEventListener('click', async () => {
    const btn = document.getElementById('btn-refresh-style-profile');
    const status = document.getElementById('style-profile-status');
    btn.disabled = true;
    btn.textContent = '刷新中...';
    status.textContent = '';
    try {
      const profile = await safeInvoke('refresh_style_profile');
      renderStyleProfile(profile);
      if (!profile) {
        status.textContent = '没有可用的已采用回复。';
      }
    } catch (err) {
      status.textContent = '刷新失败：' + String(err);
      console.error('Failed to refresh style profile:', err);
    } finally {
      btn.disabled = false;
      btn.textContent = '刷新画像';
    }
  });

  document.getElementById('btn-reset-style-profile').addEventListener('click', async () => {
    if (!confirm('清空本地风格画像？')) return;
    const status = document.getElementById('style-profile-status');
    try {
      await safeInvoke('reset_style_profile');
      renderStyleProfile(null);
      status.textContent = '已重置。';
    } catch (err) {
      status.textContent = '重置失败：' + String(err);
      console.error('Failed to reset style profile:', err);
    }
  });
}

async function loadContacts() {
  try {
    const contacts = await safeInvoke('list_contacts');
    currentContacts = contacts || [];
    renderContacts(currentContacts);
    renderManualFactContactSelect(currentContacts);
    renderWorkspaceContactSelect(currentContacts);
    await refreshWorkspacePanels();
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
        '<button class="small-btn" data-action="clear">清空上下文和记忆</button>' +
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
      row.querySelector('.contact-meta').textContent = (contact.channel || 'wechat') + ' · 上下文和记忆已清空';
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

function renderManualFactContactSelect(contacts) {
  const select = document.getElementById('fact-contact-select');
  if (!select) return;
  const previous = select.value;
  select.innerHTML = '<option value="">选择联系人</option>';
  contacts.forEach((contact) => {
    const option = document.createElement('option');
    option.value = contact.id;
    option.textContent = contact.alias + (contact.is_allowlisted ? '' : '（停用）');
    option.disabled = !contact.is_allowlisted;
    select.appendChild(option);
  });
  if (previous && contacts.some((contact) => contact.id === previous && contact.is_allowlisted)) {
    select.value = previous;
  }
  loadContactFacts(select.value).catch((err) => console.error('Load facts failed:', err));
}

function renderWorkspaceContactSelect(contacts) {
  const select = document.getElementById('workspace-contact-select');
  if (!select) return;
  const previous = select.value;
  select.innerHTML = '<option value="">选择联系人</option>';
  contacts.forEach((contact) => {
    const option = document.createElement('option');
    option.value = contact.id;
    option.textContent = contact.alias + (contact.is_allowlisted ? '' : '（停用）');
    select.appendChild(option);
  });
  if (previous && contacts.some((contact) => contact.id === previous)) {
    select.value = previous;
  } else {
    const firstEnabled = contacts.find((contact) => contact.is_allowlisted);
    if (firstEnabled) select.value = firstEnabled.id;
  }
}

async function refreshWorkspacePanels() {
  await Promise.all([
    loadRelationshipCard().catch(() => {}),
    loadMemoryInbox().catch(() => {}),
    loadReminderCenter().catch(() => {}),
  ]);
}

function selectedWorkspaceContactId() {
  return document.getElementById('workspace-contact-select')?.value || '';
}

async function loadPrivacyGuideStatus() {
  const box = document.getElementById('privacy-guide-box');
  if (!box) return;
  try {
    const status = await safeInvoke('get_privacy_guide_status');
    box.innerHTML =
      '<div><strong>本地存储：</strong>' + escapeHtml(status.data_path || '') + '</div>' +
      '<div><strong>日志目录：</strong>' + escapeHtml(status.log_path || '') + '</div>' +
      '<div><strong>截图/Provider：</strong>截图生成会发送给当前配置的视觉 provider；本地 OCR 优先尝试 Apple Vision。</div>' +
      '<div><strong>前端 Shell：</strong>' + (status.shell_execute_exposed_to_frontend ? '已暴露' : '未暴露') + '</div>' +
      '<div><strong>正文日志：</strong>' + (status.debug_log_body_enabled ? '已开启' : '默认关闭') + '</div>' +
      '<div class="permission-note">EchoMate 不自动发送消息，不默认扫描全量聊天历史；记忆和提醒需要用户确认。</div>' +
      (status.onboarding_completed ? '' : '<button id="btn-ack-privacy" class="small-btn">我知道了</button>');
    const btn = document.getElementById('btn-ack-privacy');
    if (btn) {
      btn.addEventListener('click', async () => {
        await safeInvoke('acknowledge_privacy_guide');
        await loadPrivacyGuideStatus();
      });
    }
  } catch (err) {
    box.textContent = '读取隐私状态失败：' + String(err);
  }
}

async function loadRelationshipCard() {
  const box = document.getElementById('relationship-card-box');
  if (!box) return;
  const contactId = selectedWorkspaceContactId();
  if (!contactId) {
    box.innerHTML = '<div class="empty-row">选择联系人后查看关系卡。</div>';
    return;
  }
  const card = await safeInvoke('get_relationship_card', { contactId });
  box.innerHTML =
    '<div class="fact-row">' +
      '<div class="fact-main">' +
        '<div class="fact-title">' + escapeHtml(card.contact?.alias || '联系人') + '</div>' +
        '<div class="fact-meta">' + escapeHtml(card.interaction_cadence || '') + '</div>' +
        '<div class="fact-note">最近停在：' + escapeHtml(card.last_stop || '') + '</div>' +
      '</div>' +
    '</div>' +
    renderCompactList('手动资料', card.contact_facts, (fact) => factTypeLabel(fact.fact_type) + ' · ' + fact.value) +
    renderCompactList('已批准记忆', card.memories, (item) => memoryTypeLabel(item.memory_type) + ' · ' + item.value) +
    renderCompactList('待处理候选', card.pending_memory_candidates, (item) => memoryTypeLabel(item.memory_type) + ' · ' + item.value) +
    renderCompactList('提醒', card.reminders, (item) => (item.reminder?.status || '') + ' · ' + (item.memory_item?.value || ''));
}

async function loadMemoryInbox() {
  const list = document.getElementById('memory-inbox-list');
  if (!list) return;
  const contactId = selectedWorkspaceContactId();
  if (!contactId) {
    list.innerHTML = '<div class="empty-row">选择联系人后查看候选记忆。</div>';
    return;
  }
  const items = await safeInvoke('list_memory_candidate_inbox', { contactId });
  list.innerHTML = '';
  if (!items || items.length === 0) {
    list.innerHTML = '<div class="empty-row">暂无待处理候选记忆。</div>';
    return;
  }
  items.forEach((item) => {
    const row = document.createElement('div');
    row.className = 'fact-row';
    row.innerHTML =
      '<div class="fact-main">' +
        '<div class="fact-title">' + escapeHtml(memoryTypeLabel(item.memory_type)) + ' · ' + escapeHtml(item.value) + '</div>' +
        '<div class="fact-meta">' + escapeHtml(item.sensitivity) + ' · 置信度 ' + confidenceText(item.confidence) + ' · ' + formatLocalTime(item.created_at) + '</div>' +
        (item.reason ? '<div class="fact-note">' + escapeHtml(item.reason) + '</div>' : '') +
      '</div>' +
      '<button class="small-btn" data-action="confirm">记住</button>' +
      '<button class="small-btn danger" data-action="ignore">忽略</button>';
    list.appendChild(row);
    row.querySelector('[data-action="confirm"]').addEventListener('click', async () => {
      await safeInvoke('confirm_memory_candidate_record', { id: item.id });
      row.remove();
      await loadRelationshipCard().catch(() => {});
    });
    row.querySelector('[data-action="ignore"]').addEventListener('click', async () => {
      await safeInvoke('ignore_memory_candidate_record', { id: item.id });
      row.remove();
    });
  });
}

async function loadReminderCenter() {
  const list = document.getElementById('reminder-center-list');
  if (!list) return;
  const contactId = selectedWorkspaceContactId();
  const items = await safeInvoke('list_reminders', { contactId: contactId || null });
  list.innerHTML = '';
  if (!items || items.length === 0) {
    list.innerHTML = '<div class="empty-row">暂无提醒。</div>';
    return;
  }
  items.forEach((item) => {
    const row = document.createElement('div');
    row.className = 'fact-row';
    row.innerHTML =
      '<div class="fact-main">' +
        '<div class="fact-title">' + escapeHtml(item.memory_item?.value || '提醒') + '</div>' +
        '<div class="fact-meta">' + escapeHtml(item.reminder?.status || '') + ' · ' + formatLocalTime(item.reminder?.trigger_at) + ' · 延后 ' + (item.reminder?.snooze_count || 0) + ' 次</div>' +
        (item.reminder?.reason ? '<div class="fact-note">' + escapeHtml(item.reminder.reason) + '</div>' : '') +
      '</div>' +
      '<button class="small-btn" data-action="complete">完成</button>' +
      '<button class="small-btn" data-action="snooze">延后 1 天</button>' +
      '<button class="small-btn" data-action="mute-contact">静默联系人</button>' +
      '<button class="small-btn" data-action="mute-kind">静默类型</button>' +
      '<button class="small-btn danger" data-action="delete">删除</button>';
    list.appendChild(row);
    row.querySelector('[data-action="complete"]').addEventListener('click', async () => {
      await safeInvoke('complete_reminder', { id: item.reminder.id });
      row.remove();
    });
    row.querySelector('[data-action="snooze"]').addEventListener('click', async () => {
      await safeInvoke('snooze_reminder', { id: item.reminder.id, minutes: 1440 });
      await loadReminderCenter();
    });
    row.querySelector('[data-action="mute-contact"]').addEventListener('click', async () => {
      await safeInvoke('mute_reminders', { contactId: item.reminder.contact_id || contactId || null, kind: null, hours: 168 });
      await loadReminderCenter();
    });
    row.querySelector('[data-action="mute-kind"]').addEventListener('click', async () => {
      await safeInvoke('mute_reminders', { contactId: null, kind: item.reminder.kind || 'follow_up', hours: 168 });
      await loadReminderCenter();
    });
    row.querySelector('[data-action="delete"]').addEventListener('click', async () => {
      await safeInvoke('delete_reminder', { id: item.reminder.id });
      row.remove();
    });
  });
}

async function classifyManualFacts() {
  const contactId = document.getElementById('fact-contact-select')?.value || '';
  const note = document.getElementById('manual-fact-note')?.value.trim() || '';
  const status = document.getElementById('manual-fact-status');
  const saveBtn = document.getElementById('btn-save-facts');
  if (!contactId) {
    status.textContent = '请先选择已启用的联系人。';
    return;
  }
  if (!note) {
    status.textContent = '请先输入要补充的资料。';
    return;
  }
  status.textContent = '归类中...';
  saveBtn.disabled = true;
  pendingManualFacts = [];
  try {
    const result = await safeInvoke('classify_contact_facts', { contactId, note });
    pendingManualFacts = (result?.facts || []).filter((fact) => fact.value);
    renderManualFactPreview(result);
    saveBtn.disabled = pendingManualFacts.length === 0;
    status.textContent = pendingManualFacts.length
      ? '已归类 ' + pendingManualFacts.length + ' 条，确认后保存。'
      : '没有识别到可保存的资料。';
  } catch (err) {
    status.textContent = '归类失败：' + String(err);
    renderManualFactPreview(null);
  }
}

async function saveManualFacts() {
  const contactId = document.getElementById('fact-contact-select')?.value || '';
  const status = document.getElementById('manual-fact-status');
  if (!contactId || pendingManualFacts.length === 0) return;
  try {
    const saved = await safeInvoke('save_contact_facts', { contactId, facts: pendingManualFacts });
    pendingManualFacts = [];
    document.getElementById('btn-save-facts').disabled = true;
    document.getElementById('manual-fact-note').value = '';
    renderManualFactPreview(null);
    await loadContactFacts(contactId);
    status.textContent = '已保存 ' + (saved?.length || 0) + ' 条用户手动补充资料。';
  } catch (err) {
    status.textContent = '保存失败：' + String(err);
  }
}

function renderManualFactPreview(result) {
  const box = document.getElementById('manual-fact-preview');
  if (!box) return;
  box.innerHTML = '';
  const facts = result?.facts || [];
  if (!facts.length) return;
  facts.forEach((fact) => {
    const row = document.createElement('div');
    row.className = 'fact-row preview';
    row.innerHTML =
      '<div class="fact-main">' +
        '<div class="fact-title">' + escapeHtml(factTypeLabel(fact.fact_type)) + ' · ' + escapeHtml(fact.value) + '</div>' +
        '<div class="fact-meta">用户手动补充 · ' + escapeHtml(fact.sensitivity || 'normal') + ' · ' + escapeHtml(fact.usage_policy || 'contextual') + ' · 置信度 ' + confidenceText(fact.confidence) + '</div>' +
        (fact.source_note ? '<div class="fact-note">' + escapeHtml(fact.source_note) + '</div>' : '') +
      '</div>';
    box.appendChild(row);
  });
  if (result?.usage_guidance) {
    const guidance = document.createElement('div');
    guidance.className = 'empty-row';
    guidance.textContent = result.usage_guidance;
    box.appendChild(guidance);
  }
}

async function loadContactFacts(contactId) {
  const list = document.getElementById('manual-fact-list');
  if (!list) return;
  list.innerHTML = '';
  if (!contactId) {
    list.innerHTML = '<div class="empty-row">选择联系人后查看已保存的用户手动补充资料。</div>';
    return;
  }
  const facts = await safeInvoke('list_contact_facts', { contactId });
  renderContactFacts(facts || []);
}

function renderContactFacts(facts) {
  const list = document.getElementById('manual-fact-list');
  list.innerHTML = '';
  if (!facts.length) {
    list.innerHTML = '<div class="empty-row">还没有已保存的手动补充资料。</div>';
    return;
  }
  facts.forEach((fact) => {
    const row = document.createElement('div');
    row.className = 'fact-row';
    row.innerHTML =
      '<div class="fact-main">' +
        '<div class="fact-title">' + escapeHtml(factTypeLabel(fact.fact_type)) + ' · ' + escapeHtml(fact.value) + '</div>' +
        '<div class="fact-meta">用户手动补充 · ' + escapeHtml(fact.sensitivity) + ' · ' + escapeHtml(fact.usage_policy) + ' · 更新 ' + formatLocalTime(fact.updated_at) + '</div>' +
        (fact.source_note ? '<div class="fact-note">' + escapeHtml(fact.source_note) + '</div>' : '') +
      '</div>' +
      '<button class="small-btn danger" data-action="delete-fact">删除</button>';
    list.appendChild(row);
    row.querySelector('[data-action="delete-fact"]').addEventListener('click', async () => {
      await safeInvoke('delete_contact_fact', { id: fact.id });
      row.remove();
      if (!list.querySelector('.fact-row')) renderContactFacts([]);
    });
  });
}

async function loadDataAuditReport() {
  const box = document.getElementById('data-audit-box');
  if (!box) return;
  try {
    const report = await safeInvoke('get_data_audit_report');
    const counts = report.counts || [];
    const findings = report.contamination_findings || [];
    box.innerHTML =
      '<div><strong>生成时间：</strong>' + escapeHtml(formatLocalTime(report.generated_at)) + '</div>' +
      '<div><strong>保留策略：</strong>' + escapeHtml(String(report.retention_days || 0)) + ' 天</div>' +
      '<div><strong>污染扫描：</strong>' + (findings.length ? ('发现 ' + findings.length + ' 项') : '未发现 e2e/mock/test 标记') + '</div>' +
      '<div class="audit-counts">' + counts.map((item) =>
        '<span class="audit-pill">' + escapeHtml(item.table_name) + ' ' + escapeHtml(String(item.count)) + '</span>'
      ).join('') + '</div>' +
      (findings.length ? '<div class="permission-note">' + escapeHtml(findings.slice(0, 4).map((item) => item.table_name + '.' + item.field_name + '=' + item.matched_text).join('；')) + '</div>' : '');
  } catch (err) {
    box.textContent = '读取审计失败：' + String(err);
  }
}

async function exportDataSnapshot() {
  const box = document.getElementById('data-audit-box');
  try {
    const snapshot = await safeInvoke('export_data_snapshot');
    const size = JSON.stringify(snapshot).length;
    box.innerHTML = '<div><strong>导出快照已生成：</strong>' + size + ' 字符。</div><div class="permission-note">当前版本通过受控后端返回 JSON，不开放前端 shell。</div>';
  } catch (err) {
    box.textContent = '导出失败：' + String(err);
  }
}

async function clearAllData() {
  if (!confirm('清空 EchoMate 全部本地数据？这会删除联系人、上下文、记忆、提醒、画像和审计相关记录。')) return;
  await safeInvoke('clear_all_data');
  currentContacts = [];
  await loadContacts();
  await loadDataAuditReport();
}

async function clearLogs() {
  await safeInvoke('clear_logs');
  const box = document.getElementById('data-audit-box');
  if (box) box.innerHTML = '<div><strong>日志已清理。</strong></div>';
  await loadPrivacyGuideStatus();
}

function renderCompactList(title, items, labelFn) {
  const values = Array.isArray(items) ? items.slice(0, 6) : [];
  if (!values.length) {
    return '<div class="empty-row">' + escapeHtml(title) + '：暂无</div>';
  }
  return '<div class="compact-block"><div class="fact-meta">' + escapeHtml(title) + '</div>' +
    values.map((item) => '<div class="compact-row">' + escapeHtml(labelFn(item)) + '</div>').join('') +
    '</div>';
}

function memoryTypeLabel(type) {
  const labels = {
    event: '事件',
    preference: '偏好',
    boundary: '边界',
    stress_point: '压力点',
    relationship_milestone: '关系节点'
  };
  return labels[type] || '记忆';
}

function factTypeLabel(type) {
  const labels = {
    birth_year: '出生年份',
    age_band: '年龄段',
    hometown: '籍贯',
    current_city: '现居城市',
    work_city: '工作城市',
    occupation: '职业',
    preference: '偏好',
    boundary: '边界',
    important_date: '重要日期',
    temporary_state: '临时状态',
    note: '资料',
  };
  return labels[type] || '资料';
}

function confidenceText(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) return '低';
  if (n >= 0.75) return '较高';
  if (n >= 0.45) return '中';
  return '低';
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

async function loadStyleProfile() {
  try {
    const profile = await safeInvoke('get_style_profile');
    renderStyleProfile(profile);
  } catch (err) {
    document.getElementById('style-profile-status').textContent = '读取画像失败：' + String(err);
    console.error('Failed to load style profile:', err);
  }
}

function renderStyleProfile(profile) {
  const summaryEl = document.getElementById('style-profile-summary');
  const samplesEl = document.getElementById('style-profile-samples');
  const avgEl = document.getElementById('style-profile-avg');
  const updatedEl = document.getElementById('style-profile-updated');
  const tagsEl = document.getElementById('style-profile-tags');
  const rulesEl = document.getElementById('style-profile-rules');
  const statusEl = document.getElementById('style-profile-status');
  tagsEl.innerHTML = '';
  rulesEl.innerHTML = '';
  statusEl.textContent = '';

  if (!profile) {
    summaryEl.textContent = '暂无本地画像';
    samplesEl.textContent = '0';
    avgEl.textContent = '-';
    updatedEl.textContent = '未生成';
    return;
  }

  const payload = parseStyleProfileJson(profile.profile_json);
  summaryEl.textContent = payload.summary || '本地画像已生成';
  samplesEl.textContent = String(profile.sample_count || 0);
  avgEl.textContent = typeof payload.avg_chars === 'number'
    ? Math.round(payload.avg_chars) + ' 字'
    : '-';
  updatedEl.textContent = formatLocalTime(profile.updated_at);

  const labels = Array.isArray(payload.tone_labels) ? payload.tone_labels : [];
  labels.forEach((label) => {
    const tag = document.createElement('span');
    tag.className = 'style-tag';
    tag.textContent = label;
    tagsEl.appendChild(tag);
  });

  const rules = []
    .concat(Array.isArray(payload.generation_rules) ? payload.generation_rules.slice(0, 4) : [])
    .concat(Array.isArray(payload.avoid_rules) ? payload.avoid_rules.slice(0, 2) : []);
  rules.forEach((rule) => {
    const row = document.createElement('div');
    row.className = 'style-rule';
    row.textContent = rule;
    rulesEl.appendChild(row);
  });
}

function parseStyleProfileJson(raw) {
  if (!raw) return {};
  try {
    return JSON.parse(raw);
  } catch (err) {
    console.warn('Failed to parse style profile JSON:', err);
    return { summary: raw };
  }
}

function formatLocalTime(value) {
  if (!value) return '未生成';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}
