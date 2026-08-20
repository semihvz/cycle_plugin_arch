# Node-Based Data Flow & Data Router Engine Architecture

In accordance with system requirements, a **fully independent and configuration-driven Core System (Data Router / Flow Engine)** has been designed to decouple inter-plugin communication from the main Orchestrator (TUI/System Management).

## 1. Core Philosophy: Plugins = Functions (Nodes)
Every plugin is designed as an independent function / black box with inputs and outputs.
A plugin does not know where data comes from or where data goes; it simply reads its assigned **Input Buffer** and writes to its **Output Buffer**.

## 2. Dynamic Routing via Configuration File
Connecting outputs of producer plugins to inputs of consumer plugins is not hardcoded. This structure is defined by the user in a `flow_config.json` (or TOML/YAML) file.

**Sample Configuration Structure (`flow_config.toml`):**
```toml
# DATA PRODUCERS (Sources)
[[plugin]]
name = "all_markprices"
type = "producer"
outputs = ["stream_markprice"]

[[plugin]]
name = "all_aggtrades"
type = "producer"
outputs = ["stream_trades"]

# DATA PROCESSORS / ANALYZERS
[[plugin]]
name = "ms_analyzer"
type = "processor"
inputs = { markprice = "stream_markprice", trades = "stream_trades" }
outputs = ["stream_ms_signals"]

# DECISION MAKERS / EXECUTION (Consumers)
[[plugin]]
name = "plugin_breakout"
type = "consumer"
inputs = { signals = "stream_ms_signals" }
```
With this configuration, plugins connect together like LEGO bricks, allowing full system re-architecting without modifying any code.

## 3. Zero-Copy Shared Memory Transfer (RAM)
Instead of copying bytes from one plugin to another, a **Shared Memory (Pointers)** architecture is used.
* When the Data Router initializes the config, it allocates a shared memory buffer (e.g. 1MB allocated memory block pointer) named `stream_markprice`.
* It grants **Write Permission (Write Pointer)** to `all_markprices` and **Read Permission (Read Pointer)** to `ms_analyzer`.
* As soon as `all_markprices` writes a new price update, `ms_analyzer` reads it instantaneously with zero-copy microsecond latency.

## 4. Communication Types
1. **Continuous Streaming (Pub-Sub):** Real-time price and orderbook streaming updated thousands of times per second via shared memory.
2. **Request - Response (RPC):** Direct targeted requests like "Fetch the last 10 minutes of OHLCV data". The Data Router forwards these requests along defined routes and returns the result instantly over RAM.

## 5. Health & Monitoring Mechanism (Health Check & Watchdog)
An independent Watchdog runs inside the Data Router to ensure data flow health:
* **Heartbeat & Timestamp:** Each shared memory block begins with a `last_updated_timestamp` (last update in milliseconds).
* **Continuous Monitoring:** The Data Router inspects all streams every 500ms. If `stream_markprice` has not updated for 2 seconds (when market is open), the system emits a alert to the Orchestrator and user ("MarkPrice stream stalled!").
* **Data Validation:** Performs header and checksum checks to verify data structure integrity, instantly detecting corrupted memory writes and isolating malfunctioning plugins.

---

## User Review Required

> [!IMPORTANT]
> This design defines a separate core layer acting as an "Industrial Data Bus / Fabric" alongside the Orchestrator.
> 
> This architectural plan enables zero-copy data routing and health monitoring driven entirely by configuration files.
