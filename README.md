# Cycle Orchestrator - Core Architecture

The main system (`orchestrator`) is designed as a lightweight central shell completely isolated from business logic (plugins), establishing management, user interface, and zero-latency RAM-based inter-plugin communication infrastructure.

The core architectural components of the system are detailed below:

## 1. The Core Concept
The core system contains no built-in trading logic, network request code, or database handling. Its sole purpose is to dynamically load compiled shared libraries (`.so` / `.dll`) into memory at runtime, manage their lifecycle, and facilitate zero-copy zero-latency inter-plugin data routing via shared memory buffers.

## 2. Modules and Responsibilities

### `orchestrator.rs` (Central Manager)
The heart of the system.
- **DashMap Usage:** Utilizes `systems: DashMap<String, SystemBox>` for thread-safe concurrent storage of loaded plugins (systems) in RAM.
- **Responsibilities:** Executes plugin registration (`register_system`), unregistration (`unregister_system`), and standard C-ABI endpoint dispatcher calls directly via memory references without asynchronous blocking delays.

### `system.rs` (Plugin Contract)
The strict trait contract every plugin must implement.
- **`System` Trait:** Forces plugins to expose identity, input/output ports, standard C-ABI endpoints, and memory contexts.
- **`SystemContext`:** Dedicated data structure for each plugin containing its ID, running state (`is_running`), data validity flag (`is_data_valid`), and dedicated allocated RAM buffer (`memory`).
- **Dynamic Dispatch:** Uses `Box<dyn System>` to inspect and manage plugins uniformly without knowing internal implementations.

### `endpoint.rs` (Standard Communication Protocol)
The RPC protocol operating over shared RAM buffers rather than sockets.
- **Core Endpoints:** `Start`, `Stop`, `DataMonitor`, `Inbox`, `Outbox`, etc.
- When a plugin is loaded, the orchestrator triggers these endpoints to initialize and control execution state.

### `memory.rs` (Zero-Latency Shared Memory)
Designed to swap zero-copy memory buffers between systems without file or network overhead.
- Leverages `Arc<RwLock<Vec<u8>>>`.
- Large binary market feeds (such as L2 orderbook diffs) are stored and accessed instantaneously by the orchestrator and downstream consumer plugins.

## 3. Dynamic Plugin Loading (`libloading`)
The main core has zero compile-time dependencies on specific plugins.
At runtime, it scans `target/debug/` for `libplugin_*.so` (Linux) or `*.dll` (Windows) dynamic libraries.
When selected by the user, `libloading::Library::new()` loads the library into RAM, invokes `create_plugin`, instantiates the plugin, and registers it to the system registry (`register_system`).

## 4. Terminal UI (TUI)
Located under `crates/core/orchestrator/src/tui_interface/`.
- **Ratatui & Crossterm:** Provides a lightweight terminal UI dashboard.
- **Event Loop:** Handles key bindings and mouse events for controlling systems (selection, start, stop, monitor, unload).
- Automatically invokes `DataMonitor` on the selected plugin per frame render to display real-time hex and structured feeds.

## 5. Project Layout

```text
cycle-orc/
├── config/
│   └── config.json              # Stream & plugin DAG routing configs
├── data/
│   ├── binance_market_data.db   # Market data database (SQLite)
│   └── paper_exchange.db        # Simulation data database (SQLite)
├── docs/                        # Architecture and design specifications
│   ├── CYCLE_LANG_SPECIFICATION.md
│   ├── FLOW_ENGINE_PLAN.md
│   ├── HFT_ANALYSIS.md
│   ├── IMPLEMENTATION_PLAN.md
│   └── PROJECT_DOCS.md
└── crates/
    ├── apps/                    # CLI and standalone binaries
    ├── core/                    # Orchestrator & Flow Engine Core
    ├── interfaces/              # TUI and Web Interfaces
    └── plugins/                 # Producer, Analytics, Strategy, and Execution Plugins
```

