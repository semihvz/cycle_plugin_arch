/**
 * Cycle-ORC Telemetry Console (Port 8080)
 * High-Speed Zero-Latency WebSocket Controller & Real-Time Orchestrator Dashboard
 */

(function () {
  'use strict';

  const state = {
    activeTab: 'telemetry',
    ws: null,
    wsConnected: false,
    selectedSystemId: null,
    systems: [],
    lastSystemsJson: '',
    logs: [],
    autoscrollLogs: true,
    pauseHexStream: false
  };

  const el = {
    serverStatusBadge: document.getElementById('server-status-badge'),
    latencyPill: document.getElementById('latency-pill'),
    wsLatencyVal: document.getElementById('ws-latency-val'),
    tabBtnTelemetry: document.getElementById('tab-btn-telemetry'),
    tabBtnHex: document.getElementById('tab-btn-hex'),
    tabBtnLogs: document.getElementById('tab-btn-logs'),
    pageTelemetry: document.getElementById('page-telemetry'),
    pageHex: document.getElementById('page-hex'),
    pageLogs: document.getElementById('page-logs'),
    valTotalSystems: document.getElementById('val-total-systems'),
    valRunningSystems: document.getElementById('val-running-systems'),
    valCpuUsage: document.getElementById('val-cpu-usage'),
    valRamUsage: document.getElementById('val-ram-usage'),
    systemsCardList: document.getElementById('systems-card-list'),
    btnRefreshSystems: document.getElementById('btn-refresh-systems'),
    focusSystemTitle: document.getElementById('focus-system-title'),
    focusSystemAddr: document.getElementById('focus-system-addr'),
    focusBufferLen: document.getElementById('focus-buffer-len'),
    focusPreviewHex: document.getElementById('focus-preview-hex'),
    miniLogBox: document.getElementById('mini-log-box'),
    btnGotoLogs: document.getElementById('btn-goto-logs'),
    hexSystemName: document.getElementById('hex-system-name'),
    btnPauseHex: document.getElementById('btn-pause-hex'),
    btnClearHex: document.getElementById('btn-clear-hex'),
    hexMatrixDisplay: document.getElementById('hex-matrix-display'),
    asciiStringDisplay: document.getElementById('ascii-string-display'),
    logFilterInput: document.getElementById('log-filter-input'),
    btnToggleAutoscroll: document.getElementById('btn-toggle-autoscroll'),
    btnClearLogs: document.getElementById('btn-clear-logs'),
    mainConsoleOutput: document.getElementById('main-console-output'),
    toastContainer: document.getElementById('toast-container')
  };

  function init() {
    setupMainTabSwitching();
    setupCardEventDelegation();
    setupWebSocket();
    setupGlobalHotkeys();
    setupActions();
  }

  function setupMainTabSwitching() {
    const tabs = [
      { btn: el.tabBtnTelemetry, page: el.pageTelemetry, name: 'telemetry' },
      { btn: el.tabBtnHex, page: el.pageHex, name: 'hex' },
      { btn: el.tabBtnLogs, page: el.pageLogs, name: 'logs' }
    ];

    tabs.forEach(t => {
      if (!t.btn) return;
      t.btn.addEventListener('click', () => switchTab(t.name));
    });

    if (el.btnGotoLogs) {
      el.btnGotoLogs.addEventListener('click', () => switchTab('logs'));
    }
  }

  function switchTab(tabName) {
    state.activeTab = tabName;
    [el.tabBtnTelemetry, el.tabBtnHex, el.tabBtnLogs].forEach(btn => {
      if (!btn) return;
      btn.classList.toggle('active', btn.dataset.tab === tabName);
    });

    [el.pageTelemetry, el.pageHex, el.pageLogs].forEach(page => {
      if (!page) return;
      page.classList.toggle('active', page.id === `page-${tabName}`);
    });
  }

  // EVENT DELEGATION FOR SYSTEM CARDS (Prevents DOM destruction bugs)
  function setupCardEventDelegation() {
    if (!el.systemsCardList) return;

    el.systemsCardList.addEventListener('click', (e) => {
      const actionBtn = e.target.closest('[data-action]');
      if (actionBtn) {
        e.stopPropagation();
        const action = actionBtn.dataset.action;
        const id = actionBtn.dataset.id;

        if (action === 'start') {
          sendWsCommand({ type: 'start', id });
          showToast(`🚀 ${id} başlatılıyor...`, 'info');
        } else if (action === 'stop') {
          sendWsCommand({ type: 'stop', id });
          showToast(`⏹️ ${id} durduruluyor...`, 'info');
        } else if (action === 'monitor') {
          selectSystem(id);
        } else if (action === 'delete') {
          if (confirm(`${id} eklentisini silmek istediğinizden emin misiniz?`)) {
            sendWsCommand({ type: 'delete', id });
            showToast(`🗑️ ${id} silindi.`, 'info');
          }
        }
        return;
      }

      const card = e.target.closest('.system-card');
      if (card && card.dataset.id) {
        selectSystem(card.dataset.id);
      }
    });
  }

  // WebSocket Client targeting Port 8080
  function setupWebSocket() {
    let host = location.host;
    if (!host || location.protocol === 'file:') {
      host = 'localhost:8080';
    }
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${host}/ws`;

    updateServerBadge('loading', 'Port 8080 Bağlanıyor...');

    try {
      state.ws = new WebSocket(wsUrl);

      state.ws.onopen = () => {
        state.wsConnected = true;
        updateServerBadge('online', 'Port 8080 Canlı Bağlandı (0ms RAM)');
        showToast('Port 8080 Telemetri WebSocket bağlantısı kuruldu.', 'success');
        sendWsCommand({ type: 'ping' });
      };

      state.ws.onmessage = (event) => {
        try {
          const msg = JSON.parse(event.data);
          handleServerFrame(msg);
        } catch (err) {
          console.error('WS Parse Error:', err);
        }
      };

      state.ws.onclose = () => {
        state.wsConnected = false;
        updateServerBadge('offline', 'Bağlantı Kesildi');
        setTimeout(setupWebSocket, 2000);
      };

      state.ws.onerror = () => {
        state.wsConnected = false;
        updateServerBadge('offline', 'Hata');
      };
    } catch (e) {
      updateServerBadge('offline', 'Bağlanamadı');
      setTimeout(setupWebSocket, 3000);
    }
  }

  function sendWsCommand(cmdObj) {
    if (state.ws && state.ws.readyState === WebSocket.OPEN) {
      state.ws.send(JSON.stringify(cmdObj));
    } else {
      showToast('WebSocket bağlantısı hazır değil!', 'error');
    }
  }

  function updateServerBadge(type, text) {
    if (!el.serverStatusBadge) return;
    if (type === 'online') {
      el.serverStatusBadge.className = 'sub-badge success';
      el.serverStatusBadge.innerHTML = `<i class="fa-solid fa-circle-check"></i> ${text}`;
    } else if (type === 'loading') {
      el.serverStatusBadge.className = 'sub-badge warning';
      el.serverStatusBadge.innerHTML = `<i class="fa-solid fa-circle-notch fa-spin"></i> ${text}`;
    } else {
      el.serverStatusBadge.className = 'sub-badge danger';
      el.serverStatusBadge.innerHTML = `<i class="fa-solid fa-triangle-exclamation"></i> ${text}`;
    }
  }

  function handleServerFrame(msg) {
    if (msg.type === 'telemetry') {
      renderTelemetry(msg);
    } else if (msg.type === 'log') {
      appendLogMessage(msg.message);
    }
  }

  function renderTelemetry(data) {
    state.systems = data.systems || [];

    if (el.valTotalSystems) el.valTotalSystems.textContent = data.telemetry.total_systems;
    if (el.valRunningSystems) el.valRunningSystems.textContent = data.telemetry.running_systems;
    if (el.valCpuUsage) el.valCpuUsage.textContent = `${data.telemetry.cpu_usage.toFixed(1)}%`;
    if (el.valRamUsage) el.valRamUsage.textContent = `${data.telemetry.memory_used_mb} MB`;
    if (el.wsLatencyVal) el.wsLatencyVal.textContent = `< 1 ms`;

    // Smart DOM diffing check to prevent click interruption
    const systemsJson = JSON.stringify(state.systems) + `:${state.selectedSystemId}`;
    if (systemsJson !== state.lastSystemsJson && el.systemsCardList) {
      state.lastSystemsJson = systemsJson;

      if (state.systems.length === 0) {
        el.systemsCardList.innerHTML = `
          <div class="empty-state">
            <i class="fa-solid fa-ghost"></i>
            <p>Yüklü eklenti bulunamadı. Eklentileri derleyip orchestrator'ı çalıştırın.</p>
          </div>`;
      } else {
        el.systemsCardList.innerHTML = state.systems.map(s => {
          const isSelected = state.selectedSystemId === s.id;
          const statusBadge = s.is_running
            ? `<span class="badge-status running"><i class="fa-solid fa-play"></i> Çalışıyor</span>`
            : `<span class="badge-status stopped"><i class="fa-solid fa-stop"></i> Durduruldu</span>`;
          const validBadge = s.is_data_valid
            ? `<span style="color:var(--accent-emerald);">[RAM Geçerli]</span>`
            : `<span style="color:var(--text-dim);">[Beklemede]</span>`;

          return `
            <div class="system-card ${isSelected ? 'active-monitored' : ''}" data-id="${s.id}">
              <div class="sys-info-group">
                <div class="sys-title-row">
                  <h4>${s.name}</h4>
                  ${statusBadge}
                </div>
                <div class="sys-sub-row">
                  <span>RAM Adresi: <strong>${s.memory_addr}</strong></span>
                  <span>Veri Durumu: ${validBadge}</span>
                </div>
              </div>
              <div class="sys-controls">
                <button class="btn btn-secondary btn-sm" data-action="start" data-id="${s.id}" title="Başlat [S]">
                  <i class="fa-solid fa-play" style="color:var(--accent-emerald);"></i> Başlat
                </button>
                <button class="btn btn-secondary btn-sm" data-action="stop" data-id="${s.id}" title="Durdur [X]">
                  <i class="fa-solid fa-stop" style="color:var(--accent-pink);"></i> Durdur
                </button>
                <button class="btn btn-secondary btn-sm" data-action="monitor" data-id="${s.id}" title="Canlı Takip [M]">
                  <i class="fa-solid fa-eye" style="color:var(--accent-cyan);"></i> İzle
                </button>
                <button class="btn btn-secondary btn-sm" data-action="delete" data-id="${s.id}" title="Sil [D]">
                  <i class="fa-solid fa-trash"></i> Sil
                </button>
              </div>
            </div>`;
        }).join('');
      }
    }

    if (data.monitored_id) {
      state.selectedSystemId = data.monitored_id;
      if (el.focusSystemTitle) el.focusSystemTitle.textContent = data.monitored_id;
      if (el.hexSystemName) el.hexSystemName.textContent = `Seçili: ${data.monitored_id}`;
    }

    if (data.monitored_hex !== undefined) {
      const hexText = data.monitored_hex || 'Buffer boş (0 byte)';
      const asciiText = data.monitored_str || 'Metin çıktısı yok';

      if (el.focusPreviewHex) el.focusPreviewHex.textContent = hexText.slice(0, 300);
      if (el.focusBufferLen) el.focusBufferLen.textContent = `${data.monitored_bytes_len} bytes`;

      if (!state.pauseHexStream) {
        if (el.hexMatrixDisplay) el.hexMatrixDisplay.textContent = hexText;
        if (el.asciiStringDisplay) el.asciiStringDisplay.textContent = asciiText;
      }
    }
  }

  function selectSystem(id) {
    state.selectedSystemId = id;
    sendWsCommand({ type: 'monitor', id });
    showToast(`İzleme odaklandı: ${id}`, 'info');
  }

  function appendLogMessage(logLine) {
    state.logs.push(logLine);
    if (state.logs.length > 200) state.logs.shift();

    if (el.miniLogBox) {
      const div = document.createElement('div');
      div.className = 'log-line';
      div.textContent = logLine;
      el.miniLogBox.appendChild(div);
      if (el.miniLogBox.children.length > 30) el.miniLogBox.removeChild(el.miniLogBox.firstChild);
      el.miniLogBox.scrollTop = el.miniLogBox.scrollHeight;
    }

    if (el.mainConsoleOutput) {
      const div = document.createElement('div');
      div.className = 'log-line';
      if (logLine.includes('HATA') || logLine.includes('hata')) div.classList.add('error');
      else if (logLine.includes('UYARI') || logLine.includes('uyarı')) div.classList.add('warn');
      else div.classList.add('info');

      div.textContent = logLine;
      el.mainConsoleOutput.appendChild(div);

      if (state.autoscrollLogs) {
        el.mainConsoleOutput.scrollTop = el.mainConsoleOutput.scrollHeight;
      }
    }
  }

  function setupActions() {
    if (el.btnRefreshSystems) {
      el.btnRefreshSystems.addEventListener('click', () => {
        state.lastSystemsJson = '';
        sendWsCommand({ type: 'ping' });
      });
    }

    if (el.btnClearLogs) {
      el.btnClearLogs.addEventListener('click', () => {
        state.logs = [];
        if (el.mainConsoleOutput) el.mainConsoleOutput.innerHTML = '';
        if (el.miniLogBox) el.miniLogBox.innerHTML = '';
      });
    }

    if (el.btnToggleAutoscroll) {
      el.btnToggleAutoscroll.addEventListener('click', () => {
        state.autoscrollLogs = !state.autoscrollLogs;
        el.btnToggleAutoscroll.innerHTML = `<i class="fa-solid fa-arrow-down-long"></i> Oto-Kaydır: ${state.autoscrollLogs ? 'AÇIK' : 'KAPALI'}`;
      });
    }

    if (el.btnPauseHex) {
      el.btnPauseHex.addEventListener('click', () => {
        state.pauseHexStream = !state.pauseHexStream;
        el.btnPauseHex.innerHTML = `<i class="fa-solid fa-${state.pauseHexStream ? 'play' : 'pause'}"></i> ${state.pauseHexStream ? 'Devam Et' : 'Duraklat'}`;
      });
    }

    if (el.btnClearHex) {
      el.btnClearHex.addEventListener('click', () => {
        if (el.hexMatrixDisplay) el.hexMatrixDisplay.textContent = 'Temizlendi.';
        if (el.asciiStringDisplay) el.asciiStringDisplay.textContent = 'Temizlendi.';
      });
    }
  }

  function setupGlobalHotkeys() {
    window.addEventListener('keydown', (e) => {
      const tag = document.activeElement.tagName.toLowerCase();
      if (tag === 'input' || tag === 'textarea' || tag === 'select') return;

      const key = e.key.toLowerCase();
      if (key === '1') { switchTab('telemetry'); return; }
      if (key === '2') { switchTab('hex'); return; }
      if (key === '3') { switchTab('logs'); return; }

      if (state.selectedSystemId) {
        if (key === 's') {
          e.preventDefault();
          sendWsCommand({ type: 'start', id: state.selectedSystemId });
          showToast(`[S] ${state.selectedSystemId} başlatıldı.`, 'success');
        } else if (key === 'x') {
          e.preventDefault();
          sendWsCommand({ type: 'stop', id: state.selectedSystemId });
          showToast(`[X] ${state.selectedSystemId} durduruldu.`, 'warn');
        } else if (key === 'm') {
          e.preventDefault();
          sendWsCommand({ type: 'monitor', id: state.selectedSystemId });
          switchTab('hex');
        } else if (key === 'd') {
          e.preventDefault();
          sendWsCommand({ type: 'delete', id: state.selectedSystemId });
        }
      }
    });
  }

  function showToast(msg, type = 'info') {
    if (!el.toastContainer) return;
    const toast = document.createElement('div');
    toast.className = `toast ${type}`;
    toast.innerHTML = `<i class="fa-solid fa-${type === 'success' ? 'circle-check' : 'circle-info'}"></i> ${msg}`;
    el.toastContainer.appendChild(toast);
    setTimeout(() => toast.remove(), 3500);
  }

  document.addEventListener('DOMContentLoaded', init);

})();
