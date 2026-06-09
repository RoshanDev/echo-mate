// EchoMate Frontend — Candidate Popup Logic
import { invoke } from './lib/@tauri-apps/api/core.js';
import { listen } from './lib/@tauri-apps/api/event.js';

// State
let currentCandidates = [];
let currentProvider = 'codex';
let isGenerating = false;
let currentActionCard = null;
let currentSituation = null;
let currentSourceSummary = '';
let currentMemoryCandidates = [];
let currentReminderCandidates = [];
let currentContextSummary = null;
let currentContextPolicy = null;
let currentContextRecord = null;
let currentSourceCards = [];
let currentScreenshotAnalysis = null;
let currentContacts = [];

// Card color palette — cycles through 5 tints
const CARD_COLORS = ['card-green', 'card-blue', 'card-purple', 'card-coral', 'card-pink'];
const GENERATION_COMMANDS = new Set([
  'generate_replies',
  'generate_replies_from_screenshot',
  'generate_topics',
  'regenerate_candidates',
  'regenerate_with_style'
]);

// ≡≡≡ Safe invoke wrapper ≡≡≡
async function safeInvoke(cmd, args) {
  try {
    return args ? await invoke(cmd, args) : await invoke(cmd);
  } catch (err) {
    const msg = err?.message || err;
    if (GENERATION_COMMANDS.has(cmd) && isAlreadyGeneratingMessage(msg)) {
      showLoading('正在生成候选回复...');
      return undefined;
    }
    showError(GENERATION_COMMANDS.has(cmd) ? msg : (cmd + ' 失败: ' + msg));
    throw err;
  }
}

// ≡≡≡ Initialize ≡≡≡
document.addEventListener('DOMContentLoaded', () => {
  document.getElementById('status-text').textContent = '就绪，自动识别剪贴板文字或截图';

  listen('candidates-ready', handleCandidatesReady);
  listen('generation-error', handleError);
  listen('generation-started', handleGenerationStarted);
  listen('reminder-due', handleReminderDue);
  listen('inbound-signal', handleInboundSignal);

  setupButtons();
  loadContacts();
  recoverLastGenerationView();
  recoverReminderPanel();
});

// ≡≡≡ Button Setup ≡≡≡
function setupButtons() {
  document.getElementById('btn-close').addEventListener('click', () => safeInvoke('hide_window'));
  document.getElementById('btn-settings').addEventListener('click', () => safeInvoke('open_settings'));
  document.getElementById('btn-regenerate').addEventListener('click', () => triggerGeneration('regenerate_candidates'));
  document.getElementById('btn-conservative').addEventListener('click', () => triggerGeneration('regenerate_with_style', { style: 'conservative' }));
  document.getElementById('btn-fun').addEventListener('click', () => triggerGeneration('regenerate_with_style', { style: 'fun' }));

  const testBtn = document.getElementById('btn-test-generate');
  if (testBtn) {
    testBtn.addEventListener('click', () => triggerGeneration('generate_replies'));
  }

  const topicBtn = document.getElementById('btn-topic-generate');
  if (topicBtn) {
    topicBtn.addEventListener('click', () => {
      const hint = document.getElementById('topic-hint-input')?.value.trim() || '';
      triggerGeneration(
        'generate_topics',
        hint ? { topicHint: hint } : { topicHint: null },
        hint ? '正在参考你的方向找话题...' : '正在找自然话题...'
      );
    });
  }

  const screenshotBtn = document.getElementById('btn-screenshot-generate');
  if (screenshotBtn) {
    screenshotBtn.addEventListener('click', () => {
      triggerGeneration('generate_replies_from_screenshot', undefined, '框选聊天截图...');
    });
  }

  const contactSelect = document.getElementById('contact-select');
  if (contactSelect) {
    contactSelect.addEventListener('change', async () => {
      await safeInvoke('set_active_contact', { contactId: contactSelect.value }).catch(() => {});
      const selected = currentContacts.find((contact) => contact.id === contactSelect.value);
      document.getElementById('status-text').textContent = selected
        ? '当前联系人：' + selected.alias
        : '未选择联系人，不保存上下文';
    });
  }
}

function triggerGeneration(cmd, args, loadingText) {
  if (isGenerating) return;
  showLoading(loadingText);
  safeInvoke(cmd, args).catch(() => {});
}

