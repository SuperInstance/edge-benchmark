# edge-benchmark

Benchmarking primitives for edge runtimes (Cloudflare Workers compatible).

**No filesystem. No threads. No chrono.** Works in `no_std` (with `std` feature for `Instant`-based timing). Zero dependencies in `no_std` mode.

## Features

| Component | Description |
|---|---|
| `EdgeTimer` | Sub-millisecond timer using `std::time::Instant` (or no-op in `no_std`) |
| `Benchmark` | Run a closure N times, compute mean/median/p95/p99/min/max/stddev |
| `BenchmarkSuite` | Run multiple named benchmarks, produce results |
| `BenchmarkReport` | Serialize results as JSON (hand-rolled, no serde) or Markdown table |
| `ComparisonReport` | Compare two suites (before/after optimization) with delta% |
| `StatisticalSignificance` | Welch's t-test between two benchmark results |

## Usage

```rust
use edge_benchmark::{Benchmark, BenchmarkSuite, ComparisonReport, t_test};

// Run a single benchmark
let result = Benchmark::run("hash_sha256", 1000, || {
    // your code here
});

// Build a suite
let mut suite = BenchmarkSuite::new("my-suite");
suite.add(result);
println!("{}", suite.to_json());
println!("{}", suite.to_markdown());

// Compare before/after
let cmp = ComparisonReport::new(baseline_suite, optimized_suite);
println!("{}", cmp.to_markdown());

// Statistical significance
let sig = t_test(&result_a, &result_b, 0.05);
println!("significant: {}", sig.significant);
```

## `no_std` Usage

```toml
[dependencies]
edge-benchmark = { version = "0.1", default-features = false }
```

Timer operations return 0 in `no_std` mode (no wall-clock available), but all statistical analysis and reporting still works with manually-provided samples.

## License

MIT
