# HFT (Ultra-Low Latency) Architecture Transition Plan

A comprehensive architectural overhaul has been implemented to transform the core system (Orchestrator) from a conventional management shell into a **Real-Time HFT Engine** capable of competing at microsecond/nanosecond speed scales.

> ⚠️ **Breaking Change Notice**
> This transition has completely refactored the core `System` trait interface and memory management of the orchestrator. All newly authored plugins must strictly conform to these new "Zero-Copy" and "V-Table-Free" (C-ABI) specifications.

## ❓ Approved Decisions
- ✅ When authoring new plugins, instead of the Rust `dyn System` trait, native C-ABI style `extern "C"` functions are exposed. This is the optimal path for ultra-low latency HFT.
- ✅ CPU core pinning constraints are applied. The main execution thread is pinned to CPU Core 0.

## 🛠 Architectural Changes Implemented

The following components have been rewritten to adhere to ultra-low latency HFT standards (Lock-free, Zero-copy, No V-Table, CPU Pinning):

---

### Orchestrator Core Structures

#### [MODIFY] memory.rs
- **Removed:** `Arc<RwLock<Vec<u8>>>` mutex/rwlock mechanism.
- **Added:** 
  - `crossbeam::queue::ArrayQueue` based **Lock-Free Ring Buffer** for incoming and outgoing messages (Inbox/Outbox). Threads execute reads and writes concurrently without lock contention.

#### [MODIFY] system.rs
- **Removed:** `dyn System` dynamic dispatch (V-Table lookup latency).
- **Added:**
  - `SystemInstance` structure storing raw memory pointers (`*mut c_void`) and raw function pointers (`extern "C" fn(payload: *const u8, len: usize)`). When an endpoint call is made, CPU instructions execute directly without virtual table lookup.
  - Inter-plugin payloads are passed as zero-copy reference slices (`&[u8]`) instead of allocating heap memory (`Vec<u8>`).
  - State flags (`is_running`, `is_data_valid`) are tracked lock-free via `Arc<AtomicBool>` instead of `Arc<RwLock<bool>>`.

#### [MODIFY] endpoint.rs
- **Added:** `#[repr(u32)]` for C-ABI FFI memory layout compatibility. Each endpoint is assigned a fixed integer discriminant.

#### [MODIFY] orchestrator.rs
- **Removed:** Heavy `DashMap` concurrency bottlenecks.
- **Added:**
  - Systems stored efficiently inside `Vec<Arc<SystemInstance>>`.
  - Routing calls optimized for zero-copy payload forwarding via `&[u8]` slice references.

#### [MODIFY] main.rs
- **Removed:** OS-dependent thread scheduling logic and legacy `create_plugin` (`Box<dyn System>`) dynamic dispatch.
- **Added:** 
  - Pinned main thread to CPU Core 0 using `core_affinity` to prevent L1/L2 CPU cache misses.
  - New `init_plugin` C-ABI loader pattern (`extern "C" fn(state_out) -> RawEndpointFn`).
  - 1MB pre-allocated `hft_buf` buffer ensuring zero heap allocations on the hot path.

---

## 🧪 Verification Results
1. ✅ **Compilation:** Successfully compiled via `cargo check` and `cargo build` with HFT dependencies (`crossbeam`, `core_affinity`). Zero errors, zero warnings.
2. ✅ **Version Control:** Changes committed and pushed to GitHub.

## 📊 Before / After Architecture Comparison

| Component | Legacy Architecture (Bottleneck) | New Architecture (Ultra-HFT) |
|---|---|---|
| **memory.rs** | `Arc<RwLock<Vec<u8>>>` (Lock Contention) | `crossbeam::ArrayQueue` (Lock-free Ring Buffer) |
| **system.rs** | `Box<dyn System>` + V-Table | `SystemInstance` + `extern "C"` raw fn pointers |
| **endpoint.rs** | Rust Enum | `#[repr(u32)]` C-ABI Enum |
| **orchestrator.rs** | `DashMap` (Lock Overhead) | `Vec<Arc<SystemInstance>>` + zero-copy `&[u8]` |
| **main.rs (Plugin Loader)** | `create_plugin` → `Box<dyn System>` | `init_plugin` → C-ABI raw function pointer |
| **main.rs (CPU Pinning)** | OS Thread Scheduling | Pinned to CPU Core 0 via `core_affinity` |
| **main.rs (Memory)** | Dynamic `Vec<u8>` heap allocations | 1MB pre-allocated static `hft_buf` |