// ≡≡≡ Event Handlers ≡≡≡
function handleGenerationStarted(event) {
  const source = event.payload?.source;
  const label = source === 'screenshot'
    ? '正在理解聊天截图...'
    : (source === 'topic' ? '正在找自然话题...' : undefined);
  showLoading(label);
  const dot = document.getElementById('status-dot');
  if (dot) dot.classList.add('active');
}

function handleCandidatesReady(event) {
  const data = event.payload;
  currentCandidates = data.candidates || [];
  currentSituation = data.situation || null;
  currentActionCard = data.action_card || null;
  currentSourceSummary = data.source_summary || '';
  currentMemoryCandidates = data.memory_candidates || [];
  currentReminderCandidates = data.reminder_candidates || [];
  currentContextSummary = data.context_summary || null;
  currentContextPolicy = data.context_policy || null;
  currentContextRecord = data.context_record || null;
  currentSourceCards = data.source_cards || [];
  currentScreenshotAnalysis = data.screenshot_analysis || null;
  currentProvider = data.provider || 'codex';

  document.getElementById('status-text').textContent = '已生成 ' + currentCandidates.length + ' 条候选回复';
  const dot = document.getElementById('status-dot');
  if (dot) dot.classList.remove('active');
  setGenerating(false);

  const modeIndicator = document.getElementById('mode-indicator');
  modeIndicator.style.display = 'flex';
  document.getElementById('mode-label').textContent = (data.mode || 'standard');
  document.getElementById('provider-label').textContent = currentProvider.toUpperCase();

  const badge = document.getElementById('provider-badge');
  badge.style.display = 'inline';
  badge.textContent = currentProvider.toUpperCase();

  renderInsights();
  renderCandidates(currentCandidates);
  document.getElementById('actions').style.display = 'flex';
  hideLoading();
}

function handleError(event) {
  const dot = document.getElementById('status-dot');
  if (dot) dot.classList.remove('active');
  setGenerating(false);

  let msg = '未知错误';
  if (typeof event.payload === 'string') {
    msg = event.payload;
  } else if (event.payload?.message) {
    msg = event.payload.message;
  }
  if (isAlreadyGeneratingMessage(msg)) {
    showLoading('正在生成候选回复...');
    return;
  }
  showError(msg);
}

function handleReminderDue(event) {
  const detail = event.payload;
  if (!detail) return;
  renderReminderPanel(detail);
  document.getElementById('status-text').textContent = '有一条跟进提醒';
}

function handleInboundSignal(event) {
  const payload = event.payload || {};
  const contact = payload.contact || {};
  const banner = document.getElementById('inbound-banner');
  banner.style.display = 'block';
  banner.textContent = (contact.alias || '白名单联系人') + ' 有新消息信号，EchoMate 不会自动生成或发送';
}

// ≡≡≡ Render ≡≡≡
function renderInsights() {
  const container = document.getElementById('insights-container');
  const hasAction = currentActionCard && currentActionCard.reason;
  const hasContext = currentContextSummary || currentContextPolicy || currentSourceCards.length > 0;
  const hasScreenshot = currentScreenshotAnalysis && (
    (currentScreenshotAnalysis.turns || []).length > 0 ||
    currentScreenshotAnalysis.last_reply_target ||
    (currentScreenshotAnalysis.warnings || []).length > 0
  );
  const hasMemory = currentMemoryCandidates.length > 0;
  const hasReminder = currentReminderCandidates.length > 0;

  container.style.display = (hasAction || hasContext || hasScreenshot || hasMemory || hasReminder) ? 'block' : 'none';
  renderActionCard(currentActionCard, currentSituation);
  renderContextCard(currentContextSummary, currentContextPolicy, currentContextRecord, currentSourceCards);
  renderScreenshotAnalysis(currentScreenshotAnalysis);
  renderMemoryCards(currentMemoryCandidates);
  renderReminderCards(currentReminderCandidates);
}

