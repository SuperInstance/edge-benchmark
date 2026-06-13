# Edge Benchmark

**Edge Benchmark** is a Rust library providing benchmarking primitives designed for edge runtimes (Cloudflare Workers compatible) — supporting `no_std` environments with sub-millisecond timing, statistical analysis, Welch's t-test for significance testing, and Markdown/JSON report generation.

## Why It Matters

Benchmarking on edge runtimes is fundamentally different from server benchmarking: there are no threads, no filesystem, no `chrono`, and limited wall-clock access. Standard benchmarking crates (`criterion`, `iai`) assume a full OS environment and fail on Workers, WASM, and embedded targets. Edge Benchmark provides the minimal primitives needed — a timer wrapper, sample collection, percentile computation, and comparison — that work in both `std` and `no_std` environments. This enables continuous performance monitoring on edge deployments, catching regressions introduced by Worker runtime updates, V8 engine changes, or code modifications. The built-in Welch's t-test ensures that reported regressions are statistically significant, not just measurement noise.

## How It Works

**EdgeTimer:** Wraps `std::time::Instant` (std feature) or provides a no-op stub (no_std). The timer is created and started in one call, and `elapsed_ns()` / `elapsed_us()` return nanosecond/microsecond resolution.

**Benchmark::run:** Executes a closure N times, collecting per-iteration nanosecond timings:

```
run(name, iterations, f):
  samples = Vec::with_capacity(iterations)
  for _ in 0..iterations:
    t0 = EdgeTimer::start()
    f()
    samples.push(t0.elapsed_ns())
  return BenchmarkResult::from_samples(name, samples)
```

**BenchmarkResult statistics:** Computed from sorted samples:

| Statistic | Formula | Complexity |
|-----------|---------|------------|
| Mean | Σxᵢ/n | O(n) |
| Median | Middle element(s) of sorted | O(1) |
| P95, P99 | Percentile index = (pct/100)×(n−1) | O(1) from sorted |
| StdDev | √(Σ(xᵢ−μ)²/n) | O(n) |
| Min, Max | First/last of sorted | O(1) |

**Welch's t-test:** For comparing two benchmark results:

```
t = (mean_a − mean_b) / √(var_a/n_a + var_b/n_b)
```

With simplified critical-value lookup: |t| > 1.96 at α=0.05, > 2.576 at α=0.01, > 3.291 at α=0.001.

**Comparison report:** Deltas matched by benchmark name:

```
delta% = ((optimized_mean − baseline_mean) / baseline_mean) × 100
```

Negative delta = faster (improvement). Positive delta = slower (regression).

## Quick Start

```rust
use edge_benchmark::{Benchmark, BenchmarkSuite, ComparisonReport};

fn main() {
    let result = Benchmark::run("hash_op", 1000, || {
        let _ = 1u64.wrapping_mul(0x9e3779b97f4a7c15);
    });
    println!("{}", result.to_json());
}
```

## API

| Type | Description |
|------|-------------|
| `EdgeTimer` | Monotonic timer (std/no_std) |
| `BenchmarkResult` | Per-benchmark statistics |
| `BenchmarkSuite` | Named collection of results |
| `ComparisonReport` | Baseline vs. optimized comparison |
| `t_test` | Welch's t-test for significance |
| `BenchmarkReport` | JSON/Markdown output |

## Architecture Notes

Edge Benchmark provides the **edge-runtime performance monitoring** for γ + η = C. When conservation-law verification runs on Cloudflare Workers (η-layer), Edge Benchmark ensures it meets its latency budget. A regression that pushes conservation verification over the CPU time limit (50ms on Workers) would violate the real-time conservation contract.

See [ARCHITECTURE.md](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md).

## References

1. Welch, B.L. (1947). "The Generalization of 'Student's' Problem." *Biometrika*, 34(1/2), 28–35.
2. Cloudflare (2024). *Workers Runtime Limits: CPU Time*. developers.cloudflare.com.

## License

MIT
