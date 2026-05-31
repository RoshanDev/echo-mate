// EchoMate Frontend — Candidate Popup Logic
import { invoke } from './lib/@tauri-apps/api/core.js';
import { listen } from './lib/@tauri-apps/api/event.js';

// State
let currentCandidates = [];
let currentProvider = 'codex';
let isGenerating = false;

// Card color palette — cycles through 5 tints
const CARD_COLORS = ['card-green', 'card-blue', 'card-purple', 'card-coral', 'card-pink'];
const GENERATION_COMMANDS = new Set([
  'generate_replies',
  'generate_replies_from_screenshot',
  'regenerate_candidates',
  'regenerate_with_style'
]);

// ≡≡≡ Safe invoke wrapper ≡≡≡
async function safeInvoke(cmd, args) {
  try {
    return args ? await invoke(cmd, args) : await invoke(cmd);
  } catch (err) {
    const msg = err?.message || err;
    showError(GENERATION_COMMANDS.has(cmd) ? msg : (cmd + ' 失败: ' + msg));
    throw err;
  }
}

// ≡≡≡ Initialize ≡≡≡
document.addEventListener('DOMContentLoaded', () => {
  document.getElementById('status-text').textContent = '就绪，点击"生成回复"或按热键触发';

  listen('candidates-ready', handleCandidatesReady);
  listen('generation-error', handleError);
  listen('generation-started', handleGenerationStarted);

  setupButtons();
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

  const screenshotBtn = document.getElementById('btn-screenshot-generate');
  if (screenshotBtn) {
    screenshotBtn.addEventListener('click', () => {
      triggerGeneration('generate_replies_from_screenshot', undefined, '框选聊天截图...');
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
  showLoading(source === 'screenshot' ? '正在理解聊天截图...' : undefined);
  const dot = document.getElementById('status-dot');
  if (dot) dot.classList.add('active');
}

function handleCandidatesReady(event) {
  const data = event.payload;
  currentCandidates = data.candidates || [];
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
  showError(msg);
}

// ≡≡≡ Render ≡≡≡
function renderCandidates(candidates) {
  const list = document.getElementById('candidates-list');
  list.innerHTML = '';

  candidates.forEach((c, i) => {
    const card = document.createElement('div');
    card.className = 'candidate-card ' + CARD_COLORS[i % CARD_COLORS.length];

    let tagsHtml = renderTags(c.style_tags || (c.tone ? [c.tone] : []));
    if (c.risk_flags && c.risk_flags.length > 0 && c.risk_flags[0] !== 'none') {
      tagsHtml += '<span class="tag risk">⚠ ' + escapeHtml(c.risk_flags.join(', ')) + '</span>';
    }

    card.innerHTML =
      '<div class="candidate-index">候选 ' + (i + 1) + (c.reason ? ' — ' + escapeHtml(c.reason) : '') + '</div>' +
      '<div class="candidate-text">' + escapeHtml(c.text) + '</div>' +
      '<div class="candidate-meta">' +
        '<div class="candidate-tags">' + tagsHtml + '</div>' +
        '<button class="copy-btn" data-index="' + i + '">复制</button>' +
      '</div>';

    list.appendChild(card);

    card.querySelector('.copy-btn').addEventListener('click', (e) => {
      e.stopPropagation();
      copyCandidate(i, c.text);
    });

    // Click card to copy
    card.addEventListener('click', () => copyCandidate(i, c.text));
  });
}

function renderTags(tags) {
  if (!tags || tags.length === 0) return '';
  const tagList = Array.isArray(tags) ? tags : [tags];
  return tagList.map(t => '<span class="tag">' + escapeHtml(t) + '</span>').join('');
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text || '';
  return div.innerHTML;
}

// ≡≡≡ Copy ≡≡≡
async function copyCandidate(index, text) {
  try {
    await safeInvoke('copy_candidate', { candidateIndex: index, text });
    const buttons = document.querySelectorAll('.copy-btn');
    const btn = buttons[index];
    if (btn) {
      btn.textContent = '已复制!';
      btn.classList.add('copied');
      setTimeout(() => {
        btn.textContent = '复制';
        btn.classList.remove('copied');
      }, 1500);
    }

    // Highlight the selected card
    const cards = document.querySelectorAll('.candidate-card');
    cards.forEach(c => c.classList.remove('selected'));
    if (cards[index]) cards[index].classList.add('selected');

    document.getElementById('status-text').textContent = '已复制候选 ' + (index + 1);
  } catch (err) {
    console.error('Copy failed:', err);
    showError('复制失败: ' + (err?.message || err));
  }
}

// ≡≡≡ Loading/Error UI ≡≡≡
function showLoading(label) {
  setGenerating(true);
  document.getElementById('loading').style.display = 'flex';
  document.getElementById('candidates-list').innerHTML = '';
  document.getElementById('error-msg').style.display = 'none';
  document.getElementById('actions').style.display = 'none';
  document.getElementById('status-text').textContent = label || '正在生成候选回复...';
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
  ['btn-test-generate', 'btn-screenshot-generate', 'btn-regenerate', 'btn-conservative', 'btn-fun'].forEach((id) => {
    const btn = document.getElementById(id);
    if (btn) btn.disabled = active;
  });
}