function renderActionCard(action, situation) {
  const section = document.getElementById('action-section');
  const card = document.getElementById('action-card');
  if ((!action || !action.reason) && (!situation || !situation.summary)) {
    section.style.display = 'none';
    card.innerHTML = '';
    return;
  }
  section.style.display = 'block';
  card.innerHTML =
    '<div class="action-type">' + escapeHtml(actionLabel(action?.action_type || situation?.action_type)) + '</div>' +
    (situation?.summary ? '<div class="insight-text">' + escapeHtml(situation.summary) + '</div>' : '') +
    (action?.reason ? '<div class="insight-text">' + escapeHtml(action.reason) + '</div>' : '') +
    (situation?.staleness ? '<div class="source-line">时效：' + escapeHtml(stalenessLabel(situation.staleness)) + '</div>' : '') +
    (situation?.relationship_signal ? '<div class="source-line">弱信号：' + escapeHtml(situation.relationship_signal) + '</div>' : '') +
    '<div class="confidence">置信度 ' + confidenceText(action?.confidence ?? situation?.confidence) + '</div>';
}

function renderContextCard(summary, policy, record, sourceCards) {
  const section = document.getElementById('context-section');
  const card = document.getElementById('context-card');
  const hasSummary = summary && (summary.summary || summary.source_ref || summary.source_kind);
  const hasPolicy = policy && policy.reason;
  const hasSources = Array.isArray(sourceCards) && sourceCards.length > 0;
  if (!hasSummary && !hasPolicy && !hasSources) {
    section.style.display = 'none';
    card.innerHTML = '';
    return;
  }
  section.style.display = 'block';
  const source = summary?.source_excerpt || summary?.source_ref || summary?.source_kind || '当前触发';
  const allowText = policy?.can_save_context
    ? '白名单联系人：可保存用户确认的本地上下文'
    : '联系人不在白名单或隐私模式开启：不保存上下文';
  card.innerHTML =
    '<div class="action-type">' + escapeHtml(allowText) + '</div>' +
    (summary?.summary ? '<div class="insight-text">' + escapeHtml(summary.summary) + '</div>' : '') +
    (currentSourceSummary ? '<div class="insight-text">' + escapeHtml(currentSourceSummary) + '</div>' : '') +
    '<div class="source-line">来源：' + escapeHtml(source) + '</div>' +
    renderSourceCards(sourceCards || []) +
    (policy?.reason ? '<div class="confidence">' + escapeHtml(policy.reason) + '</div>' : '') +
    (record?.id
      ? '<div class="mini-actions"><button class="tiny-btn danger" data-action="delete-context">删除这条上下文</button></div>'
      : '');
  const deleteBtn = card.querySelector('[data-action="delete-context"]');
  if (deleteBtn) {
    deleteBtn.addEventListener('click', async (e) => {
      e.stopPropagation();
      await safeInvoke('delete_context_summary', { id: record.id });
      currentContextRecord = null;
      card.innerHTML =
        '<div class="action-type">已删除这条上下文</div>' +
        '<div class="confidence">后续生成不会再读取这条摘要。</div>';
    });
  }
}

function renderScreenshotAnalysis(analysis) {
  const section = document.getElementById('screenshot-analysis-section');
  const card = document.getElementById('screenshot-analysis-card');
  const turns = analysis?.turns || [];
  const warnings = analysis?.warnings || [];
  if (!analysis || (!turns.length && !analysis.last_reply_target && !warnings.length)) {
    section.style.display = 'none';
    card.innerHTML = '';
    return;
  }
  section.style.display = 'block';
  const turnRows = turns.slice(-5).map((turn) =>
    '<div class="source-card-row">' +
      '<div class="source-card-title">' + escapeHtml(speakerLabel(turn.speaker)) + ' · ' + escapeHtml(mediaKindLabel(turn.media_kind)) + '</div>' +
      (turn.text ? '<div class="source-card-detail">' + escapeHtml(turn.text) + '</div>' : '') +
      '<div class="source-card-meta">' +
        (turn.visible_time_label ? '可见时间：' + escapeHtml(turn.visible_time_label) + ' · ' : '') +
        '可信度 ' + confidenceText(turn.confidence) +
      '</div>' +
    '</div>'
  ).join('');
  card.innerHTML =
    (analysis.last_reply_target ? '<div class="action-type">最后可回复：' + escapeHtml(analysis.last_reply_target) + '</div>' : '') +
    '<div class="source-line">时间：' + escapeHtml(analysis.inferred_chat_time || 'unknown') + ' · ' + escapeHtml(stalenessLabel(analysis.staleness)) + '</div>' +
    (turnRows ? '<div class="source-card-list">' + turnRows + '</div>' : '') +
    (warnings.length ? '<div class="confidence">提示：' + escapeHtml(warnings.slice(0, 3).join('；')) + '</div>' : '');
}

