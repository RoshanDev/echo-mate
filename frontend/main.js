// EchoMate Frontend — Candidate Popup Logic
import { invoke } from './lib/@tauri-apps/api/core.js';
import { listen } from './lib/@tauri-apps/api/event.js';

// State
let currentCandidates = [];
let currentProvider = 'codex';

// ≡≡≡ Safe invoke wrapper ≡≡≡
async function safeInvoke(cmd, args) {
  try {
    return args ? await invoke(cmd, args) : await invoke(cmd);
  } catch (err) {
    showError(cmd + ' 失败: ' + (err?.message || err));
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
  document.getElementById('btn-regenerate').addEventListener('click', () => safeInvoke('regenerate_candidates'));
  document.getElementById('btn-conservative').addEventListener('click', () => safeInvoke('regenerate_with_style', { style: 'conservative' }));
  document.getElementById('btn-fun').addEventListener('click', () => safeInvoke('regenerate_with_style', { style: 'fun' }));

  const testBtn = document.getElementById('btn-test-generate');
  if (testBtn) {
    testBtn.addEventListener('click', () => {
      showLoading();
      safeInvoke('generate_replies').catch(() => {});
    });
  }
}

// ≡≡≡ Event Handlers ≡≡≡
function handleGenerationStarted() {
  showLoading();
}

function handleCandidatesReady(event) {
  const data = event.payload;
  currentCandidates = data.candidates || [];
  currentProvider = data.provider || 'codex';

  document.getElementById('status-text').textContent = '来信已读取（剪贴板）';
  const modeIndicator = document.getElementById('mode-indicator');
  modeIndicator.style.display = 'flex';
  document.getElementById('mode-label').textContent = '模式：' + (data.mode || 'standard');
  document.getElementById('provider-label').textContent = 'Provider: ' + currentProvider;

  const badge = document.getElementById('provider-badge');
  badge.style.display = 'inline';
  badge.textContent = currentProvider.toUpperCase();

  renderCandidates(currentCandidates);
  document.getElementById('actions').style.display = 'flex';
  hideLoading();
}

function handleError(event) {
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
    card.className = 'candidate-card';

    let tagsHtml = renderTags(c.style_tags || (c.tone ? [c.tone] : []));
    if (c.risk_flags && c.risk_flags.length > 0 && c.risk_flags[0] !== 'none') {
      tagsHtml += '<span class="tag risk">⚠ ' + escapeHtml(c.risk_flags.join(', ')) + '</span>';
    }

    card.innerHTML =
      '<div class="candidate-index">候选 ' + (i + 1) + '</div>' +
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
    await navigator.clipboard.writeText(text);
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
    safeInvoke('record_copy', { candidateIndex: index }).catch(() => {});
    setTimeout(() => safeInvoke('hide_window').catch(() => {}), 800);
  } catch (err) {
    console.error('Copy failed:', err);
  }
}

// ≡≡≡ Loading/Error UI ≡≡≡
function showLoading() {
  document.getElementById('loading').style.display = 'flex';
  document.getElementById('candidates-list').innerHTML = '';
  document.getElementById('error-msg').style.display = 'none';
  document.getElementById('actions').style.display = 'none';
  document.getElementById('status-text').textContent = '正在生成候选回复...';
}

function hideLoading() {
  document.getElementById('loading').style.display = 'none';
}

function showError(msg) {
  document.getElementById('loading').style.display = 'none';
  document.getElementById('error-msg').style.display = 'block';
  document.getElementById('error-msg').textContent = '❌ ' + msg;
  document.getElementById('status-text').textContent = '生成失败';
  setTimeout(() => {
    document.getElementById('error-msg').style.display = 'none';
  }, 8000);
}
