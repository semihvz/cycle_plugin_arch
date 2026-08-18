/**
 * Cycle-ORC Telemetry Console & Full Visual JSON Studio (Port 8080)
 * High-Speed Zero-Latency WebSocket Controller & Interactive Pipeline Flow Studio
 */

(function () {
  'use strict';

  // Application State Store
  const state = {
    activeTab: 'telemetry',
    ws: null,
    wsConnected: false,
    selectedSystemId: null,
    systems: [],
    availablePlugins: [],
    lastSystemsJson: '',
    lastAvailableJson: '',
    logs: [],
    shellHistory: [],
    shellHistoryIdx: -1,
    autoscrollLogs: true,
    pauseHexStream: false,
    // Visual JSON Studio State
    jsonData: null,
    rawText: '',
    isValidJson: true,
    history: [],
    historyIndex: -1,
    editorViewMode: 'split', // 'split' | 'tree' | 'flow' | 'code'
    searchQuery: '',
    collapsedPaths: new Set(),
    flowPositions: new Map(), // pluginIndex -> { x, y }
    selectedNodeIdx: null
  };

  // DOM Elements Registry
  const el = {
    serverStatusBadge: document.getElementById('server-status-badge'),
    latencyPill: document.getElementById('latency-pill'),
    wsLatencyVal: document.getElementById('ws-latency-val'),
    tabBtnTelemetry: document.getElementById('tab-btn-telemetry'),
    tabBtnHex: document.getElementById('tab-btn-hex'),
    tabBtnLogs: document.getElementById('tab-btn-logs'),
    tabBtnShell: document.getElementById('tab-btn-shell'),
    tabBtnEditor: document.getElementById('tab-btn-editor'),
    pageTelemetry: document.getElementById('page-telemetry'),
    pageHex: document.getElementById('page-hex'),
    pageLogs: document.getElementById('page-logs'),
    pageShell: document.getElementById('page-shell'),
    pageEditor: document.getElementById('page-editor'),
    valTotalSystems: document.getElementById('val-total-systems'),
    valRunningSystems: document.getElementById('val-running-systems'),
    valCpuUsage: document.getElementById('val-cpu-usage'),
    valRamUsage: document.getElementById('val-ram-usage'),
    systemsCardList: document.getElementById('systems-card-list'),
    btnRefreshSystems: document.getElementById('btn-refresh-systems'),
    btnOpenLoadModal: document.getElementById('btn-open-load-modal'),
    loadPluginModal: document.getElementById('load-plugin-modal'),
    closeLoadModalBtn: document.getElementById('close-load-modal-btn'),
    cancelLoadModalBtn: document.getElementById('cancel-load-modal-btn'),
    confirmLoadModalBtn: document.getElementById('confirm-load-modal-btn'),
    availablePluginsGrid: document.getElementById('available-plugins-grid'),
    manualPluginInput: document.getElementById('manual-plugin-input'),
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
    shellOutputWindow: document.getElementById('shell-output-window'),
    shellInputField: document.getElementById('shell-input-field'),
    shellSendBtn: document.getElementById('shell-send-btn'),
    // Visual JSON Studio Elements
    btnViewSplit: document.getElementById('btn-view-split'),
    btnViewTree: document.getElementById('btn-view-tree'),
    btnViewFlow: document.getElementById('btn-view-flow'),
    btnViewCode: document.getElementById('btn-view-code'),
    mainWorkspace: document.getElementById('main-workspace'),
    paneCode: document.getElementById('pane-code'),
    paneVisual: document.getElementById('pane-visual'),
    viewTreeContainer: document.getElementById('view-tree-container'),
    viewFlowContainer: document.getElementById('view-flow-container'),
    rawJsonTextarea: document.getElementById('raw-json-textarea'),
    lineNumbers: document.getElementById('line-numbers'),
    codeValidStatus: document.getElementById('code-valid-status'),
    treeContentRoot: document.getElementById('tree-content-root'),
    flowNodesLayer: document.getElementById('flow-nodes-layer'),
    flowSvgLayer: document.getElementById('flow-svg-layer'),
    globalSearchInput: document.getElementById('global-search-input'),
    clearSearchBtn: document.getElementById('clear-search-btn'),
    btnUndo: document.getElementById('btn-undo'),
    btnRedo: document.getElementById('btn-redo'),
    btnFormat: document.getElementById('btn-format'),
    btnReload: document.getElementById('btn-reload'),
    btnSaveWorkspace: document.getElementById('btn-save-workspace'),
    btnCopyCode: document.getElementById('btn-copy-code'),
    btnMinifyCode: document.getElementById('btn-minify-code'),
    btnTemplates: document.getElementById('btn-templates'),
    templatesMenu: document.getElementById('templates-menu'),
    jsonBreadcrumbs: document.getElementById('json-breadcrumbs'),
    statsInfo: document.getElementById('stats-info'),
    btnExpandAll: document.getElementById('btn-expand-all'),
    btnCollapseAll: document.getElementById('btn-collapse-all'),
    btnAddPlugin: document.getElementById('btn-add-plugin'),
    nodeInspector: document.getElementById('node-inspector'),
    closeInspectorBtn: document.getElementById('close-inspector-btn'),
    inspectorBody: document.getElementById('inspector-body'),
    toastContainer: document.getElementById('toast-container')
  };

  function init() {
    setupMainTabSwitching();
    setupCardEventDelegation();
    setupWebSocket();
    setupGlobalHotkeys();
    setupActions();
    setupShellInput();
    setupVisualStudioHandlers();
    fetchConfig();
  }

  function setupMainTabSwitching() {
    const tabs = [
      { btn: el.tabBtnTelemetry, page: el.pageTelemetry, name: 'telemetry' },
      { btn: el.tabBtnHex, page: el.pageHex, name: 'hex' },
      { btn: el.tabBtnLogs, page: el.pageLogs, name: 'logs' },
      { btn: el.tabBtnShell, page: el.pageShell, name: 'shell' },
      { btn: el.tabBtnEditor, page: el.pageEditor, name: 'editor' }
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
    [el.tabBtnTelemetry, el.tabBtnHex, el.tabBtnLogs, el.tabBtnShell, el.tabBtnEditor].forEach(btn => {
      if (!btn) return;
      btn.classList.toggle('active', btn.dataset.tab === tabName);
    });

    [el.pageTelemetry, el.pageHex, el.pageLogs, el.pageShell, el.pageEditor].forEach(page => {
      if (!page) return;
      page.classList.toggle('active', page.id === `page-${tabName}`);
    });

    if (tabName === 'shell' && el.shellInputField) {
      setTimeout(() => el.shellInputField.focus(), 100);
    } else if (tabName === 'editor') {
      fetchConfig();
    }
  }

  // EVENT DELEGATION FOR SYSTEM CARDS
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
    } else if (msg.type === 'shell_output') {
      appendShellOutput(msg.command, msg.output);
    }
  }

  function renderTelemetry(data) {
    state.systems = data.systems || [];
    state.availablePlugins = data.available_plugins || [];

    if (el.valTotalSystems) el.valTotalSystems.textContent = data.telemetry.total_systems;
    if (el.valRunningSystems) el.valRunningSystems.textContent = data.telemetry.running_systems;
    if (el.valCpuUsage) el.valCpuUsage.textContent = `${data.telemetry.cpu_usage.toFixed(1)}%`;
    if (el.valRamUsage) el.valRamUsage.textContent = `${data.telemetry.memory_used_mb} MB`;
    if (el.wsLatencyVal) el.wsLatencyVal.textContent = `< 1 ms`;

    // Render Available Plugins Pill Grid in Modal
    const availableJson = JSON.stringify(state.availablePlugins);
    if (availableJson !== state.lastAvailableJson && el.availablePluginsGrid) {
      state.lastAvailableJson = availableJson;
      if (state.availablePlugins.length === 0) {
        el.availablePluginsGrid.innerHTML = `<div style="grid-column:1/-1; color:var(--text-muted); font-size:0.8rem;">Eklenti bulunamadı (target/debug).</div>`;
      } else {
        el.availablePluginsGrid.innerHTML = state.availablePlugins.map(name => {
          const isLoaded = state.systems.some(s => s.id === name);
          const badgeHtml = isLoaded
            ? `<span style="color:var(--accent-emerald); font-size:0.7rem;"><i class="fa-solid fa-check"></i> Yüklü</span>`
            : `<span style="color:var(--accent-cyan); font-size:0.7rem;"><i class="fa-solid fa-plus"></i> Yükle</span>`;

          return `
            <div class="plugin-pill-item" data-name="${name}">
              <span>${name}</span>
              ${badgeHtml}
            </div>`;
        }).join('');

        el.availablePluginsGrid.querySelectorAll('.plugin-pill-item').forEach(pill => {
          pill.addEventListener('click', () => {
            const pluginName = pill.dataset.name;
            loadPlugin(pluginName);
          });
        });
      }
    }

    // Smart DOM diffing check to prevent click interruption
    const systemsJson = JSON.stringify(state.systems) + `:${state.selectedSystemId}`;
    if (systemsJson !== state.lastSystemsJson && el.systemsCardList) {
      state.lastSystemsJson = systemsJson;

      if (state.systems.length === 0) {
        el.systemsCardList.innerHTML = `
          <div class="empty-state">
            <i class="fa-solid fa-ghost"></i>
            <p>Yüklü eklenti bulunamadı. "Eklenti Yükle" butonuna tıklayarak sisteme eklenti bağlayın.</p>
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

  /* ==========================================================================
     FULL FEATURED VISUAL JSON & PIPELINE FLOW STUDIO
     ========================================================================== */

  function fetchConfig() {
    fetch('/api/config')
      .then(res => res.json())
      .then(data => {
        updateDataStore(data, true);
        showToast('flow_config.json başarıyla yüklendi.', 'info');
      })
      .catch(err => {
        console.error('Config fetch error:', err);
      });
  }

  function saveConfig() {
    if (!state.isValidJson) {
      showToast('Geçersiz JSON formatı! Lütfen hatayı düzeltin.', 'error');
      return;
    }

    fetch('/api/config', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(state.jsonData)
    })
      .then(res => res.json())
      .then(resData => {
        if (resData.status === 'ok') {
          showToast('💾 flow_config.json başarıyla kaydedildi!', 'success');
        } else {
          showToast('Kaydetme hatası!', 'error');
        }
      })
      .catch(err => showToast('Kaydetme isteği başarısız!', 'error'));
  }

  function updateDataStore(newData, resetHistory = false) {
    state.jsonData = newData;
    state.rawText = JSON.stringify(newData, null, 2);
    state.isValidJson = true;

    if (resetHistory) {
      state.history = [state.rawText];
      state.historyIndex = 0;
    } else {
      pushHistory(state.rawText);
    }

    if (el.rawJsonTextarea) el.rawJsonTextarea.value = state.rawText;
    updateLineNumbers();
    updateValidationStatus(true);
    updateStatsInfo();
    renderAllViews();
    updateUndoRedoButtons();
  }

  function pushHistory(text) {
    if (state.history[state.historyIndex] === text) return;
    state.history = state.history.slice(0, state.historyIndex + 1);
    state.history.push(text);
    state.historyIndex = state.history.length - 1;
    updateUndoRedoButtons();
  }

  function undo() {
    if (state.historyIndex > 0) {
      state.historyIndex--;
      applyHistoryState();
    }
  }

  function redo() {
    if (state.historyIndex < state.history.length - 1) {
      state.historyIndex++;
      applyHistoryState();
    }
  }

  function applyHistoryState() {
    const text = state.history[state.historyIndex];
    try {
      state.jsonData = JSON.parse(text);
      state.rawText = text;
      state.isValidJson = true;
      if (el.rawJsonTextarea) el.rawJsonTextarea.value = text;
      updateLineNumbers();
      updateValidationStatus(true);
      updateStatsInfo();
      renderAllViews();
      showToast('Değişiklik uygulandı.', 'info');
    } catch (e) {}
    updateUndoRedoButtons();
  }

  function updateUndoRedoButtons() {
    if (el.btnUndo) el.btnUndo.disabled = state.historyIndex <= 0;
    if (el.btnRedo) el.btnRedo.disabled = state.historyIndex >= state.history.length - 1;
  }

  function setEditorViewMode(mode) {
    state.editorViewMode = mode;
    [el.btnViewSplit, el.btnViewTree, el.btnViewFlow, el.btnViewCode].forEach(btn => {
      if (!btn) return;
      btn.classList.toggle('active', btn.dataset.view === mode);
    });

    if (!el.mainWorkspace) return;
    el.mainWorkspace.className = `main-workspace ${mode}-mode`;

    if (el.viewTreeContainer) el.viewTreeContainer.classList.toggle('active', mode === 'split' || mode === 'tree');
    if (el.viewFlowContainer) el.viewFlowContainer.classList.toggle('active', mode === 'flow');

    if (mode === 'flow') {
      setTimeout(renderFlowCanvas, 50);
    }
  }

  function updateLineNumbers() {
    if (!el.rawJsonTextarea || !el.lineNumbers) return;
    const lines = el.rawJsonTextarea.value.split('\n').length;
    let numbers = '';
    for (let i = 1; i <= lines; i++) {
      numbers += i + '\n';
    }
    el.lineNumbers.textContent = numbers;
  }

  function updateValidationStatus(isValid) {
    state.isValidJson = isValid;
    if (!el.codeValidStatus) return;
    if (isValid) {
      el.codeValidStatus.className = 'valid-indicator';
      el.codeValidStatus.innerHTML = `<i class="fa-solid fa-circle-check"></i> Geçerli JSON`;
    } else {
      el.codeValidStatus.className = 'valid-indicator invalid';
      el.codeValidStatus.innerHTML = `<i class="fa-solid fa-circle-xmark"></i> Hatalı Syntax`;
    }
  }

  function updateStatsInfo() {
    if (!el.statsInfo) return;
    const count = Array.isArray(state.jsonData) ? state.jsonData.length : (state.jsonData ? 1 : 0);
    el.statsInfo.innerHTML = `<i class="fa-solid fa-cubes"></i> ${count} Düğüm | ${count > 1 ? count - 1 : 0} Akış Bağlantısı`;
  }

  function renderAllViews() {
    renderTree();
    if (state.editorViewMode === 'flow') {
      renderFlowCanvas();
    }
  }

  function renderTree() {
    if (!el.treeContentRoot) return;

    if (!Array.isArray(state.jsonData) || state.jsonData.length === 0) {
      el.treeContentRoot.innerHTML = `<div style="color:var(--text-muted); font-size:0.85rem;">Düğüm bulunamadı. Lütfen bir şablon seçin veya eklenti ekleyin.</div>`;
      return;
    }

    el.treeContentRoot.innerHTML = state.jsonData.map((node, nodeIdx) => {
      const pluginName = node.plugin_name || `Düğüm #${nodeIdx + 1}`;
      const keys = Object.keys(node).filter(k => k !== 'plugin_name');

      const paramsHtml = keys.map(k => {
        let valStr = typeof node[k] === 'object' ? JSON.stringify(node[k]) : String(node[k]);
        return `
          <div class="node-row" style="margin-left: 20px;">
            <span class="node-key">${k}</span>:
            <input type="text" class="node-val-input" data-node="${nodeIdx}" data-key="${k}" value="${valStr.replace(/"/g, '&quot;')}">
          </div>`;
      }).join('');

      return `
        <div class="json-node-card" style="margin-bottom: 12px;">
          <div class="node-header-row">
            <h5>🧩 ${pluginName}</h5>
            <button class="btn-micro" data-remove-node="${nodeIdx}"><i class="fa-solid fa-trash"></i> Sil</button>
          </div>
          <div class="node-params-grid">
            ${paramsHtml}
          </div>
        </div>`;
    }).join('');

    // Attach inline value change handlers
    el.treeContentRoot.querySelectorAll('.node-val-input').forEach(input => {
      input.addEventListener('change', () => {
        const nodeIdx = parseInt(input.dataset.node);
        const key = input.dataset.key;
        let newRaw = input.value;
        let parsedVal = newRaw;
        if (newRaw === 'true') parsedVal = true;
        else if (newRaw === 'false') parsedVal = false;
        else if (!isNaN(Number(newRaw)) && newRaw.trim() !== '') parsedVal = Number(newRaw);

        if (state.jsonData[nodeIdx]) {
          state.jsonData[nodeIdx][key] = parsedVal;
          state.rawText = JSON.stringify(state.jsonData, null, 2);
          if (el.rawJsonTextarea) el.rawJsonTextarea.value = state.rawText;
          pushHistory(state.rawText);
        }
      });
    });

    el.treeContentRoot.querySelectorAll('[data-remove-node]').forEach(btn => {
      btn.addEventListener('click', () => {
        const idx = parseInt(btn.dataset.removeNode);
        state.jsonData.splice(idx, 1);
        updateDataStore(state.jsonData);
      });
    });
  }

  /* FLOW GRAPH CANVAS RENDERER */
  function renderFlowCanvas() {
    if (!el.flowNodesLayer || !el.flowSvgLayer) return;

    if (!Array.isArray(state.jsonData) || state.jsonData.length === 0) {
      el.flowNodesLayer.innerHTML = `<div style="padding:20px; color:var(--text-muted);">Akış grafiği için düğüm bulunamadı.</div>`;
      el.flowSvgLayer.innerHTML = '';
      return;
    }

    let nodeHtml = '';
    const startX = 60;
    const startY = 60;
    const gapX = 320;

    state.jsonData.forEach((plugin, idx) => {
      if (!state.flowPositions.has(idx)) {
        state.flowPositions.set(idx, { x: startX + (idx * gapX), y: startY + (idx % 2 * 60) });
      }
      const pos = state.flowPositions.get(idx);
      const isSelected = state.selectedNodeIdx === idx;

      nodeHtml += `
        <div class="flow-node-card ${isSelected ? 'selected' : ''}" data-node-idx="${idx}" style="left:${pos.x}px; top:${pos.y}px;">
          <div class="flow-node-header">
            <span class="flow-node-title"><i class="fa-solid fa-cube"></i> ${plugin.plugin_name || 'Eklenti'}</span>
            <span class="flow-node-category">Düğüm #${idx + 1}</span>
          </div>
          <div class="flow-ports-body">
            <div class="port-group">
              <span class="port-title">Girdi / Çıktı Portları</span>
              <div class="port-item">
                <span>Data In</span>
                <span class="port-dot" title="Giriş Portu"></span>
              </div>
              <div class="port-item">
                <span>Data Out</span>
                <span class="port-dot" title="Çıkış Portu"></span>
              </div>
            </div>
          </div>
        </div>`;
    });

    el.flowNodesLayer.innerHTML = nodeHtml;
    drawFlowConnections();

    // Enable Node Dragging
    el.flowNodesLayer.querySelectorAll('.flow-node-card').forEach(card => {
      let isDragging = false;
      let startMouseX = 0, startMouseY = 0;
      let initialNodeX = 0, initialNodeY = 0;

      card.addEventListener('mousedown', (e) => {
        isDragging = true;
        const idx = parseInt(card.dataset.nodeIdx);
        state.selectedNodeIdx = idx;
        const pos = state.flowPositions.get(idx);
        initialNodeX = pos.x;
        initialNodeY = pos.y;
        startMouseX = e.clientX;
        startMouseY = e.clientY;

        const onMouseMove = (moveEvt) => {
          if (!isDragging) return;
          const dx = moveEvt.clientX - startMouseX;
          const dy = moveEvt.clientY - startMouseY;
          const newX = Math.max(10, initialNodeX + dx);
          const newY = Math.max(10, initialNodeY + dy);
          card.style.left = `${newX}px`;
          card.style.top = `${newY}px`;
          state.flowPositions.set(idx, { x: newX, y: newY });
          drawFlowConnections();
        };

        const onMouseUp = () => {
          isDragging = false;
          window.removeEventListener('mousemove', onMouseMove);
          window.removeEventListener('mouseup', onMouseUp);
        };

        window.addEventListener('mousemove', onMouseMove);
        window.addEventListener('mouseup', onMouseUp);
      });
    });
  }

  function drawFlowConnections() {
    if (!el.flowSvgLayer || !Array.isArray(state.jsonData)) return;

    let svgHtml = `
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--accent-cyan)" />
        </marker>
      </defs>`;

    for (let i = 0; i < state.jsonData.length - 1; i++) {
      const p1 = state.flowPositions.get(i);
      const p2 = state.flowPositions.get(i + 1);
      if (!p1 || !p2) continue;

      const x1 = p1.x + 260;
      const y1 = p1.y + 70;
      const x2 = p2.x;
      const y2 = p2.y + 70;

      const cx1 = x1 + (x2 - x1) / 2;
      const cy1 = y1;
      const cx2 = x1 + (x2 - x1) / 2;
      const cy2 = y2;

      const pathData = `M ${x1} ${y1} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${x2} ${y2}`;
      svgHtml += `<path class="flow-svg-wire" d="${pathData}" marker-end="url(#arrow)" />`;
    }

    el.flowSvgLayer.innerHTML = svgHtml;
  }

  function setupVisualStudioHandlers() {
    if (el.btnViewSplit) el.btnViewSplit.addEventListener('click', () => setEditorViewMode('split'));
    if (el.btnViewTree) el.btnViewTree.addEventListener('click', () => setEditorViewMode('tree'));
    if (el.btnViewFlow) el.btnViewFlow.addEventListener('click', () => setEditorViewMode('flow'));
    if (el.btnViewCode) el.btnViewCode.addEventListener('click', () => setEditorViewMode('code'));

    if (el.btnUndo) el.btnUndo.addEventListener('click', undo);
    if (el.btnRedo) el.btnRedo.addEventListener('click', redo);

    if (el.btnFormat) {
      el.btnFormat.addEventListener('click', () => {
        if (!state.isValidJson) return;
        state.rawText = JSON.stringify(state.jsonData, null, 2);
        if (el.rawJsonTextarea) el.rawJsonTextarea.value = state.rawText;
        updateLineNumbers();
        pushHistory(state.rawText);
        showToast('JSON formatlandı (Prettify).', 'info');
      });
    }

    if (el.btnReload) el.btnReload.addEventListener('click', fetchConfig);
    if (el.btnSaveWorkspace) el.btnSaveWorkspace.addEventListener('click', saveConfig);

    if (el.btnCopyCode) {
      el.btnCopyCode.addEventListener('click', () => {
        if (el.rawJsonTextarea) {
          navigator.clipboard.writeText(el.rawJsonTextarea.value);
          showToast('JSON panoya kopyalandı.', 'success');
        }
      });
    }

    if (el.btnMinifyCode) {
      el.btnMinifyCode.addEventListener('click', () => {
        if (!state.isValidJson) return;
        state.rawText = JSON.stringify(state.jsonData);
        if (el.rawJsonTextarea) el.rawJsonTextarea.value = state.rawText;
        updateLineNumbers();
        pushHistory(state.rawText);
        showToast('JSON sıkıştırıldı (Minify).', 'info');
      });
    }

    // Template Selector
    if (el.templatesMenu) {
      el.templatesMenu.querySelectorAll('[data-template]').forEach(item => {
        item.addEventListener('click', (e) => {
          e.preventDefault();
          const tName = item.dataset.template;
          if (tName === 'default_flow') {
            updateDataStore([
              { plugin_name: "plugin_binance_gateway", symbol: "btcusdt", enable: true },
              { plugin_name: "plugin_aggtrade_ohlcv", timeframe: 60, enable: true },
              { plugin_name: "plugin_breakout", threshold: 1.5, enable: true },
              { plugin_name: "plugin_binance_trader", leverage: 10, enable: true }
            ]);
          } else if (tName === 'hft_pipeline') {
            updateDataStore([
              { plugin_name: "plugin_oi_fetcher", symbol: "ethusdt" },
              { plugin_name: "plugin_ms_analyzer", threshold_ms: 50 },
              { plugin_name: "plugin_paper_exchange", balance: 10000 }
            ]);
          } else if (tName === 'empty_array') {
            updateDataStore([]);
          }
          showToast(`Şablon yüklendi: ${tName}`, 'success');
        });
      });
    }

    if (el.btnAddPlugin || el.btnEditorAddNode) {
      const handler = () => {
        const name = prompt('Eklenti Adı (örn: plugin_breakout):', 'plugin_new');
        if (name) {
          if (!Array.isArray(state.jsonData)) state.jsonData = [];
          state.jsonData.push({
            plugin_name: name,
            symbol: 'BTCUSDT',
            enable: true
          });
          updateDataStore(state.jsonData);
        }
      };
      if (el.btnAddPlugin) el.btnAddPlugin.addEventListener('click', handler);
      if (el.btnEditorAddNode) el.btnEditorAddNode.addEventListener('click', handler);
    }

    if (el.rawJsonTextarea) {
      el.rawJsonTextarea.addEventListener('input', () => {
        updateLineNumbers();
        const text = el.rawJsonTextarea.value;
        try {
          state.jsonData = JSON.parse(text);
          state.rawText = text;
          updateValidationStatus(true);
          updateStatsInfo();
          renderAllViews();
          pushHistory(text);
        } catch (e) {
          updateValidationStatus(false);
        }
      });
    }
  }

  function setupShellInput() {
    if (!el.shellInputField) return;

    el.shellInputField.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        submitShellCommand();
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        navigateShellHistory(-1);
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        navigateShellHistory(1);
      }
    });

    if (el.shellSendBtn) {
      el.shellSendBtn.addEventListener('click', submitShellCommand);
    }

    document.querySelectorAll('.shell-badge-cmd').forEach(badge => {
      badge.addEventListener('click', () => {
        const cmd = badge.dataset.cmd;
        if (cmd === 'clear') {
          clearShellOutput();
        } else {
          el.shellInputField.value = cmd;
          submitShellCommand();
        }
      });
    });
  }

  function submitShellCommand() {
    const cmd = el.shellInputField.value.trim();
    if (!cmd) return;

    if (cmd.toLowerCase() === 'clear') {
      clearShellOutput();
      el.shellInputField.value = '';
      return;
    }

    state.shellHistory.push(cmd);
    state.shellHistoryIdx = state.shellHistory.length;

    sendWsCommand({ type: 'shell_input', command: cmd });
    el.shellInputField.value = '';
  }

  function navigateShellHistory(direction) {
    if (state.shellHistory.length === 0) return;
    state.shellHistoryIdx += direction;
    if (state.shellHistoryIdx < 0) state.shellHistoryIdx = 0;
    if (state.shellHistoryIdx >= state.shellHistory.length) {
      state.shellHistoryIdx = state.shellHistory.length;
      el.shellInputField.value = '';
      return;
    }
    el.shellInputField.value = state.shellHistory[state.shellHistoryIdx];
  }

  function appendShellOutput(cmd, outputText) {
    if (!el.shellOutputWindow) return;

    const cmdDiv = document.createElement('div');
    cmdDiv.className = 'shell-line cmd-input-line';
    cmdDiv.textContent = `> cycle-orc:hft-shell> ${cmd}`;
    el.shellOutputWindow.appendChild(cmdDiv);

    if (outputText) {
      const outDiv = document.createElement('div');
      outDiv.className = 'shell-line cmd-output-line';
      outDiv.textContent = outputText;
      el.shellOutputWindow.appendChild(outDiv);
    }

    el.shellOutputWindow.scrollTop = el.shellOutputWindow.scrollHeight;
  }

  function clearShellOutput() {
    if (!el.shellOutputWindow) return;
    el.shellOutputWindow.innerHTML = `
      <div class="shell-line sys-intro">
=== CYCLE-ORC INTERACTIVE HFT SHELL CONSOLE ===
Tüm orkestratör komutlarını (help, list, start <id>, stop <id>, del <id>, load <name>, status) yazabilirsiniz.
Komut geçmişinde gezinmek için [Yukarı / Aşağı] ok tuşlarını kullanabilirsiniz.
--------------------------------------------------------------------------------</div>`;
  }

  function loadPlugin(pluginName) {
    if (!pluginName) return;
    sendWsCommand({ type: 'load', name: pluginName });
    showToast(`🧩 ${pluginName} eklentisi yükleniyor...`, 'info');
    closeLoadModal();
  }

  function openLoadModal() {
    if (el.loadPluginModal) el.loadPluginModal.style.display = 'flex';
  }

  function closeLoadModal() {
    if (el.loadPluginModal) el.loadPluginModal.style.display = 'none';
    if (el.manualPluginInput) el.manualPluginInput.value = '';
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
    if (el.btnOpenLoadModal) {
      el.btnOpenLoadModal.addEventListener('click', openLoadModal);
    }

    if (el.closeLoadModalBtn) {
      el.closeLoadModalBtn.addEventListener('click', closeLoadModal);
    }

    if (el.cancelLoadModalBtn) {
      el.cancelLoadModalBtn.addEventListener('click', closeLoadModal);
    }

    if (el.confirmLoadModalBtn) {
      el.confirmLoadModalBtn.addEventListener('click', () => {
        const val = el.manualPluginInput ? el.manualPluginInput.value.trim() : '';
        if (val) loadPlugin(val);
        else showToast('Lütfen bir eklenti adı girin veya listeden seçin.', 'warn');
      });
    }

    if (el.btnRefreshSystems) {
      el.btnRefreshSystems.addEventListener('click', () => {
        state.lastSystemsJson = '';
        state.lastAvailableJson = '';
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
      if (key === '4' || key === '`') { switchTab('shell'); return; }
      if (key === '5') { switchTab('editor'); return; }
      if (key === 'a' || key === 'l') { e.preventDefault(); openLoadModal(); return; }

      if (e.ctrlKey && key === 's') {
        e.preventDefault();
        saveConfig();
        return;
      }
      if (e.ctrlKey && key === 'z') {
        e.preventDefault();
        undo();
        return;
      }
      if (e.ctrlKey && key === 'y') {
        e.preventDefault();
        redo();
        return;
      }

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
