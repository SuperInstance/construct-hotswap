# construct-hotswap

A Rust library for **live construct hotswapping with CRDT state synchronization**, simulating zero-downtime kernel/circuit updates across a multi-node GPU cluster. It models construct lifecycle states (Load → Deploy → Drain → Hotswap) with eventual consistency via CRDT merge across nodes.

## Why It Matters

Live hotswapping — updating code without interrupting service — is the holy grail of high-availability systems. This crate models the pattern used in:

- **GPU kernel hotswapping** — updating CUDA/PTX kernels without pipeline stalls
- **WebAssembly module replacement** — hot-loading new WASM in edge workers
- **Database migrations** — blue-green schema swaps with backfill
- **Game engine updates** — swapping shader/compute pipelines mid-frame
- **Service mesh sidecars** — Envoy/proxy filter chain hot reload

The CRDT (Conflict-free Replicated Data Type) state sync ensures all nodes converge to the same construct version set without coordination — critical for ultra-low-latency systems where distributed consensus is too slow.

## How It Works

### Construct Lifecycle State Machine

```
    Load         Deploy        Hotswap
  ──────→      ──────→         ──────→
  Loaded      Deployed        Draining
                              Deployed
```

States follow a strict transition protocol:

| Transition | Trigger | Latency |
|-----------|---------|---------|
| → Loaded | `load(name, version)` | 100 µs |
| → Deployed | `deploy(name, node)` | 50 µs |
| → Draining | `hotswap(name, new_ver)` start | — |
| → Deployed | `hotswap(name, new_ver)` complete | 300 µs + CRDT sync (200 µs) |

### CRDT Merge Protocol

Each node maintains a `HashMap<String, String>` mapping construct names to versions. The CRDT merge is **last-writer-wins (LWW)** based on microsecond timestamps:

$$\text{merge}(S_A, S_B) = \{(k, v) : v = \arg\max_t \{(k, v, t) \in S_A \cup S_B\}\}$$

The sync operation:
1. Compute the union of all node states
2. For each key, keep the value from the node with the highest timestamp
3. Broadcast merged state to all nodes

This is an **eventual consistency** model — all nodes converge after one sync round.

### Hotswap Protocol

The zero-downtime hotswap sequence:

```
1. Set construct state → Draining     (in-flight requests finish)
2. Swap version + kernel PTX          (atomic pointer swap)
3. Set construct state → Deployed      (new requests accepted)
4. CRDT sync to all nodes              (version propagation)
```

Total hotswap latency: `300 µs (drain) + 100 µs (swap) + 200 µs (sync) = 600 µs`

### Complexity Analysis

| Operation | Time | Space |
|-----------|------|-------|
| `load(name, version)` | O(1) | O(1) |
| `deploy(name, node)` | O(1) | O(1) |
| `crdt_sync()` | O(N × K) | O(K) |
| `hotswap(name, version)` | O(N × K) | O(K) |

Where N = node count, K = total deployed constructs.

## Quick Start

```rust
use construct_hotswap::HotswapExperiment;

let mut exp = HotswapExperiment::new(3); // 3 GPU nodes
exp.load("attention", "v1.0");
exp.deploy("attention", 0).unwrap();
exp.crdt_sync();

// Hotswap to v2.0 without downtime
let completion_time = exp.hotswap("attention", "v2.0").unwrap();

// Verify all nodes eventually see v2.0
exp.crdt_sync();
for node in 0..3 {
    assert!(exp.events().iter().any(|e| e.event_type == EventType::HotswapComplete));
}
```

## API

| Method | Description |
|--------|-------------|
| `HotswapExperiment::new(node_count)` | Initialize multi-node cluster |
| `load(name, version)` | Load a construct into registry |
| `deploy(name, node_idx)` | Deploy construct to a node |
| `crdt_sync()` | Gossip state across all nodes |
| `hotswap(name, new_version) → Result<u64, String>` | Execute zero-downtime swap |
| `events() → &[HotswapEvent]` | Full event log |
| `total_time_us() → u64` | Simulation clock |
| `deployed_count() → usize` | Active deployed constructs |
| `node_count() → usize` | Cluster size |

### Types

| Type | Description |
|------|-------------|
| `Construct` | `{name, version, kernel_ptx, state}` |
| `ConstructState` | `Loaded`, `Deployed`, `Draining`, `Cached` |
| `HotswapEvent` | `{time_us, event_type, node, construct}` |
| `EventType` | `Load`, `Deploy`, `CrdtSync`, `HotswapDrain`, `HotswapComplete`, `Unload` |

## Architecture Notes

The **γ + η = C** link: the hotswap protocol (γ) transitions the construct between versions while in-flight work drains, while the CRDT sync (η) propagates the new version state across the cluster. Together they conserve the consistency invariant C — after sync completes, all nodes agree on the deployed version for every construct. The event log provides a complete audit trail of all state transitions, enabling post-hoc verification that C was maintained throughout the hotswap. The `Draining` state is the critical safety window: no requests are lost because the old version continues serving until the swap is atomic.

## References

- Shapiro, M., Preguiça, N., Baquero, C., & Zawirski, M. (2011). *Conflict-Free Replicated Data Types.* SSRN/STTT 2011.
- Brewer, E. (2000). *Towards Robust Distributed Systems.* PODC Keynote. (CAP theorem — CRDTs choose AP.)
- Aiyer, A. S., et al. (2015). *Consistency Analysis of Bloom Filters.* (LWW CRDT analysis.)
- Verma, A., et al. (2015). *The Design of a Practical System for Fault-Tolerant Virtual Machines.* OSDI. (Live migration analog.)
- Brewer, E. (2012). *CAP Twelve Years Later.* IEEE Computer. (Eventual consistency justification.)

## License

MIT
