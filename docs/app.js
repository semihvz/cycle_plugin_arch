/* Cycle Orchestrator Architecture & Research Models App Logic */

document.addEventListener('DOMContentLoaded', () => {
    initNavigation();
    initSearch();
    initKeyboardShortcuts();
    initMobileMenu();

    // Default load first section
    loadSection('overview');
});

// Navigation state
let currentSectionId = 'overview';

function initNavigation() {
    const navMenu = document.getElementById('navMenu');
    if (!navMenu || !docContent) return;

    let html = '';
    let currentCategory = '';

    docContent.sections.forEach(sec => {
        if (sec.category !== currentCategory) {
            currentCategory = sec.category;
            html += `<div class="nav-section-title">${currentCategory}</div>`;
        }

        html += `
            <a class="nav-item ${sec.id === currentSectionId ? 'active' : ''}" 
               data-id="${sec.id}" 
               onclick="loadSection('${sec.id}')">
                <span class="icon">${sec.icon}</span>
                <span>${sec.title}</span>
            </a>
        `;
    });

    navMenu.innerHTML = html;
}

function loadSection(sectionId) {
    const sec = docContent.sections.find(s => s.id === sectionId);
    if (!sec) return;

    currentSectionId = sectionId;

    // Update nav active states
    document.querySelectorAll('.nav-item').forEach(item => {
        if (item.getAttribute('data-id') === sectionId) {
            item.classList.add('active');
        } else {
            item.classList.remove('active');
        }
    });

    // Update breadcrumb
    const breadcrumb = document.getElementById('breadcrumb');
    if (breadcrumb) {
        breadcrumb.textContent = `Sistem Mimarisi / ${sec.category} / ${sec.title}`;
    }

    // Render content
    const wrapper = document.getElementById('contentWrapper');
    if (wrapper) {
        wrapper.innerHTML = sec.content;

        // Render KaTeX LaTeX math if available
        if (window.renderMathInElement) {
            try {
                window.renderMathInElement(wrapper, {
                    delimiters: [
                        { left: '$$', right: '$$', display: true },
                        { left: '$', right: '$', display: false },
                        { left: '\\(', right: '\\)', display: false },
                        { left: '\\[', right: '\\]', display: true }
                    ],
                    throwOnError: false
                });
            } catch (err) {
                console.warn('KaTeX render error:', err);
            }
        }
    }

    // Scroll to top
    window.scrollTo({ top: 0, behavior: 'smooth' });

    // Initializations for interactive sections
    if (sectionId === 'dag-simulator') {
        setTimeout(initDagCanvas, 100);
    } else if (sectionId === 'market-breakout-model') {
        setTimeout(calculateBreakoutModel, 100);
    }
}

// Search Functionality
function initSearch() {
    const input = document.getElementById('searchInput');
    if (!input) return;

    input.addEventListener('input', (e) => {
        const query = e.target.value.toLowerCase().trim();
        if (!query) {
            initNavigation();
            return;
        }

        const navMenu = document.getElementById('navMenu');
        let html = '<div class="nav-section-title">ARAMA SONUÇLARI</div>';

        const matches = docContent.sections.filter(sec => 
            sec.title.toLowerCase().includes(query) ||
            sec.summary.toLowerCase().includes(query) ||
            sec.content.toLowerCase().includes(query)
        );

        if (matches.length === 0) {
            html += `<div style="padding: 12px; font-size: 13px; color: var(--text-dim);">Sonuç bulunamadı</div>`;
        } else {
            matches.forEach(sec => {
                html += `
                    <a class="nav-item ${sec.id === currentSectionId ? 'active' : ''}" 
                       data-id="${sec.id}" 
                       onclick="loadSection('${sec.id}')">
                        <span class="icon">${sec.icon}</span>
                        <span>${sec.title}</span>
                    </a>
                `;
            });
        }

        navMenu.innerHTML = html;
    });
}

function initKeyboardShortcuts() {
    document.addEventListener('keydown', (e) => {
        if (e.key === '/' && document.activeElement.tagName !== 'INPUT') {
            e.preventDefault();
            const input = document.getElementById('searchInput');
            if (input) input.focus();
        }
    });
}

function initMobileMenu() {
    const btn = document.getElementById('mobileMenuBtn');
    const sidebar = document.getElementById('sidebar');
    if (btn && sidebar) {
        btn.addEventListener('click', () => {
            sidebar.classList.toggle('open');
        });
    }
}