function renderSourceCards(cards) {
  if (!Array.isArray(cards) || cards.length === 0) return '';
  return '<div class="source-card-list">' + cards.map((card) => {
    const timeBits = [
      card.captured_at ? '捕获/保存：' + formatShortTime(card.captured_at) : '',
      card.visible_message_time ? '可见时间：' + card.visible_message_time : '',
      card.inferred_chat_time ? '聊天时间：' + card.inferred_chat_time : '',
    ].filter(Boolean).join(' · ');
    const confidence = Number.isFinite(Number(card.source_confidence))
      ? '可信度 ' + confidenceText(card.source_confidence)
      : '';
    return '<div class="source-card-row">' +
      '<div class="source-card-title">' + escapeHtml(card.title || sourceKindLabel(card.source_kind)) + '</div>' +
      (card.detail ? '<div class="source-card-detail">' + escapeHtml(card.detail) + '</div>' : '') +
      '<div class="source-card-meta">' +
        escapeHtml(sourceFactLabel(card.fact_source)) +
        (timeBits ? ' · ' + escapeHtml(timeBits) : '') +
        (confidence ? ' · ' + escapeHtml(confidence) : '') +
      '</div>' +
    '</div>';
  }).join('') + '</div>';
}

function renderMemoryCards(items) {
  const section = document.getElementById('memory-section');
  const list = document.getElementById('memory-cards');
  list.innerHTML = '';
  if (!items || items.length === 0) {
    section.style.display = 'none';
    return;
  }
  section.style.display = 'block';

  items.forEach((item, index) => {
    const card = document.createElement('div');
    card.className = 'mini-card';
    card.innerHTML =
      '<div class="mini-card-top">' +
        '<span class="mini-label">' + escapeHtml(memoryTypeLabel(item.memory_type)) + '</span>' +
        '<span class="mini-confidence">' + confidenceText(item.confidence) + '</span>' +
      '</div>' +
      '<div class="mini-value">' + escapeHtml(item.value) + '</div>' +
      sourceHtml(item) +
      '<div class="mini-actions">' +
        '<button class="tiny-btn primary" data-action="save">保存</button>' +
        '<button class="tiny-btn" data-action="ignore">忽略</button>' +
      '</div>';
    list.appendChild(card);

    card.querySelector('[data-action="save"]').addEventListener('click', async (e) => {
      e.stopPropagation();
      try {
        const saved = await safeInvoke('save_memory_candidate', { candidate: item });
        card.classList.add('saved');
        card.innerHTML =
          '<div class="mini-card-top">' +
            '<span class="mini-label">已保存</span>' +
            '<span class="mini-confidence">' + confidenceText(saved.confidence) + '</span>' +
          '</div>' +
          '<div class="mini-value">' + escapeHtml(saved.value) + '</div>' +
          sourceHtml(saved) +
          '<div class="mini-actions">' +
            '<button class="tiny-btn danger" data-action="delete">删除</button>' +
          '</div>';
        card.querySelector('[data-action="delete"]').addEventListener('click', async (event) => {
          event.stopPropagation();
          await safeInvoke('delete_memory', { id: saved.id });
          card.remove();
          refreshInsightContainer();
        });
        document.getElementById('status-text').textContent = '已保存一条记忆';
      } catch (err) {
        console.error('Save memory failed:', err);
      }
    });

    card.querySelector('[data-action="ignore"]').addEventListener('click', async (e) => {
      e.stopPropagation();
      await safeInvoke('ignore_memory_candidate', { candidateIndex: index }).catch(() => {});
      card.remove();
      refreshInsightContainer();
    });
  });
}

