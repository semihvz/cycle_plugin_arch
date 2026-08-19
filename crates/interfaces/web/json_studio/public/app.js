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
    canvasZoom: 1.0,
    connectingState: null, // { sourcePlugin, streamId, x1, y1 }
    inspectorPluginIdx: null,
    activeAddTarget: null // { parentData, key, isArray }
  };

  // DOM Elements
  const el = {
    serverStatusBadge: document.getElementById('server-status-badge'),
    btnViewSplit: document.getElementById('btn-view-split'),
    btnViewCards: document.getElementById('btn-view-cards'),
    btnViewTree: document.getElementById('btn-view-tree'),
    btnViewFlow: document.getElementById('btn-view-flow'),
    btnViewCode: document.getElementById('btn-view-code'),
    mainWorkspace: document.getElementById('main-workspace'),
    paneCode: document.getElementById('pane-code'),
    paneVisual: document.getElementById('pane-visual'),
    viewCardsContainer: document.getElementById('view-cards-container'),
    cardsGridRoot: document.getElementById('cards-grid-root'),
    btnAddCardPlugin: document.getElementById('btn-add-card-plugin'),
    viewTreeContainer: document.getElementById('view-tree-container'),
    viewFlowContainer: document.getElementById('view-flow-container'),
    rawJsonTextarea: document.getElementById('raw-json-textarea'),
    lineNumbers: document.getElementById('line-numbers'),
    codeValidStatus: document.getElementById('code-valid-status'),
    treeContentRoot: document.getElementById('tree-content-root'),
    flowCanvasWrapper: document.getElementById('flow-canvas-wrapper'),
    flowNodesLayer: document.getElementById('flow-nodes-layer'),
    flowSvgLayer: document.getElementById('flow-svg-layer'),
    btnZoomIn: document.getElementById('btn-zoom-in'),
    btnZoomOut: document.getElementById('btn-zoom-out'),
    btnZoomReset: document.getElementById('btn-zoom-reset'),
    btnAddFlowPlugin: document.getElementById('btn-add-flow-plugin'),
    metricActiveStreams: document.getElementById('metric-active-streams'),
    metricProducers: document.getElementById('metric-producers'),
    metricConsumers: document.getElementById('metric-consumers'),
    zoomLevelIndicator: document.getElementById('zoom-level-indicator'),
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
    renderCardsView();
    renderTreeView();
    renderFlowCanvas();
  }

  function renderCardsView() {
    if (!el.cardsGridRoot) return;
    el.cardsGridRoot.innerHTML = '';

    if (!Array.isArray(state.data)) {
      const msg = document.createElement('div');
      msg.className = 'empty-state-notice';
      msg.style.padding = '30px';
      msg.style.color = 'var(--text-muted)';
      msg.innerHTML = `<i class="fa-solid fa-circle-exclamation"></i> Kök öğe bir eklenti dizisi ([]) değil. Kartsal görünüm eklenti listesi için düzenlenmiştir.`;
      el.cardsGridRoot.appendChild(msg);
      return;
    }

    const query = (state.searchQuery || '').toLowerCase().trim();

    state.data.forEach((plugin, pIdx) => {
      const pluginName = plugin.plugin_name || `plugin_${pIdx + 1}`;

      if (query) {
        const fullStr = JSON.stringify(plugin).toLowerCase();
        if (!fullStr.includes(query)) return;
      }

      const isEnabled = plugin.enabled !== false;

      const card = document.createElement('div');
      card.className = `plugin-card ${isEnabled ? 'card-enabled' : 'card-disabled'}`;

      // Card Header
      const header = document.createElement('div');
      header.className = 'card-header';
      header.innerHTML = `
        <div class="card-title-group">
          <span class="card-drag-handle"><i class="fa-solid fa-grip-vertical"></i></span>
          <i class="fa-solid fa-cube plugin-icon"></i>
          <input type="text" class="card-name-input" value="${escapeHtml(pluginName)}" placeholder="Eklenti Adı...">
        </div>
        <div class="card-header-actions">
          <label class="switch-toggle" title="Eklentiyi Başlangıçta Çalıştır (enabled)">
            <input type="checkbox" class="toggle-enabled" ${isEnabled ? 'checked' : ''}>
            <span class="switch-slider"></span>
            <span class="toggle-label">${isEnabled ? 'AKTİF' : 'PASİF'}</span>
          </label>
          <button class="btn-card-action btn-move-up" title="Yukarı Taşı" ${pIdx === 0 ? 'disabled' : ''}><i class="fa-solid fa-arrow-up"></i></button>
          <button class="btn-card-action btn-move-down" title="Aşağı Taşı" ${pIdx === state.data.length - 1 ? 'disabled' : ''}><i class="fa-solid fa-arrow-down"></i></button>
          <button class="btn-card-action btn-duplicate" title="Kopyala"><i class="fa-solid fa-copy"></i></button>
          <button class="btn-card-action btn-delete danger" title="Sil"><i class="fa-solid fa-trash-can"></i></button>
        </div>
      `;

      const nameInput = header.querySelector('.card-name-input');
      nameInput.addEventListener('change', (e) => {
        plugin.plugin_name = e.target.value.trim();
        updateDataStore(state.data);
      });

      const enabledToggle = header.querySelector('.toggle-enabled');
      enabledToggle.addEventListener('change', (e) => {
        plugin.enabled = e.target.checked;
        updateDataStore(state.data);
      });

      header.querySelector('.btn-move-up').addEventListener('click', () => {
        if (pIdx > 0) {
          const temp = state.data[pIdx];
          state.data[pIdx] = state.data[pIdx - 1];
          state.data[pIdx - 1] = temp;
          updateDataStore(state.data);
        }
      });

      header.querySelector('.btn-move-down').addEventListener('click', () => {
        if (pIdx < state.data.length - 1) {
          const temp = state.data[pIdx];
          state.data[pIdx] = state.data[pIdx + 1];
          state.data[pIdx + 1] = temp;
          updateDataStore(state.data);
        }
      });

      header.querySelector('.btn-duplicate').addEventListener('click', () => {
        const clone = JSON.parse(JSON.stringify(plugin));
        clone.plugin_name = (clone.plugin_name || 'plugin') + '_copy';
        state.data.splice(pIdx + 1, 0, clone);
        updateDataStore(state.data);
        showToast(`'${pluginName}' kopyalandı.`, 'info');
      });

      header.querySelector('.btn-delete').addEventListener('click', () => {
        if (confirm(`'${pluginName}' eklentisini silmek istediğinizden emin misiniz?`)) {
          state.data.splice(pIdx, 1);
          updateDataStore(state.data);
          showToast(`'${pluginName}' silindi.`, 'warning');
        }
      });

      card.appendChild(header);

      // Card Body
      const body = document.createElement('div');
      body.className = 'card-body';

      // --- SECTION 1: INPUTS ---
      const inputsSec = document.createElement('div');
      inputsSec.className = 'card-section';
      const inputsList = Array.isArray(plugin.plugin_inputs) ? plugin.plugin_inputs : [];
      inputsSec.innerHTML = `
        <div class="section-title">
          <span><i class="fa-solid fa-right-to-bracket"></i> Giriş Akışları (plugin_inputs)</span>
          <button class="btn-micro btn-add-input"><i class="fa-solid fa-plus"></i> Giriş Ekle</button>
        </div>
        <div class="chips-container inputs-chips"></div>
      `;

      const inputsChipsContainer = inputsSec.querySelector('.inputs-chips');
      if (inputsList.length === 0) {
        inputsChipsContainer.innerHTML = `<span class="empty-chip">Giriş akışı yok (Üretici Eklenti)</span>`;
      } else {
        inputsList.forEach((inp, inpIdx) => {
          const chip = document.createElement('div');
          chip.className = 'chip chip-input';
          chip.innerHTML = `
            <span class="chip-label" title="Kaynak: ${escapeHtml(inp.source || '')}">${escapeHtml(inp.source || '?')} ➔ <strong>${escapeHtml(inp.stream_id || '')}</strong></span>
            <button class="chip-remove" title="Girişi Sil"><i class="fa-solid fa-xmark"></i></button>
          `;
          chip.querySelector('.chip-remove').addEventListener('click', () => {
            plugin.plugin_inputs.splice(inpIdx, 1);
            updateDataStore(state.data);
          });
          inputsChipsContainer.appendChild(chip);
        });
      }

      inputsSec.querySelector('.btn-add-input').addEventListener('click', () => {
        const source = prompt('Kaynak Eklenti Adı (source):', 'plugin_binance_gateway');
        if (!source) return;
        const streamId = prompt('Akış Kimliği (stream_id):', 'stream_bestprice');
        if (!streamId) return;

        if (!Array.isArray(plugin.plugin_inputs)) plugin.plugin_inputs = [];
        plugin.plugin_inputs.push({ source, stream_id: streamId, params: {} });
        updateDataStore(state.data);
      });

      body.appendChild(inputsSec);

      // --- SECTION 2: OUTPUTS ---
      const outputsSec = document.createElement('div');
      outputsSec.className = 'card-section';
      const outputsList = Array.isArray(plugin.plugin_outputs) ? plugin.plugin_outputs : [];
      outputsSec.innerHTML = `
        <div class="section-title">
          <span><i class="fa-solid fa-right-from-bracket"></i> Çıkış Akışları (plugin_outputs)</span>
        </div>
        <div class="chips-container outputs-chips"></div>
        <div class="add-output-row">
          <input type="text" class="input-new-output" placeholder="+ Akış adı yazıp Enter'a basın...">
          <button class="btn-micro btn-add-output"><i class="fa-solid fa-plus"></i> Ekle</button>
        </div>
      `;

      const outputsChipsContainer = outputsSec.querySelector('.outputs-chips');
      if (outputsList.length === 0) {
        outputsChipsContainer.innerHTML = `<span class="empty-chip">Çıkış akışı yok (Tüketici / İşleyici)</span>`;
      } else {
        outputsList.forEach((outStream, outIdx) => {
          const chip = document.createElement('div');
          chip.className = 'chip chip-output';
          chip.innerHTML = `
            <span class="chip-label">📡 ${escapeHtml(outStream)}</span>
            <button class="chip-remove" title="Çıkışı Sil"><i class="fa-solid fa-xmark"></i></button>
          `;
          chip.querySelector('.chip-remove').addEventListener('click', () => {
            plugin.plugin_outputs.splice(outIdx, 1);
            updateDataStore(state.data);
          });
          outputsChipsContainer.appendChild(chip);
        });
      }

      const newOutInput = outputsSec.querySelector('.input-new-output');
      const addOutBtn = outputsSec.querySelector('.btn-add-output');

      const handleAddOutput = () => {
        const val = newOutInput.value.trim();
        if (val) {
          if (!Array.isArray(plugin.plugin_outputs)) plugin.plugin_outputs = [];
          if (!plugin.plugin_outputs.includes(val)) {
            plugin.plugin_outputs.push(val);
            updateDataStore(state.data);
          }
        }
      };

      addOutBtn.addEventListener('click', handleAddOutput);
      newOutInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') handleAddOutput();
      });

      body.appendChild(outputsSec);

      // --- SECTION 3: PARAMETERS ---
      const paramsSec = document.createElement('div');
      paramsSec.className = 'card-section';
      paramsSec.innerHTML = `
        <div class="section-title">
          <span><i class="fa-solid fa-gears"></i> Eklenti Parametreleri (plugin_params)</span>
          <button class="btn-micro btn-add-param"><i class="fa-solid fa-plus"></i> Parametre Ekle</button>
        </div>
        <div class="params-form-grid"></div>
      `;

      const paramsGrid = paramsSec.querySelector('.params-form-grid');
      const paramsObj = plugin.plugin_params || {};

      const keys = Object.keys(paramsObj);
      if (keys.length === 0) {
        paramsGrid.innerHTML = `<span class="empty-chip">Özel parametre yok</span>`;
      } else {
        keys.forEach(pKey => {
          const pVal = paramsObj[pKey];
          const row = document.createElement('div');
          row.className = 'param-row';

          const label = document.createElement('label');
          label.className = 'param-label';
          label.textContent = pKey + ':';
          row.appendChild(label);

          const fieldCont = document.createElement('div');
          fieldCont.className = 'param-field';

          if (typeof pVal === 'boolean') {
            const toggle = document.createElement('label');
            toggle.className = 'switch-toggle mini';
            toggle.innerHTML = `
              <input type="checkbox" ${pVal ? 'checked' : ''}>
              <span class="switch-slider"></span>
            `;
            toggle.querySelector('input').addEventListener('change', (e) => {
              plugin.plugin_params[pKey] = e.target.checked;
              updateDataStore(state.data);
            });
            fieldCont.appendChild(toggle);
          } else if (Array.isArray(pVal)) {
            const arrTags = document.createElement('div');
            arrTags.className = 'array-tags-container';
            pVal.forEach((item, itemIdx) => {
              const tag = document.createElement('span');
              tag.className = 'array-tag';
              tag.innerHTML = `${escapeHtml(String(item))} <i class="fa-solid fa-xmark del-item"></i>`;
              tag.querySelector('.del-item').addEventListener('click', () => {
                pVal.splice(itemIdx, 1);
                updateDataStore(state.data);
              });
              arrTags.appendChild(tag);
            });

            const addTagInput = document.createElement('input');
            addTagInput.type = 'text';
            addTagInput.className = 'input-add-tag';
            addTagInput.placeholder = '+ Ekle';
            addTagInput.addEventListener('keydown', (e) => {
              if (e.key === 'Enter' && addTagInput.value.trim()) {
                pVal.push(addTagInput.value.trim().toUpperCase());
                updateDataStore(state.data);
              }
            });
            arrTags.appendChild(addTagInput);
            fieldCont.appendChild(arrTags);
          } else if (typeof pVal === 'number') {
            const numInput = document.createElement('input');
            numInput.type = 'number';
            numInput.className = 'param-input-num';
            numInput.value = pVal;
            numInput.addEventListener('change', (e) => {
              plugin.plugin_params[pKey] = Number(e.target.value);
              updateDataStore(state.data);
            });
            fieldCont.appendChild(numInput);
          } else {
            const txtInput = document.createElement('input');
            txtInput.type = 'text';
            txtInput.className = 'param-input-txt';
            txtInput.value = typeof pVal === 'object' ? JSON.stringify(pVal) : String(pVal);
            txtInput.addEventListener('change', (e) => {
              try {
                plugin.plugin_params[pKey] = JSON.parse(e.target.value);
              } catch (_) {
                plugin.plugin_params[pKey] = e.target.value;
              }
              updateDataStore(state.data);
            });
            fieldCont.appendChild(txtInput);
          }

          const delParamBtn = document.createElement('button');
          delParamBtn.className = 'btn-del-param';
          delParamBtn.title = 'Parametreyi Sil';
          delParamBtn.innerHTML = `<i class="fa-solid fa-minus"></i>`;
          delParamBtn.addEventListener('click', () => {
            delete plugin.plugin_params[pKey];
            updateDataStore(state.data);
          });

          row.appendChild(fieldCont);
          row.appendChild(delParamBtn);
          paramsGrid.appendChild(row);
        });
      }

      paramsSec.querySelector('.btn-add-param').addEventListener('click', () => {
        const key = prompt('Yeni Parametre Adı (key):');
        if (!key) return;
        const val = prompt(`'${key}' Değeri:`, 'true');
        if (val === null) return;

        if (!plugin.plugin_params) plugin.plugin_params = {};
        let parsedVal = val;
        if (val === 'true') parsedVal = true;
        else if (val === 'false') parsedVal = false;
        else if (!isNaN(val) && val.trim() !== '') parsedVal = Number(val);

        plugin.plugin_params[key] = parsedVal;
        updateDataStore(state.data);
      });

      body.appendChild(paramsSec);
      card.appendChild(body);

      el.cardsGridRoot.appendChild(card);
    });
  }

  function escapeHtml(str) {
    return String(str || '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
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

  function autoLayoutGraph() {
    if (!Array.isArray(state.data)) return;
    const cols = [];
    const visited = new Set();

    // Column 0: Producers (no inputs)
    const col0 = [];
    state.data.forEach((p, idx) => {
      if (!p.plugin_inputs || p.plugin_inputs.length === 0) {
        col0.push(idx);
        visited.add(p.plugin_name);
      }
    });

    cols.push(col0.length > 0 ? col0 : [0]);

    // Subsequent Columns
    let attempts = 0;
    while (visited.size < state.data.length && attempts < 10) {
      attempts++;
      const currentCol = [];
      state.data.forEach((p, idx) => {
        if (!visited.has(p.plugin_name)) {
          const deps = (p.plugin_inputs || []).map(i => i.source);
          if (deps.every(d => visited.has(d))) {
            currentCol.push(idx);
          }
        }
      });

      if (currentCol.length === 0) {
        state.data.forEach((p, idx) => {
          if (!visited.has(p.plugin_name)) currentCol.push(idx);
        });
      }

      currentCol.forEach(idx => visited.add(state.data[idx].plugin_name));
      cols.push(currentCol);
    }

    cols.forEach((colPlugins, colIdx) => {
      colPlugins.forEach((pIdx, rowIdx) => {
        const p = state.data[pIdx];
        if (p) {
          state.flowNodePositions.set(p.plugin_name, {
            x: 40 + colIdx * 380,
            y: 40 + rowIdx * 240
          });
        }
      });
    });

    renderFlowCanvas();
  }

  function renderFlowCanvas() {
    if (!el.flowNodesLayer || !Array.isArray(state.data)) return;
    el.flowNodesLayer.innerHTML = '';
    if (el.flowSvgLayer) {
      const defs = el.flowSvgLayer.querySelector('defs');
      el.flowSvgLayer.innerHTML = '';
      if (defs) el.flowSvgLayer.appendChild(defs);
    }

    if (state.flowNodePositions.size === 0) {
      autoLayoutGraph();
      return;
    }

    state.data.forEach((plugin, idx) => {
      const pluginName = plugin.plugin_name || `plugin_${idx + 1}`;
      const isEnabled = plugin.enabled !== false;
      const pos = state.flowNodePositions.get(pluginName) || {
        x: 40 + (idx % 3) * 380,
        y: 40 + Math.floor(idx / 3) * 240
      };

      const card = document.createElement('div');
      card.className = `er-table-card ${isEnabled ? 'enabled' : 'disabled'}`;
      card.style.left = `${pos.x}px`;
      card.style.top = `${pos.y}px`;
      card.dataset.pluginName = pluginName;
      card.dataset.pluginIdx = idx;

      // Table Card Header
      const header = document.createElement('div');
      header.className = 'er-card-header';
      header.innerHTML = `
        <div class="er-title-group">
          <i class="fa-solid fa-table-list er-table-icon"></i>
          <input type="text" class="er-name-input" value="${escapeHtml(pluginName)}">
        </div>
        <div class="er-actions">
          <label class="switch-toggle mini" title="Başlangıç Durumu">
            <input type="checkbox" class="toggle-enabled" ${isEnabled ? 'checked' : ''}>
            <span class="switch-slider"></span>
          </label>
          <button class="btn-micro-action btn-duplicate" title="Kopyala"><i class="fa-solid fa-copy"></i></button>
          <button class="btn-micro-action btn-delete danger" title="Sil"><i class="fa-solid fa-trash-can"></i></button>
        </div>
      `;

      // Header Dragging Event
      let isDragging = false;
      let startX, startY, initialX, initialY;

      header.addEventListener('mousedown', (e) => {
        if (e.target.tagName === 'INPUT' || e.target.closest('.switch-toggle') || e.target.closest('button')) return;
        isDragging = true;
        startX = e.clientX;
        startY = e.clientY;
        initialX = pos.x;
        initialY = pos.y;
        card.style.zIndex = '1000';
        e.preventDefault();
      });

      window.addEventListener('mousemove', (e) => {
        if (!isDragging) return;
        const dx = e.clientX - startX;
        const dy = e.clientY - startY;
        pos.x = Math.max(10, initialX + dx);
        pos.y = Math.max(10, initialY + dy);
        card.style.left = `${pos.x}px`;
        card.style.top = `${pos.y}px`;
        state.flowNodePositions.set(pluginName, pos);
        drawConnectingWires();
      });

      window.addEventListener('mouseup', () => {
        if (isDragging) {
          isDragging = false;
          card.style.zIndex = '10';
        }
      });

      // Name Input
      const nameInput = header.querySelector('.er-name-input');
      nameInput.addEventListener('change', (e) => {
        const oldName = plugin.plugin_name;
        const newName = e.target.value.trim();
        if (newName && newName !== oldName) {
          plugin.plugin_name = newName;
          state.flowNodePositions.delete(oldName);
          state.flowNodePositions.set(newName, pos);
          updateDataStore(state.data);
        }
      });

      // Enabled Switch
      header.querySelector('.toggle-enabled').addEventListener('change', (e) => {
        plugin.enabled = e.target.checked;
        updateDataStore(state.data);
      });

      // Duplicate & Delete
      header.querySelector('.btn-duplicate').addEventListener('click', () => {
        const clone = JSON.parse(JSON.stringify(plugin));
        clone.plugin_name = (clone.plugin_name || 'plugin') + '_copy';
        state.data.splice(idx + 1, 0, clone);
        updateDataStore(state.data);
      });

      header.querySelector('.btn-delete').addEventListener('click', () => {
        if (confirm(`'${pluginName}' tablosunu silmek istiyor musunuz?`)) {
          state.data.splice(idx, 1);
          state.flowNodePositions.delete(pluginName);
          updateDataStore(state.data);
        }
      });

      card.appendChild(header);

      // --- PORTS ROW: INPUT PORTS (LEFT) & OUTPUT PORTS (RIGHT) ---
      const portsContainer = document.createElement('div');
      portsContainer.className = 'er-ports-container';

      // Inputs Column (Left Ports)
      const inputsCol = document.createElement('div');
      inputsCol.className = 'er-ports-col inputs-col';
      inputsCol.innerHTML = `<div class="col-header"><i class="fa-solid fa-right-to-bracket"></i> Girdiler</div>`;

      const inputsList = Array.isArray(plugin.plugin_inputs) ? plugin.plugin_inputs : [];
      inputsList.forEach((inp, inpIdx) => {
        const portRow = document.createElement('div');
        portRow.className = 'port-row in-port-row';
        portRow.innerHTML = `
          <span class="port-bullet in-bullet" data-plugin="${escapeHtml(pluginName)}" data-stream="${escapeHtml(inp.stream_id)}" id="port-in-${escapeHtml(pluginName)}-${escapeHtml(inp.stream_id)}"></span>
          <span class="port-label" title="Kaynak: ${escapeHtml(inp.source)}">${escapeHtml(inp.source)} ➔ <strong>${escapeHtml(inp.stream_id)}</strong></span>
          <i class="fa-solid fa-xmark del-port" title="Bağlantıyı Kopar"></i>
        `;
        portRow.querySelector('.del-port').addEventListener('click', () => {
          plugin.plugin_inputs.splice(inpIdx, 1);
          updateDataStore(state.data);
        });
        inputsCol.appendChild(portRow);
      });

      const addInBtn = document.createElement('button');
      addInBtn.className = 'btn-add-port';
      addInBtn.innerHTML = `<i class="fa-solid fa-plus"></i> Giriş Portu`;
      addInBtn.addEventListener('click', () => {
        const availableStreams = [];
        state.data.forEach(p => {
          if (p.plugin_name !== pluginName && Array.isArray(p.plugin_outputs)) {
            p.plugin_outputs.forEach(stream => {
              availableStreams.push({ source: p.plugin_name, stream_id: stream });
            });
          }
        });

        if (availableStreams.length === 0) {
          showToast('Sistemde bağlanacak yayınlanan çıktı akışı bulunamadı.', 'warning');
          return;
        }

        const optionsText = availableStreams.map((s, i) => `${i + 1}. Kaynak: ${s.source} ➔ Akış: ${s.stream_id}`).join('\n');
        const choice = prompt(`Bağlanacak akışın numarasını girin:\n\n${optionsText}`);
        if (!choice) return;
        const selectedIdx = parseInt(choice, 10) - 1;

        if (!isNaN(selectedIdx) && availableStreams[selectedIdx]) {
          const sel = availableStreams[selectedIdx];
          if (!Array.isArray(plugin.plugin_inputs)) plugin.plugin_inputs = [];
          plugin.plugin_inputs.push({ source: sel.source, stream_id: sel.stream_id, params: {} });
          updateDataStore(state.data);
          showToast(`⚡ '${sel.stream_id}' akışı bağlandı!`, 'success');
        }
      });
      inputsCol.appendChild(addInBtn);
      portsContainer.appendChild(inputsCol);

      // Outputs Column (Right Ports)
      const outputsCol = document.createElement('div');
      outputsCol.className = 'er-ports-col outputs-col';
      outputsCol.innerHTML = `<div class="col-header"><i class="fa-solid fa-right-from-bracket"></i> Çıktılar</div>`;

      const outputsList = Array.isArray(plugin.plugin_outputs) ? plugin.plugin_outputs : [];
      outputsList.forEach((outStream) => {
        const portRow = document.createElement('div');
        portRow.className = 'port-row out-port-row';
        portRow.innerHTML = `
          <span class="port-label">📡 ${escapeHtml(outStream)}</span>
          <i class="fa-solid fa-xmark del-port" title="Çıktıyı Sil"></i>
          <span class="port-bullet out-bullet" data-plugin="${escapeHtml(pluginName)}" data-stream="${escapeHtml(outStream)}" id="port-out-${escapeHtml(pluginName)}-${escapeHtml(outStream)}" title="Sürükleyerek Giriş Portuna Bağlayın"></span>
        `;

        const outBullet = portRow.querySelector('.out-bullet');
        outBullet.addEventListener('mousedown', (e) => {
          e.stopPropagation();
          e.preventDefault();

          const rect = outBullet.getBoundingClientRect();
          const wrapperRect = el.flowCanvasWrapper.getBoundingClientRect();
          const zoom = state.canvasZoom || 1.0;

          const startX = (rect.left + rect.width / 2 - wrapperRect.left + el.flowCanvasWrapper.scrollLeft) / zoom;
          const startY = (rect.top + rect.height / 2 - wrapperRect.top + el.flowCanvasWrapper.scrollTop) / zoom;

          state.connectingState = {
            sourcePlugin: pluginName,
            streamId: outStream,
            startX,
            startY
          };

          document.querySelectorAll('.in-bullet').forEach(b => b.classList.add('pulse-connectable'));
        });

        portRow.querySelector('.del-port').addEventListener('click', () => {
          const outIdx = plugin.plugin_outputs.indexOf(outStream);
          if (outIdx !== -1) plugin.plugin_outputs.splice(outIdx, 1);
          updateDataStore(state.data);
        });
        outputsCol.appendChild(portRow);
      });

      const addOutBtn = document.createElement('button');
      addOutBtn.className = 'btn-add-port';
      addOutBtn.innerHTML = `<i class="fa-solid fa-plus"></i> Çıkış Portu`;
      addOutBtn.addEventListener('click', () => {
        const streamId = prompt('Yeni Çıkış Akışı Adı (stream_id):');
        if (!streamId) return;
        if (!Array.isArray(plugin.plugin_outputs)) plugin.plugin_outputs = [];
        if (!plugin.plugin_outputs.includes(streamId)) {
          plugin.plugin_outputs.push(streamId);
          updateDataStore(state.data);
        }
      });
      outputsCol.appendChild(addOutBtn);
      portsContainer.appendChild(outputsCol);

      card.appendChild(portsContainer);

      // --- DB COLUMNS / PROPERTIES SECTION (plugin_params) ---
      const propsContainer = document.createElement('div');
      propsContainer.className = 'er-props-container';
      propsContainer.innerHTML = `
        <div class="props-header">
          <span><i class="fa-solid fa-list-check"></i> Kart Sütunları & Özellikler</span>
          <button class="btn-add-prop"><i class="fa-solid fa-plus"></i> Özellik Ekle</button>
        </div>
        <div class="props-list"></div>
      `;

      const propsList = propsContainer.querySelector('.props-list');
      const paramsObj = plugin.plugin_params || {};
      const paramKeys = Object.keys(paramsObj);

      if (paramKeys.length === 0) {
        propsList.innerHTML = `<span class="empty-chip">Sütun / Özellik yok</span>`;
      } else {
        paramKeys.forEach(pKey => {
          const pVal = paramsObj[pKey];
          const pType = getValueType(pVal);

          const propRow = document.createElement('div');
          propRow.className = 'prop-row';
          propRow.innerHTML = `
            <span class="prop-name">🔑 ${escapeHtml(pKey)}</span>
            <span class="prop-type type-${pType}">${pType}</span>
            <div class="prop-val-wrap"></div>
            <i class="fa-solid fa-xmark del-prop" title="Özelliği Sil"></i>
          `;

          const valWrap = propRow.querySelector('.prop-val-wrap');
          if (typeof pVal === 'boolean') {
            const toggle = document.createElement('label');
            toggle.className = 'switch-toggle mini';
            toggle.innerHTML = `<input type="checkbox" ${pVal ? 'checked' : ''}><span class="switch-slider"></span>`;
            toggle.querySelector('input').addEventListener('change', (e) => {
              plugin.plugin_params[pKey] = e.target.checked;
              updateDataStore(state.data);
            });
            valWrap.appendChild(toggle);
          } else if (Array.isArray(pVal)) {
            const tagsDiv = document.createElement('div');
            tagsDiv.className = 'arr-tags-mini';
            pVal.forEach((v, vIdx) => {
              const tag = document.createElement('span');
              tag.className = 'tag-mini';
              tag.innerHTML = `${escapeHtml(String(v))} <i class="fa-solid fa-xmark"></i>`;
              tag.querySelector('i').addEventListener('click', () => {
                pVal.splice(vIdx, 1);
                updateDataStore(state.data);
              });
              tagsDiv.appendChild(tag);
            });
            valWrap.appendChild(tagsDiv);
          } else {
            const valInput = document.createElement('input');
            valInput.type = typeof pVal === 'number' ? 'number' : 'text';
            valInput.className = 'prop-input';
            valInput.value = typeof pVal === 'object' ? JSON.stringify(pVal) : String(pVal);
            valInput.addEventListener('change', (e) => {
              plugin.plugin_params[pKey] = parseInputValue(e.target.value, pType);
              updateDataStore(state.data);
            });
            valWrap.appendChild(valInput);
          }

          propRow.querySelector('.del-prop').addEventListener('click', () => {
            delete plugin.plugin_params[pKey];
            updateDataStore(state.data);
          });

          propsList.appendChild(propRow);
        });
      }

      propsContainer.querySelector('.btn-add-prop').addEventListener('click', () => {
        const key = prompt('Yeni Özellik / Sütun Adı (key):');
        if (!key) return;
        const val = prompt(`'${key}' Değeri:`, 'true');
        if (val === null) return;

        if (!plugin.plugin_params) plugin.plugin_params = {};
        plugin.plugin_params[key] = parseInputValue(val, isNaN(val) ? (val === 'true' || val === 'false' ? 'boolean' : 'string') : 'number');
        updateDataStore(state.data);
      });

      card.appendChild(propsContainer);

      el.flowNodesLayer.appendChild(card);
    });

    setTimeout(drawConnectingWires, 30);
  }

  function drawConnectingWires() {
    if (!el.flowSvgLayer || !el.flowCanvasWrapper) return;
    const defs = el.flowSvgLayer.querySelector('defs');
    el.flowSvgLayer.innerHTML = '';
    if (defs) el.flowSvgLayer.appendChild(defs);

    if (!Array.isArray(state.data)) return;

    const wrapperRect = el.flowCanvasWrapper.getBoundingClientRect();
    const zoom = state.canvasZoom || 1.0;

    let activeStreamCount = 0;
    let producerCount = 0;
    let consumerCount = 0;

    state.data.forEach(p => {
      const hasInputs = Array.isArray(p.plugin_inputs) && p.plugin_inputs.length > 0;
      const hasOutputs = Array.isArray(p.plugin_outputs) && p.plugin_outputs.length > 0;
      if (hasOutputs && !hasInputs) producerCount++;
      if (hasInputs) consumerCount++;
    });

    state.data.forEach(targetPlugin => {
      const targetName = targetPlugin.plugin_name;
      const inputs = Array.isArray(targetPlugin.plugin_inputs) ? targetPlugin.plugin_inputs : [];

      inputs.forEach((inp, inpIdx) => {
        const sourceName = inp.source;
        const streamId = inp.stream_id;

        const outBullet = document.getElementById(`port-out-${sourceName}-${streamId}`);
        const inBullet = document.getElementById(`port-in-${targetName}-${streamId}`);

        if (outBullet && inBullet) {
          activeStreamCount++;
          const outRect = outBullet.getBoundingClientRect();
          const inRect = inBullet.getBoundingClientRect();

          const x1 = (outRect.left + outRect.width / 2 - wrapperRect.left + el.flowCanvasWrapper.scrollLeft) / zoom;
          const y1 = (outRect.top + outRect.height / 2 - wrapperRect.top + el.flowCanvasWrapper.scrollTop) / zoom;

          const x2 = (inRect.left + inRect.width / 2 - wrapperRect.left + el.flowCanvasWrapper.scrollLeft) / zoom;
          const y2 = (inRect.top + inRect.height / 2 - wrapperRect.top + el.flowCanvasWrapper.scrollTop) / zoom;

          const dx = Math.abs(x2 - x1) * 0.5;
          const pathD = `M ${x1} ${y1} C ${x1 + dx} ${y1}, ${x2 - dx} ${y2}, ${x2} ${y2}`;

          const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
          path.setAttribute('d', pathD);
          path.setAttribute('class', 'flow-wire-line');
          path.setAttribute('marker-end', 'url(#arrow)');
          path.setAttribute('data-stream', streamId);
          path.setAttribute('title', `${sourceName} ➔ ${targetName} (${streamId}) — Tıklayarak Bağlantıyı Kopar`);

          path.addEventListener('click', () => {
            if (confirm(`'${sourceName}' ➔ '${targetName}' (${streamId}) bağlantısını koparmak istiyor musunuz?`)) {
              targetPlugin.plugin_inputs.splice(inpIdx, 1);
              updateDataStore(state.data);
              showToast(`'${streamId}' bağlantısı koparıldı.`, 'info');
            }
          });

          el.flowSvgLayer.appendChild(path);
        }
      });
    });

    if (el.metricActiveStreams) el.metricActiveStreams.textContent = activeStreamCount;
    if (el.metricProducers) el.metricProducers.textContent = producerCount;
    if (el.metricConsumers) el.metricConsumers.textContent = consumerCount;
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
      { b: el.btnViewCards, mode: 'cards' },
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

        if (el.viewCardsContainer) {
          el.viewCardsContainer.classList.toggle('active', item.mode === 'split' || item.mode === 'cards');
        }
        if (el.viewTreeContainer) {
          el.viewTreeContainer.classList.toggle('active', item.mode === 'tree');
        }
        if (el.viewFlowContainer) {
          el.viewFlowContainer.classList.toggle('active', item.mode === 'flow');
        }

        if (item.mode === 'flow') {
          setTimeout(renderFlowCanvas, 50);
        } else if (item.mode === 'cards' || item.mode === 'split') {
          renderCardsView();
        }
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

    if (el.globalSearchInput) {
      el.globalSearchInput.addEventListener('input', () => {
        state.searchQuery = el.globalSearchInput.value;
        renderViews();
      });
    }

    if (el.btnAddCardPlugin) {
      el.btnAddCardPlugin.addEventListener('click', () => {
        if (!Array.isArray(state.data)) state.data = [];
        const newPlugin = {
          plugin_name: "plugin_new_" + (state.data.length + 1),
          enabled: true,
          plugin_inputs: [],
          plugin_params: {},
          plugin_outputs: []
        };
        state.data.push(newPlugin);
        updateDataStore(state.data);
        showToast('Yeni eklenti kartı eklendi!', 'success');
      });
    }

    const updateZoom = (newZoom) => {
      state.canvasZoom = Math.min(2.0, Math.max(0.4, newZoom));
      if (el.flowNodesLayer) el.flowNodesLayer.style.transform = `scale(${state.canvasZoom})`;
      if (el.flowSvgLayer) el.flowSvgLayer.style.transform = `scale(${state.canvasZoom})`;
      if (el.zoomLevelIndicator) el.zoomLevelIndicator.innerHTML = `<i class="fa-solid fa-expand"></i> Yakınlaştırma: %${Math.round(state.canvasZoom * 100)}`;
      setTimeout(drawConnectingWires, 20);
    };

    if (el.btnZoomIn) el.btnZoomIn.addEventListener('click', () => updateZoom(state.canvasZoom + 0.15));
    if (el.btnZoomOut) el.btnZoomOut.addEventListener('click', () => updateZoom(state.canvasZoom - 0.15));
    if (el.btnZoomReset) el.btnZoomReset.addEventListener('click', () => updateZoom(1.0));

    if (el.btnAddFlowPlugin) {
      el.btnAddFlowPlugin.addEventListener('click', () => {
        if (!Array.isArray(state.data)) state.data = [];
        const newPlugin = {
          plugin_name: "plugin_new_" + (state.data.length + 1),
          enabled: true,
          plugin_inputs: [],
          plugin_params: {},
          plugin_outputs: []
        };
        state.data.push(newPlugin);
        updateDataStore(state.data);
        showToast('Tuvale yeni eklenti kartı eklendi!', 'success');
      });
    }

    if (el.flowCanvasWrapper) {
      el.flowCanvasWrapper.addEventListener('wheel', (e) => {
        if (e.ctrlKey || e.metaKey) {
          e.preventDefault();
          updateZoom(state.canvasZoom - e.deltaY * 0.002);
        }
      }, { passive: false });
    }

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

    // Rubber-Band Interactive Drag-to-Connect Wire Handlers
    window.addEventListener('mousemove', (e) => {
      if (!state.connectingState || !el.flowSvgLayer || !el.flowCanvasWrapper) return;

      const wrapperRect = el.flowCanvasWrapper.getBoundingClientRect();
      const zoom = state.canvasZoom || 1.0;

      const curX = (e.clientX - wrapperRect.left + el.flowCanvasWrapper.scrollLeft) / zoom;
      const curY = (e.clientY - wrapperRect.top + el.flowCanvasWrapper.scrollTop) / zoom;

      let tempPath = document.getElementById('temp-drag-wire-path');
      if (!tempPath) {
        tempPath = document.createElementNS('http://www.w3.org/2000/svg', 'path');
        tempPath.id = 'temp-drag-wire-path';
        tempPath.setAttribute('class', 'flow-wire-line temp-drag-wire');
        el.flowSvgLayer.appendChild(tempPath);
      }

      const { startX, startY } = state.connectingState;
      const dx = Math.abs(curX - startX) * 0.5;
      tempPath.setAttribute('d', `M ${startX} ${startY} C ${startX + dx} ${startY}, ${curX - dx} ${curY}, ${curX} ${curY}`);
    });

    window.addEventListener('mouseup', (e) => {
      if (!state.connectingState) return;

      const tempPath = document.getElementById('temp-drag-wire-path');
      if (tempPath) tempPath.remove();
      document.querySelectorAll('.in-bullet').forEach(b => b.classList.remove('pulse-connectable'));

      const targetBullet = e.target.closest('.in-bullet') || e.target.closest('.in-port-row')?.querySelector('.in-bullet');
      if (targetBullet) {
        const targetPluginName = targetBullet.dataset.plugin;
        const { sourcePlugin, streamId } = state.connectingState;

        if (targetPluginName === sourcePlugin) {
          showToast('Eklenti kendi kendisine bağlanamaz!', 'error');
        } else {
          const targetPluginObj = state.data.find(p => p.plugin_name === targetPluginName);
          if (targetPluginObj) {
            if (!Array.isArray(targetPluginObj.plugin_inputs)) targetPluginObj.plugin_inputs = [];

            const exists = targetPluginObj.plugin_inputs.some(i => i.source === sourcePlugin && i.stream_id === streamId);
            if (exists) {
              showToast(`'${streamId}' bağlantısı zaten mevcut.`, 'info');
            } else {
              targetPluginObj.plugin_inputs.push({ source: sourcePlugin, stream_id: streamId, params: {} });
              updateDataStore(state.data);
              showToast(`⚡ '${streamId}' akışı: '${sourcePlugin}' ➔ '${targetPluginName}' olarak bağlandı!`, 'success');
            }
          }
        }
      }

      state.connectingState = null;
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