// Copy Code Helper
function copyCode(btn) {
    const codeBlock = btn.parentElement.nextElementSibling.querySelector('code');
    if (!codeBlock) return;

    const text = codeBlock.innerText;
    navigator.clipboard.writeText(text).then(() => {
        btn.textContent = 'Kopyalandı!';
        showToast('Kod panoya kopyalandı 📋');
        setTimeout(() => {
            btn.textContent = 'Kopyala';
        }, 2000);
    });
}

// Toast Notifications
function showToast(msg) {
    const container = document.getElementById('toastContainer');
    if (!container) return;

    const toast = document.createElement('div');
    toast.className = 'toast';
    toast.textContent = msg;
    container.appendChild(toast);

    setTimeout(() => {
        toast.remove();
    }, 3000);
}

// Interactive Breakout Model Calculator
function calculateBreakoutModel() {
    const p = parseFloat(document.getElementById('calcPrice')?.value || 99.70);
    const l = parseFloat(document.getElementById('calcLevel')?.value || 100.00);
    const atr = parseFloat(document.getElementById('calcAtr')?.value || 0.80);
    const buyVol = parseFloat(document.getElementById('calcBuyVol')?.value || 450);
    const sellVol = parseFloat(document.getElementById('calcSellVol')?.value || 120);
    const initLiq = parseFloat(document.getElementById('calcInitLiq')?.value || 1000);
    const currLiq = parseFloat(document.getElementById('calcCurrLiq')?.value || 250);
    const shortLiq = parseFloat(document.getElementById('calcShortLiq')?.value || 350000);

    // 1. Activation Distance D
    const distance = Math.abs(p - l) / (atr || 1);

    // 2. Delta Ratio
    const totalVol = buyVol + sellVol;
    const deltaRatio = totalVol > 0 ? (buyVol - sellVol) / totalVol : 0;

    // 3. Liquidity Depletion
    const depletion = initLiq > 0 ? Math.max(0, (initLiq - currLiq) / initLiq) : 0;

    // 4. Sigmoid Probability Calculations
    const z1 = -0.5 + (2.8 * deltaRatio) + (3.2 * depletion) - (1.2 * distance) + (shortLiq * 0.000002);
    const probBreakout = 1 / (1 + Math.exp(-z1));

    const z2 = -0.8 + (2.2 * deltaRatio) + (2.5 * depletion) + (shortLiq * 0.0000015);
    const probSustained = 1 / (1 + Math.exp(-z2));

    // Update DOM
    if (document.getElementById('resDistance')) {
        document.getElementById('resDistance').textContent = distance.toFixed(3);
    }
    if (document.getElementById('resDeltaRatio')) {
        document.getElementById('resDeltaRatio').textContent = (deltaRatio >= 0 ? '+' : '') + deltaRatio.toFixed(3);
    }
    if (document.getElementById('resDepletion')) {
        document.getElementById('resDepletion').textContent = (depletion * 100).toFixed(1) + '%';
    }
    if (document.getElementById('resProbBreakout')) {
        document.getElementById('resProbBreakout').textContent = (probBreakout * 100).toFixed(1) + '%';
    }
    if (document.getElementById('resProbSustained')) {
        document.getElementById('resProbSustained').textContent = (probSustained * 100).toFixed(1) + '%';
    }
}

// DAG Canvas Interactive Simulator
let isSimulating = false;
let simInterval = null;