function renderReminderCards(items) {
  const section = document.getElementById('reminder-section');
  const list = document.getElementById('reminder-cards');
  list.innerHTML = '';
  if (!items || items.length === 0) {
    section.style.display = 'none';
    return;
  }
  section.style.display = 'block';

  items.forEach((item, index) => {
    const card = document.createElement('div');
    card.className = 'mini-card reminder-card';
    const inputId = 'reminder-time-' + index;
    card.innerHTML =
      '<div class="mini-card-top">' +
        '<span class="mini-label">提醒</span>' +
        '<span class="mini-confidence">' + confidenceText(item.confidence) + '</span>' +
      '</div>' +
      '<div class="mini-value">' + escapeHtml(item.memory_value) + '</div>' +
      '<div class="mini-reason">' + escapeHtml(item.reason || item.recommended_time) + '</div>' +
      sourceHtml(item) +
      '<input id="' + inputId + '" class="datetime-input" type="datetime-local" value="' + escapeHtml(toDatetimeLocal(item.trigger_at)) + '">' +
      '<div class="mini-actions">' +
        '<button class="tiny-btn primary" data-action="create">提醒我</button>' +
        '<button class="tiny-btn" data-action="ignore">忽略</button>' +
      '</div>';
    list.appendChild(card);

    card.querySelector('[data-action="create"]').addEventListener('click', async (e) => {
      e.stopPropagation();
      const triggerAt = datetimeLocalToIso(card.querySelector('#' + inputId).value);
      try {
        const detail = await safeInvoke('create_reminder_from_candidate', { candidate: item, triggerAt });
        card.classList.add('saved');
        card.querySelector('.mini-actions').innerHTML =
          '<button class="tiny-btn danger" data-action="delete">删除提醒</button>';
        card.querySelector('[data-action="delete"]').addEventListener('click', async (event) => {
          event.stopPropagation();
          await safeInvoke('delete_reminder', { id: detail.reminder.id });
          card.remove();
          refreshInsightContainer();
        });
        document.getElementById('status-text').textContent = '已创建一条提醒';
      } catch (err) {
        console.error('Create reminder failed:', err);
      }
    });

    card.querySelector('[data-action="ignore"]').addEventListener('click', async (e) => {
      e.stopPropagation();
      await safeInvoke('ignore_reminder_candidate', { candidateIndex: index }).catch(() => {});
      card.remove();
      refreshInsightContainer();
    });
  });
}

function renderCandidates(candidates) {
  const list = document.getElementById('candidates-list');
  list.innerHTML = '';

  candidates.forEach((c, i) => {
    const card = document.createElement('div');
    card.className = 'candidate-card ' + CARD_COLORS[i % CARD_COLORS.length];

    let tags = c.style_tags || (c.tone ? [c.tone] : []);
    if (c.intent_group) tags = [c.intent_group].concat(tags);
    let tagsHtml = renderTags(tags);
    if (c.risk_flags && c.risk_flags.length > 0 && c.risk_flags[0] !== 'none') {
      tagsHtml += '<span class="tag risk">⚠ ' + escapeHtml(c.risk_flags.join(', ')) + '</span>';
    }

    card.innerHTML =
      '<div class="candidate-index">候选 ' + (i + 1) + (c.reason ? ' — ' + escapeHtml(c.reason) : '') + '</div>' +
      '<div class="candidate-text">' + escapeHtml(c.text) + '</div>' +
      (c.source_refs && c.source_refs.length ? '<div class="source-line">引用：' + escapeHtml(c.source_refs.join(' / ')) + '</div>' : '') +
      '<div class="candidate-meta">' +
        '<div class="candidate-tags">' + tagsHtml + '</div>' +
        '<button class="copy-btn" data-index="' + i + '">复制</button>' +
      '</div>';

    list.appendChild(card);

    const copyBtn = card.querySelector('.copy-btn');
    copyBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      copyCandidate(i, c.text, copyBtn, card);
    });

    // Click card to copy
    card.addEventListener('click', () => copyCandidate(i, c.text, copyBtn, card));
  });
}

