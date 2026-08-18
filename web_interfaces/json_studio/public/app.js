/**
 * Cycle-ORC Visual JSON Studio - Core Application Logic
 * Interactive Visual JSON Editor & Flow Graph Canvas for flow_config.json
 */

(function () {
  'use strict';

  // State Store
  const state = {
    data: null,
    rawText: '',
    isValidJson: true,
    history: [],
    historyIndex: -1,
    viewMode: 'split', // 'split' | 'tree' | 'flow' | 'code'
    searchQuery: '',
    selectedPath: [],
    collapsedPaths: new Set(),
    flowNodePositions: new Map(), // pluginIndex -> { x, y }
    inspectorPluginIdx: null,
    activeAddTarget: null // { parentData, key, isArray }
  };

  // DOM Elements
  const el = {
    serverStatusBadge: document.getElementById('server-status-badge'),
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
    btnExpandAll: document.getElementById('btn-expand-all'),
    btnCollapseAll: document.getElementById('btn-collapse-all'),
    btnAddPlugin: document.getElementById('btn-add-plugin'),
    btnAddRootItem: document.getElementById('btn-add-root-item'),
    btnAutoLayout: document.getElementById('btn-auto-layout'),
    jsonBreadcrumbs: document.getElementById('json-breadcrumbs'),
    statsInfo: document.getElementById('stats-info'),
    nodeInspector: document.getElementById('node-inspector'),
    inspectorBody: document.getElementById('inspector-body'),
    closeInspectorBtn: document.getElementById('close-inspector-btn'),
    modalAddElement: document.getElementById('add-element-modal'),
    modalInputKey: document.getElementById('modal-input-key'),
    modalSelectType: document.getElementById('modal-select-type'),
    modalInputVal: document.getElementById('modal-input-val'),
    modalConfirmBtn: document.getElementById('modal-confirm-btn'),
    modalCancelBtn: document.getElementById('modal-cancel-btn'),
    closeModalBtn: document.getElementById('close-modal-btn'),
    toastContainer: document.getElementById('toast-container'),
    templatesMenu: document.getElementById('templates-menu')
  };

  // Sample Templates
  const TEMPLATES = {
    default_flow: [
      {
        plugin_name: "plugin_ohlcv_fetcher",
        plugin_inputs: [],
        plugin_params: {},
        plugin_outputs: ["btc_15m", "eth_5m"]
      },
      {
        plugin_name: "plugin_binance_gateway",
        plugin_inputs: [],
        plugin_params: { symbols: ["BTCUSDT", "ETHUSDT"] },
        plugin_outputs: ["stream_markprice", "stream_trades", "stream_aggtrades", "stream_depth", "stream_bestprice", "stream_liquidations"]
      },
      {
        plugin_name: "plugin_ms_analyzer",
        plugin_inputs: [
          { source: "plugin_ohlcv_fetcher", stream_id: "btc_15m", params: { symbol: "BTCUSDT", interval: "15m", limit: 5 } },
          { source: "plugin_ohlcv_fetcher", stream_id: "eth_5m", params: { symbol: "ETHUSDT", interval: "5m", limit: 5 } }
        ],
        plugin_params: { analysis_mode: "deep" },
        plugin_outputs: ["ms_signals_btc", "ms_signals_eth"]
      },
      {
        plugin_name: "plugin_breakout",
        plugin_inputs: [
          { source: "plugin_ms_analyzer", stream_id: "ms_signals_btc", params: {} }
        ],
        plugin_params: { threshold: 0.02 },
        plugin_outputs: ["breakout_signals"]
      }
    ],
    hft_pipeline: [
      {
        plugin_name: "plugin_binance_gateway",
        plugin_inputs: [],
        plugin_params: { symbols: ["BTCUSDT"] },
        plugin_outputs: ["stream_aggtrades"]
      },
      {
        plugin_name: "plugin_breakout",
        plugin_inputs: [{ source: "plugin_binance_gateway", stream_id: "stream_aggtrades", params: {} }],
        plugin_params: { threshold: 0.01 },
        plugin_outputs: ["signals"]
      }
    ],
    empty_array: []
  };

  // Initialize
  async function init() {
    setupResizer();
    setupViewSwitching();
    setupCodeEditorEvents();
    setupActionEvents();
    checkServerStatus();
    await loadWorkspaceConfig();
  }

  async function checkServerStatus() {
    try {
      const res = await fetch('/api/status');
      if (res.ok) {
        const data = await res.json();
        updateServerStatusBadge(true, `Sunucu Bağlı (${data.file_exists ? 'flow_config.json Aktif' : 'Boş'})`);
      } else {
        updateServerStatusBadge(false, 'Sunucu Yanıt Vermiyor');
      }
    } catch (e) {
      updateServerStatusBadge(false, 'Çevrimdışı (Yerel Mod)');
    }
  }

  function updateServerStatusBadge(online, text) {
    if (!el.serverStatusBadge) return;
    if (online) {
      el.serverStatusBadge.className = 'sub-badge success';
      el.serverStatusBadge.innerHTML = `<i class="fa-solid fa-circle-check"></i> ${text}`;
    } else {
      el.serverStatusBadge.className = 'sub-badge warning';
      el.serverStatusBadge.innerHTML = `<i class="fa-solid fa-triangle-exclamation"></i> ${text}`;
    }
  }

  async function loadWorkspaceConfig() {
    try {
      const res = await fetch('/api/config');
      if (res.ok) {
        const result = await res.json();
        const data = Array.isArray(result) ? result : (result.data || result);
        updateDataStore(data, false);
        showToast('flow_config.json yüklendi.', 'info');
        return;
      }
    } catch (e) {
      console.log('Sunucu ayarları okunamadı, varsayılan şablon yükleniyor...');
    }
    updateDataStore(TEMPLATES.default_flow, false);
  }

  async function saveWorkspaceConfig() {
    if (!state.isValidJson) {
      showToast('Geçersiz JSON! Lütfen hataları düzeltip tekrar deneyin.', 'error');
      return;
    }
    try {
      const payload = JSON.parse(state.rawText);
      const res = await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload)
      });
      const result = await res.json();
      if (res.ok && result.status === 'ok') {
        showToast('flow_config.json başarıyla kaydedildi!', 'success');
        updateServerStatusBadge(true, 'flow_config.json Aktif & Kaydedildi');
      } else {
        showToast('Kaydetme hatası!', 'error');
      }
    } catch (e) {
      showToast('Sunucuya kaydedilemedi: ' + e.message, 'error');
    }
  }

  function updateDataStore(newData, pushHistory = true) {
    state.data = newData;
    state.rawText = JSON.stringify(newData, null, 2);
    state.isValidJson = true;

    if (el.rawJsonTextarea) el.rawJsonTextarea.value = state.rawText;
    updateLineNumbers();
    updateValidationStatus(true);
    renderViews();
    updateStatsInfo();

    if (pushHistory) pushToHistory();
  }

  function pushToHistory() {
    state.history = state.history.slice(0, state.historyIndex + 1);
    state.history.push(JSON.stringify(state.data));
    state.historyIndex = state.history.length - 1;
    updateUndoRedoButtons();
  }

  function undo() {
    if (state.historyIndex > 0) {
      state.historyIndex--;
      const data = JSON.parse(state.history[state.historyIndex]);
      updateDataStore(data, false);
      updateUndoRedoButtons();
    }
  }

  function redo() {
    if (state.historyIndex < state.history.length - 1) {
      state.historyIndex++;
      const data = JSON.parse(state.history[state.historyIndex]);
      updateDataStore(data, false);
      updateUndoRedoButtons();
    }
  }

  function updateUndoRedoButtons() {
    if (el.btnUndo) el.btnUndo.disabled = state.historyIndex <= 0;
    if (el.btnRedo) el.btnRedo.disabled = state.historyIndex >= state.history.length - 1;
  }

  function renderViews() {
    renderTreeView();
    renderFlowCanvas();
  }

  function renderTreeView() {
    if (!el.treeContentRoot) return;
    el.treeContentRoot.innerHTML = '';
    if (state.data === null || state.data === undefined) return;
    const treeNode = createTreeNode(state.data, 'root', []);
    el.treeContentRoot.appendChild(treeNode);
  }

  function createTreeNode(value, key, path) {
    const nodeEl = document.createElement('div');
    nodeEl.className = 'tree-node';
    const type = getValueType(value);
    const pathStr = path.join('.');
    const isCollapsed = state.collapsedPaths.has(pathStr);

    const header = document.createElement('div');
    header.className = 'node-header';

    if (type === 'object' || type === 'array') {
      const toggleBtn = document.createElement('span');
      toggleBtn.className = `toggle-icon ${isCollapsed ? 'collapsed' : ''}`;
      toggleBtn.innerHTML = `<i class="fa-solid fa-chevron-down"></i>`;
      toggleBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (state.collapsedPaths.has(pathStr)) state.collapsedPaths.delete(pathStr);
        else state.collapsedPaths.add(pathStr);
        renderTreeView();
      });
      header.appendChild(toggleBtn);
    }

    const keySpan = document.createElement('span');
    keySpan.className = 'node-key';
    keySpan.textContent = key + ':';
    header.appendChild(keySpan);

    const typeBadge = document.createElement('span');
    typeBadge.className = `type-badge type-${type}`;
    typeBadge.textContent = type;
    header.appendChild(typeBadge);

    if (type !== 'object' && type !== 'array') {
      const valInput = document.createElement('input');
      valInput.className = 'node-value-input';
      valInput.value = value;
      valInput.addEventListener('change', (e) => {
        setPathValue(state.data, path, parseInputValue(e.target.value, type));
        updateDataStore(state.data);
      });
      header.appendChild(valInput);
    }

    nodeEl.appendChild(header);

    if ((type === 'object' || type === 'array') && !isCollapsed) {
      const childrenCont = document.createElement('div');
      childrenCont.className = 'node-children';
      const keys = Object.keys(value);
      keys.forEach(k => {
        childrenCont.appendChild(createTreeNode(value[k], k, [...path, k]));
      });
      nodeEl.appendChild(childrenCont);
    }

    return nodeEl;
  }

  function renderFlowCanvas() {
    if (!el.flowNodesLayer || !Array.isArray(state.data)) return;
    el.flowNodesLayer.innerHTML = '';
    state.data.forEach((plugin, idx) => {
      const node = document.createElement('div');
      node.className = 'flow-node';
      node.style.top = `${60 + idx * 100}px`;
      node.style.left = `${50 + (idx % 2) * 280}px`;
      node.innerHTML = `
        <div class="node-header"><i class="fa-solid fa-cube"></i> ${plugin.plugin_name || 'Eklenti'}</div>
        <div class="node-body">
          <div class="node-io">Girdiler: ${(plugin.plugin_inputs || []).length} | Çıktılar: ${(plugin.plugin_outputs || []).length}</div>
        </div>`;
      el.flowNodesLayer.appendChild(node);
    });
  }

  function getValueType(val) {
    if (val === null) return 'null';
    if (Array.isArray(val)) return 'array';
    return typeof val;
  }

  function parseInputValue(str, type) {
    if (type === 'number') return Number(str);
    if (type === 'boolean') return str === 'true';
    return str;
  }

  function setPathValue(obj, path, val) {
    let current = obj;
    for (let i = 0; i < path.length - 1; i++) {
      current = current[path[i]];
    }
    current[path[path.length - 1]] = val;
  }

  function updateLineNumbers() {
    if (!el.lineNumbers || !el.rawJsonTextarea) return;
    const lines = el.rawJsonTextarea.value.split('\n').length;
    el.lineNumbers.innerHTML = Array.from({ length: lines }, (_, i) => i + 1).join('<br>');
  }

  function updateValidationStatus(valid) {
    state.isValidJson = valid;
    if (!el.codeValidStatus) return;
    if (valid) {
      el.codeValidStatus.className = 'valid-indicator success';
      el.codeValidStatus.innerHTML = `<i class="fa-solid fa-circle-check"></i> Geçerli JSON`;
    } else {
      el.codeValidStatus.className = 'valid-indicator error';
      el.codeValidStatus.innerHTML = `<i class="fa-solid fa-circle-xmark"></i> Hatalı JSON`;
    }
  }

  function updateStatsInfo() {
    if (!el.statsInfo) return;
    const count = Array.isArray(state.data) ? state.data.length : 0;
    el.statsInfo.innerHTML = `<i class="fa-solid fa-cubes"></i> ${count} Eklenti / Düğüm`;
  }

  function setupResizer() {
    const resizer = document.getElementById('pane-resizer');
    const leftPane = document.getElementById('pane-code');
    if (!resizer || !leftPane) return;
    let isResizing = false;
    resizer.addEventListener('mousedown', () => isResizing = true);
    window.addEventListener('mousemove', (e) => {
      if (!isResizing) return;
      const width = (e.clientX / window.innerWidth) * 100;
      if (width > 15 && width < 85) leftPane.style.width = `${width}%`;
    });
    window.addEventListener('mouseup', () => isResizing = false);
  }

  function setupViewSwitching() {
    const btns = [
      { b: el.btnViewSplit, mode: 'split' },
      { b: el.btnViewTree, mode: 'tree' },
      { b: el.btnViewFlow, mode: 'flow' },
      { b: el.btnViewCode, mode: 'code' }
    ];
    btns.forEach(item => {
      if (!item.b) return;
      item.b.addEventListener('click', () => {
        btns.forEach(x => x.b && x.b.classList.remove('active'));
        item.b.classList.add('active');
        state.viewMode = item.mode;
        if (el.mainWorkspace) el.mainWorkspace.className = `main-workspace ${item.mode}-mode`;
      });
    });
  }

  function setupCodeEditorEvents() {
    if (!el.rawJsonTextarea) return;
    el.rawJsonTextarea.addEventListener('input', () => {
      state.rawText = el.rawJsonTextarea.value;
      updateLineNumbers();
      try {
        const parsed = JSON.parse(state.rawText);
        state.data = parsed;
        updateValidationStatus(true);
        renderViews();
        updateStatsInfo();
      } catch (e) {
        updateValidationStatus(false);
      }
    });
  }

  function setupActionEvents() {
    if (el.btnFormat) el.btnFormat.addEventListener('click', () => updateDataStore(state.data));
    if (el.btnReload) el.btnReload.addEventListener('click', loadWorkspaceConfig);
    if (el.btnSaveWorkspace) el.btnSaveWorkspace.addEventListener('click', saveWorkspaceConfig);
    if (el.btnUndo) el.btnUndo.addEventListener('click', undo);
    if (el.btnRedo) el.btnRedo.addEventListener('click', redo);

    if (el.templatesMenu) {
      el.templatesMenu.addEventListener('click', (e) => {
        const item = e.target.closest('[data-template]');
        if (item) {
          const key = item.dataset.template;
          if (TEMPLATES[key]) {
            updateDataStore(TEMPLATES[key]);
            showToast(`'${key}' şablonu yüklendi.`, 'info');
          }
        }
      });
    }

    if (el.btnExpandAll) el.btnExpandAll.addEventListener('click', () => { state.collapsedPaths.clear(); renderTreeView(); });
    if (el.btnCollapseAll) el.btnCollapseAll.addEventListener('click', () => { state.collapsedPaths.add('root'); renderTreeView(); });

    window.addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
        e.preventDefault();
        saveWorkspaceConfig();
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
