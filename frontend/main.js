// EchoMate Frontend — Candidate Popup Logic

const invoke = window.__TAURI__?.core?.invoke;

// State
let currentCandidates = [];
let currentProvider = 'codex';

// ≡≡≡ Initialize ≡≡≡
document.addEventListener('DOMContentLoaded', () => {
  setupButtons();
  // Listen for events from Rust backend
  if (window.__TAURI__?.event) {
    window.__TAURI__.event.listen('candidates-ready', handleCandidatesReady);
    window.__TAURI__.event.listen('generation-error', handleError);
    window.__TAURI__.event.listen('generation-started', handleGenerationStarted);
  }
});

// ≡≡≡ Button Setup ≡≡≡
function setupButtons() {
  document.getElementById('btn-close').addEventListener('click', () => {
    invoke('hide_window');
  });

  document.getElementById('btn-settings').addEventListener('click', () => {
    invoke('open_settings');
  });

  document.getElementById('btn-regenerate').addEventListener('click', () => {
    invoke('regenerate_candidates');
  });

  document.getElementById('btn-conservative').addEventListener('click', () => {
    invoke('regenerate_with_style', { style: 'conservative' });
  });

  document.getElementById('btn-fun').addEventListener('click', () => {
    invoke('regenerate_with_style', { style: 'fun' });
  });
}

// ≡≡≡ Event Handlers ≡≡≡
function handleGenerationStarted(_event) {
  showLoading();
}

function handleCandidatesReady(event) {
  const data = event.payload;
  currentCandidates = data.candidates || [];
  currentProvider = data.provider || 'codex';

  document.getElementById('status-text').textContent = '来信已读取（剪贴板）';
  document.getElementById('mode-indicator').style.display = 'flex';
  document.getElementById('mode-label').textContent = `模式：${data.mode || 'standard'}`;
  document.getElementById('provider-label').textContent = `Provider: ${currentProvider}`;

  const badge = document.getElementById('provider-badge');
  badge.style.display = 'inline';
  badge.textContent = currentProvider.toUpperCase();

  renderCandidates(currentCandidates);
  document.getElementById('actions').style.display = 'flex';
  hideLoading();
}

function handleError(event) {
  const msg = event.payload?.message || '未知错误';
  showError(msg);
}

// ≡≡≡ Render ≡≡≡
function renderCandidates(candidates) {
  const list = document.getElementById('candidates-list');
  list.innerHTML = '';

  candidates.forEach((c, i) => {
    const card = document.createElement('div');
    card.className = 'candidate-card';
    card.innerHTML = `
      <div class="candidate-index">候选 ${i + 1}</div>
      <div class="candidate-text">${escapeHtml(c.text)}</div>
      <div class="candidate-meta">
        <div class="candidate-tags">
          ${renderTags(c.style_tags || [c.tone || '']) }
          ${c.risk_flags && c.risk_flags.length > 0 && c.risk_flags[0] !== 'none'
            ? `<span class="tag risk">⚠ ${c.risk_flags.join(', ')}</span>` : ''}
        </div>
        <button class="copy-btn" data-index="${i}">复制</button>
      </div>
    `;
    list.appendChild(card);

    // Copy button handler
    card.querySelector('.copy-btn').addEventListener('click', (e) => {
      e.stopPropagation();
      copyCandidate(i, c.text);
    });
  });
}

function renderTags(tags) {
  if (!tags || tags.length === 0) return '';
  const tagList = Array.isArray(tags) ? tags : [tags];
  return tagList.map(t => `<span class="tag">${escapeHtml(t)}</span>`).join('');
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

// ≡≡≡ Copy ≡≡≡
async function copyCandidate(index, text) {
  try {
    await navigator.clipboard.writeText(text);
    const btn = document.querySelectorAll('.copy-btn')[index];
    if (btn) {
      btn.textContent = '已复制!';
      btn.classList.add('copied');
      setTimeout(() => {
        btn.textContent = '复制';
        btn.classList.remove('copied');
      }, 1500);
    }
    // Record send event
    invoke('record_copy', { candidateIndex: index });
    // Auto-hide popup after copy
    setTimeout(() => invoke('hide_window'), 800);
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
  document.getElementById('error-msg').textContent = `❌ ${msg}`;
  document.getElementById('status-text').textContent = '生成失败';
  // Auto-hide error after 5s
  setTimeout(() => {
    document.getElementById('error-msg').style.display = 'none';
  }, 5000);
}