function renderReminderPanel(detail) {
  const panel = document.getElementById('reminder-panel');
  if (!detail || !detail.reminder || !detail.memory_item) {
    panel.style.display = 'none';
    panel.innerHTML = '';
    return;
  }
  panel.style.display = 'block';
  const candidates = detail.follow_up_candidates || [];
  panel.innerHTML =
    '<div class="section-title">跟进提醒</div>' +
    '<div class="reminder-detail-card">' +
      '<div class="mini-label">' + escapeHtml(memoryTypeLabel(detail.memory_item.memory_type)) + '</div>' +
      '<div class="mini-value">' + escapeHtml(detail.memory_item.value) + '</div>' +
      sourceHtml(detail.memory_item) +
      '<div class="insight-text">' + escapeHtml(detail.action_card?.reason || '') + '</div>' +
      '<div class="mini-actions">' +
        '<button class="tiny-btn danger" data-action="delete">删除提醒</button>' +
      '</div>' +
    '</div>' +
    '<div class="follow-up-list"></div>';

  const list = panel.querySelector('.follow-up-list');
  candidates.forEach((candidate, index) => {
    const item = document.createElement('div');
    item.className = 'follow-up-item';
    item.innerHTML =
      '<div class="candidate-text">' + escapeHtml(candidate.text) + '</div>' +
      '<button class="copy-btn" data-index="' + index + '">复制</button>';
    list.appendChild(item);
    item.querySelector('.copy-btn').addEventListener('click', (e) => {
      e.stopPropagation();
      copyCandidate(index, candidate.text, e.currentTarget);
    });
  });

  panel.querySelector('[data-action="delete"]').addEventListener('click', async (e) => {
    e.stopPropagation();
    await safeInvoke('delete_reminder', { id: detail.reminder.id });
    panel.style.display = 'none';
    panel.innerHTML = '';
  });
}

function renderTags(tags) {
  if (!tags || tags.length === 0) return '';
  const tagList = Array.isArray(tags) ? tags : [tags];
  return tagList.map(t => '<span class="tag">' + escapeHtml(t) + '</span>').join('');
}

function sourceHtml(item) {
  const source = item.source_excerpt || item.source_ref || '';
  if (!source) return '';
  const sourcePrefix = item.fact_source === 'manual' || item.source_kind === 'contact_fact'
    ? '用户手动补充'
    : '来源';
  return '<div class="source-line">' + escapeHtml(sourcePrefix) + '：' + escapeHtml(source) + '</div>';
}

function actionLabel(type) {
  const labels = {
    continue_chat: '继续聊',
    wrap_up: '自然收束',
    light_follow_up: '轻跟进',
    do_not_push: '先别推进',
    safe_repair: '先修复',
    soft_invite_candidate: '可轻试探邀约'
  };
  return labels[type] || '继续聊';
}

function stalenessLabel(value) {
  const labels = {
    fresh: '较新',
    stale: '可能过期',
    unknown: '时间不明',
    visible_time_only: '仅有截图可见时间',
    inferred: '推断时间'
  };
  return labels[value] || value || '时间不明';
}

function speakerLabel(value) {
  const labels = {
    me: '我',
    other: '对方',
    system: '时间/系统',
    unknown: '未确定'
  };
  return labels[value] || value || '未确定';
}

