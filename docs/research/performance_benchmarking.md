# Hardware & Performance Benchmarking: Rust on ARM

## Target Hardware Performance

| Hardware | CPU | Performance Note | Recommended Latency Target |
| :--- | :--- | :--- | :--- |
| **Raspberry Pi 4** | Cortex-A72 | Reliable with PREEMPT_RT. | < 10ms (JACK) |
| **Raspberry Pi 5** | Cortex-A76 | ~120% faster than Pi 4. | < 5ms (JACK) |

### Optimization Checklist
1. **CPU Governor:** Set to `performance` to avoid clock-speed scaling latency.
2. **Audio Backend:** Prefer **JACK** on Linux for deterministic low latency.
3. **Release Mode:** Always build with `cargo build --release`.
4. **Denormals:** Enable "Flush-to-Zero" (FTZ) to prevent CPU spikes from very small float values.

## Real-Time Safety "Golden Rules"

The audio callback (hot path) MUST adhere to these rules to avoid xruns (audio glitches):
- **NO Allocations:** Avoid `Vec`, `Box`, `String`, `HashMap`.
- **NO Blocking:** Use lock-free buffers (`rtrb`, `ringbuf`) instead of `Mutex`.
- **NO I/O:** No disk reads/writes or network access.
- **NO Panics:** Use `catch_unwind` or verify code is panic-free.

## SIMD & ARM NEON Optimization

For high-density synthesis (multiple voices, heavy filters), use NEON intrinsics:
- **Vectorization:** Process 4 or 8 samples at once using `vld1q_f32` and `vmulq_f32`.
- **Memory Alignment:** Align buffers to 16-byte boundaries for faster loads.
- **Fixed-Point:** Consider `i32` fixed-point math if floating-point performance becomes a bottleneck on older ARM hardware.

## Benchmarking Tools
- **`assert_no_alloc`:** Runtime verification of allocation-free paths.
- **`criterion`:** For micro-benchmarking DSP algorithms.
- **`jack_wait` / `jack_lsp`:** For system-level latency monitoring.