function initDagCanvas() {
    const svg = document.getElementById('dagCanvas');
    if (!svg) return;

    const width = svg.clientWidth || 800;
    const height = svg.clientHeight || 400;

    svg.setAttribute('viewBox', `0 0 ${width} ${height}`);

    const nodes = [
        { id: 'gateway', label: 'Binance Gateway', sub: 'Producer (WS L2)', x: 120, y: height / 2, color: '#00f3ff' },
        { id: 'router', label: 'Memory Router', sub: 'Arc<RwLock<Vec<u8>>>', x: width / 2, y: height / 2, color: '#a855f7' },
        { id: 'breakout', label: 'Plugin Breakout', sub: 'Analytics Signal', x: width - 140, y: height / 2 - 80, color: '#00ff9d' },
        { id: 'paper', label: 'Paper Exchange', sub: 'Storage & Engine', x: width - 140, y: height / 2 + 80, color: '#00ff9d' }
    ];

    const connections = [
        { from: 'gateway', to: 'router', label: 'L2 Depth Feed' },
        { from: 'router', to: 'breakout', label: 'Stream: binance_l2' },
        { from: 'router', to: 'paper', label: 'Stream: binance_trades' }
    ];

    let svgHtml = `
        <defs>
            <linearGradient id="cyanGlow" x1="0%" y1="0%" x2="100%" y2="0%">
                <stop offset="0%" stop-color="#00f3ff" stop-opacity="0.8"/>
                <stop offset="100%" stop-color="#a855f7" stop-opacity="0.8"/>
            </linearGradient>
            <filter id="glow">
                <feGaussianBlur stdDeviation="3" result="coloredBlur"/>
                <feMerge>
                    <feMergeNode in="coloredBlur"/>
                    <feMergeNode in="SourceGraphic"/>
                </feMerge>
            </filter>
        </defs>
    `;

    // Draw connections
    connections.forEach(conn => {
        const source = nodes.find(n => n.id === conn.from);
        const target = nodes.find(n => n.id === conn.to);
        if (source && target) {
            svgHtml += `
                <g class="connection-group">
                    <line x1="${source.x}" y1="${source.y}" x2="${target.x}" y2="${target.y}" 
                          stroke="rgba(255, 255, 255, 0.15)" stroke-width="2" stroke-dasharray="6,6"/>
                    <text x="${(source.x + target.x) / 2}" y="${(source.y + target.y) / 2 - 10}" 
                          fill="#64748b" font-size="11" font-family="Fira Code" text-anchor="middle">
                          ${conn.label}
                    </text>
                </g>
            `;
        }
    });

    // Draw nodes
    nodes.forEach(node => {
        svgHtml += `
            <g class="dag-node" transform="translate(${node.x}, ${node.y})">
                <rect x="-90" y="-35" width="180" height="70" rx="12" 
                      fill="#0e1626" stroke="${node.color}" stroke-width="2" filter="url(#glow)"/>
                <text x="0" y="-8" fill="#ffffff" font-size="13" font-weight="700" text-anchor="middle">${node.label}</text>
                <text x="0" y="14" fill="#94a3b8" font-size="10" font-family="Fira Code" text-anchor="middle">${node.sub}</text>
            </g>
        `;
    });

    svg.innerHTML = svgHtml;
}

function toggleSimulation() {
    const btn = document.getElementById('startSimBtn');
    const status = document.getElementById('simStatusText');
    if (!btn || !status) return;

    isSimulating = !isSimulating;

    if (isSimulating) {
        btn.textContent = '⏸ Durdur';
        status.textContent = 'STREAMING (3.2k msg/s)';
        status.style.color = 'var(--accent-cyan)';
        simInterval = setInterval(triggerPacket, 400);
        showToast('RAM Veri Simülasyonu Başlatıldı ⚡');
    } else {
        btn.textContent = '▶ Simülasyonu Başlat';
        status.textContent = 'IDLE';
        status.style.color = 'var(--accent-green)';
        clearInterval(simInterval);
        showToast('Simülasyon Durduruldu');
    }
}

function triggerPacket() {
    const svg = document.getElementById('dagCanvas');
    if (!svg) return;

    const width = svg.clientWidth || 800;
    const height = svg.clientHeight || 400;

    const packet = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
    packet.setAttribute('r', '6');
    packet.setAttribute('fill', '#00f3ff');
    packet.setAttribute('filter', 'url(#glow)');

    svg.appendChild(packet);

    let progress = 0;
    const startX = 120;
    const startY = height / 2;
    const midX = width / 2;
    const midY = height / 2;
    const endX = width - 140;
    const endY = height / 2 - 80;

    const anim = setInterval(() => {
        progress += 0.05;
        if (progress <= 0.5) {
            const t = progress * 2;
            packet.setAttribute('cx', startX + (midX - startX) * t);
            packet.setAttribute('cy', startY + (midY - startY) * t);
        } else if (progress <= 1.0) {
            const t = (progress - 0.5) * 2;
            packet.setAttribute('cx', midX + (endX - midX) * t);
            packet.setAttribute('cy', midY + (endY - midY) * t);
        } else {
            clearInterval(anim);
            packet.remove();
        }
    }, 20);
}