function mediaKindLabel(value) {
  const labels = {
    text: '文本',
    image: '图片',
    emoji: '表情',
    quote: '引用',
    system: '系统',
    unknown: '未知'
  };
  return labels[value] || value || '未知';
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

function sourceKindLabel(kind) {
  const labels = {
    clipboard: '剪贴板',
    text: '文本',
    screenshot: '截图',
    topic: '找话题',
    memory: '已批准记忆',
    contact_fact: '用户手动补充',
    provider_run: 'Provider 调用',
    reminder: '提醒上下文',
  };
  return labels[kind] || '来源';
}

function sourceFactLabel(source) {
  const labels = {
    manual: '用户手动补充',
    clipboard: '剪贴板',
    screenshot: '截图/图片',
    notification: '通知信号',
    memory: '已批准记忆',
    provider: 'Provider',
    topic: '主动找话题',
  };
  return labels[source] || source || '本地来源';
}

function confidenceText(value) {
  const n = Number(value);
  if (!Number.isFinite(n)) return '低';
  if (n >= 0.75) return '较高';
  if (n >= 0.45) return '中';
  return '低';
}

function toDatetimeLocal(value) {
  let date = value ? new Date(value) : null;
  if (!date || Number.isNaN(date.getTime())) {
    date = new Date(Date.now() + 60 * 60 * 1000);
  }
  const offsetMs = date.getTimezoneOffset() * 60 * 1000;
  return new Date(date.getTime() - offsetMs).toISOString().slice(0, 16);
}

function datetimeLocalToIso(value) {
  const date = value ? new Date(value) : new Date(Date.now() + 60 * 60 * 1000);
  return date.toISOString();
}

function formatShortTime(value) {
  if (!value) return '';
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

function refreshInsightContainer() {
  const container = document.getElementById('insights-container');
  const hasVisibleCards = container.querySelector('.mini-card') ||
    (currentActionCard && currentActionCard.reason) ||
    (currentContextSummary || currentContextPolicy);
  container.style.display = hasVisibleCards ? 'block' : 'none';
}

async function loadContacts() {
  try {
    const [contacts, settings] = await Promise.all([
      safeInvoke('list_contacts'),
      safeInvoke('get_settings'),
    ]);
    currentContacts = contacts || [];
    const select = document.getElementById('contact-select');
    if (!select) return;
    select.innerHTML = '<option value="">未选择联系人</option>';
    currentContacts.forEach((contact) => {
      const option = document.createElement('option');
      option.value = contact.id;
      option.textContent = contact.alias + (contact.is_allowlisted ? '' : '（停用）');
      option.disabled = !contact.is_allowlisted;
      select.appendChild(option);
    });
    select.value = settings?.active_contact_id || '';
  } catch (err) {
    console.error('Load contacts failed:', err);
  }
}

async function recoverReminderPanel() {
  if (window.location.hash !== '#reminders') return;
  try {
    const detail = await safeInvoke('get_latest_notified_reminder');
    if (detail) renderReminderPanel(detail);
  } catch (err) {
    console.error('Recover reminder panel failed:', err);
  }
}

async function recoverLastGenerationView() {
  try {
    const snapshot = await safeInvoke('get_last_generation_view');
    if (!snapshot || !Array.isArray(snapshot.candidates) || snapshot.candidates.length === 0) {
      return;
    }
    handleCandidatesReady({ payload: snapshot });
  } catch (err) {
    console.error('Recover last generation view failed:', err);
  }
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}

// ≡≡≡ Copy ≡≡≡
async function copyCandidate(index, text, button, selectedCard) {
  try {
    await safeInvoke('copy_candidate', { candidateIndex: index, text });
    const btn = button || document.querySelectorAll('.copy-btn')[index];
    if (btn) {
      btn.textContent = '已复制!';
      btn.classList.add('copied');
      setTimeout(() => {
        btn.textContent = '复制';
        btn.classList.remove('copied');
      }, 1500);
    }

    if (selectedCard) {
      const cards = document.querySelectorAll('.candidate-card');
      cards.forEach(c => c.classList.remove('selected'));
      selectedCard.classList.add('selected');
    }

    document.getElementById('status-text').textContent = '已复制候选 ' + (index + 1);
  } catch (err) {
    console.error('Copy failed:', err);
    showError('复制失败: ' + (err?.message || err));
  }
}

// ≡≡≡ Loading/Error UI ≡≡≡
function showLoading(label) {
  setGenerating(true);
  const dot = document.getElementById('status-dot');
  if (dot) dot.classList.add('active');
  document.getElementById('loading').style.display = 'flex';
  document.getElementById('candidates-list').innerHTML = '';
  document.getElementById('insights-container').style.display = 'none';
  document.getElementById('reminder-panel').style.display = 'none';
  document.getElementById('error-msg').style.display = 'none';
  document.getElementById('actions').style.display = 'none';
  document.getElementById('status-text').textContent = label || '正在生成候选回复...';
}

function isAlreadyGeneratingMessage(msg) {
  return String(msg || '').includes('已有一次生成正在进行');
}

function hideLoading() {
  document.getElementById('loading').style.display = 'none';
}

function showError(msg) {
  setGenerating(false);
  document.getElementById('loading').style.display = 'none';
  document.getElementById('error-msg').style.display = 'block';
  document.getElementById('error-msg').textContent = msg;
  document.getElementById('status-text').textContent = '生成失败';
  setTimeout(() => {
    document.getElementById('error-msg').style.display = 'none';
  }, 8000);
}

function setGenerating(active) {
  isGenerating = active;
  ['btn-test-generate', 'btn-topic-generate', 'btn-screenshot-generate', 'btn-regenerate', 'btn-conservative', 'btn-fun'].forEach((id) => {
    const btn = document.getElementById(id);
    if (btn) btn.disabled = active;
  });
  const topicHint = document.getElementById('topic-hint-input');
  if (topicHint) topicHint.disabled = active;
}
