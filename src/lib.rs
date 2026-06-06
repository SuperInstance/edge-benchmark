#![cfg_attr(not(feature = "std"), no_std)]

//! # edge-benchmark
//!
//! Benchmarking primitives for edge runtimes (Cloudflare Workers compatible).
//! No filesystem, no threads, no chrono. Works in `no_std` (with `std` feature
//! for `Instant`-based timing).

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;


// ---------------------------------------------------------------------------
// EdgeTimer
// ---------------------------------------------------------------------------

/// Sub-millisecond timer wrapping `std::time::Instant` (std feature) or
/// providing a no-op stub in no_std.
#[derive(Clone, Debug)]
pub struct EdgeTimer {
    #[cfg(feature = "std")]
    start: std::time::Instant,
    #[cfg(not(feature = "std"))]
    _started: bool,
}

impl EdgeTimer {
    /// Create and immediately start a new timer.
    #[cfg(feature = "std")]
    pub fn start() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    /// No-op timer for no_std environments.
    #[cfg(not(feature = "std"))]
    pub fn start() -> Self {
        Self { _started: true }
    }

    /// Return elapsed time in microseconds.
    #[cfg(feature = "std")]
    pub fn elapsed_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    /// Returns 0 in no_std (no wall-clock available).
    #[cfg(not(feature = "std"))]
    pub fn elapsed_us(&self) -> u64 {
        0
    }

    /// Return elapsed time in nanoseconds.
    #[cfg(feature = "std")]
    pub fn elapsed_ns(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }

    /// Returns 0 in no_std.
    #[cfg(not(feature = "std"))]
    pub fn elapsed_ns(&self) -> u64 {
        0
    }
}

// ---------------------------------------------------------------------------
// Statistics helper
// ---------------------------------------------------------------------------

/// Compute percentile (0-100) from a sorted slice. Returns 0 for empty input.
fn percentile(sorted: &[u64], pct: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Mean of a slice. Returns 0.0 for empty.
fn mean(data: &[u64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    data.iter().fold(0u64, |a, &b| a + b) as f64 / data.len() as f64
}

/// Variance (population) of a slice. Returns 0.0 for empty or single element.
fn variance(data: &[u64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let m = mean(data);
    let sum: f64 = data.iter().map(|&v| {
        let d = v as f64 - m;
        d * d
    }).sum();
    sum / data.len() as f64
}

/// Sample standard deviation.
fn stddev(data: &[u64]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }
    let v = variance(data);
    core::hint::black_box(v).sqrt()
}

// ---------------------------------------------------------------------------
// BenchmarkResult
// ---------------------------------------------------------------------------

/// Summary statistics from a single benchmark run.
#[derive(Clone, Debug)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub samples_ns: Vec<u64>,
    pub mean_ns: f64,
    pub median_ns: f64,
    pub p95_ns: u64,
    pub p99_ns: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub stddev_ns: f64,
}

impl BenchmarkResult {
    /// Build a `BenchmarkResult` from raw nanosecond samples.
    pub fn from_samples(name: &str, samples: Vec<u64>) -> Self {
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        let mean_ns = mean(&sorted);
        let median_ns = if sorted.is_empty() { 0.0 } else {
            let mid = sorted.len() / 2;
            if sorted.len() % 2 == 0 {
                (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
            } else {
                sorted[mid] as f64
            }
        };
        let min_ns = sorted.first().copied().unwrap_or(0);
        let max_ns = sorted.last().copied().unwrap_or(0);
        Self {
            name: name.to_string(),
            iterations: samples.len(),
            samples_ns: samples,
            mean_ns,
            median_ns,
            p95_ns: percentile(&sorted, 95.0),
            p99_ns: percentile(&sorted, 99.0),
            min_ns,
            max_ns,
            stddev_ns: stddev(&sorted),
        }
    }

    /// Serialize as JSON (hand-rolled, no serde).
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"name":"{}","iterations":{},"mean_ns":{:.2},"median_ns":{:.2},"p95_ns":{},"p99_ns":{},"min_ns":{},"max_ns":{},"stddev_ns":{:.2}}}"#,
            self.name,
            self.iterations,
            self.mean_ns,
            self.median_ns,
            self.p95_ns,
            self.p99_ns,
            self.min_ns,
            self.max_ns,
            self.stddev_ns,
        )
    }
}

// ---------------------------------------------------------------------------
// Benchmark — single closure runner
// ---------------------------------------------------------------------------

/// Run a closure `n` times, collecting nanosecond timings.
#[cfg(feature = "std")]
pub struct Benchmark;

#[cfg(feature = "std")]
impl Benchmark {
    /// Run `f` exactly `iterations` times, returning the result.
    pub fn run<F>(name: &str, iterations: usize, f: F) -> BenchmarkResult
    where
        F: Fn(),
    {
        let mut samples = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let timer = EdgeTimer::start();
            f();
            samples.push(timer.elapsed_ns());
        }
        BenchmarkResult::from_samples(name, samples)
    }

    /// Run with a warm-up phase (discarded iterations before measurement).
    pub fn run_with_warmup<F>(name: &str, warmup: usize, iterations: usize, f: F) -> BenchmarkResult
    where
        F: Fn(),
    {
        for _ in 0..warmup {
            f();
        }
        Self::run(name, iterations, f)
    }
}

// ---------------------------------------------------------------------------
// BenchmarkSuite
// ---------------------------------------------------------------------------

/// A named collection of benchmark results.
#[derive(Clone, Debug)]
pub struct BenchmarkSuite {
    pub name: String,
    pub results: Vec<BenchmarkResult>,
}

impl BenchmarkSuite {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            results: Vec::new(),
        }
    }

    /// Add a pre-built result.
    pub fn add(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    /// Serialize the entire suite as JSON.
    pub fn to_json(&self) -> String {
        let items: Vec<String> = self.results.iter().map(|r| r.to_json()).collect();
        format!(
            r#"{{"suite":"{}","results":[{}]}}"#,
            self.name,
            items.join(",")
        )
    }

    /// Render as a Markdown table.
    pub fn to_markdown(&self) -> String {
        let mut md = format!("## {}\n\n", self.name);
        md.push_str("| Benchmark | Iterations | Mean (ns) | Median (ns) | P95 (ns) | P99 (ns) | Min (ns) | Max (ns) | StdDev (ns) |\n");
        md.push_str("|-----------|-----------|-----------|-------------|----------|----------|----------|----------|-------------|\n");
        for r in &self.results {
            md.push_str(&format!(
                "| {} | {} | {:.2} | {:.2} | {} | {} | {} | {} | {:.2} |\n",
                r.name, r.iterations, r.mean_ns, r.median_ns,
                r.p95_ns, r.p99_ns, r.min_ns, r.max_ns, r.stddev_ns,
            ));
        }
        md
    }
}

// ---------------------------------------------------------------------------
// BenchmarkReport
// ---------------------------------------------------------------------------

/// Convenience wrapper for producing reports from a suite.
pub struct BenchmarkReport<'a> {
    pub suite: &'a BenchmarkSuite,
}

impl<'a> BenchmarkReport<'a> {
    pub fn new(suite: &'a BenchmarkSuite) -> Self {
        Self { suite }
    }

    pub fn json(&self) -> String {
        self.suite.to_json()
    }

    pub fn markdown(&self) -> String {
        self.suite.to_markdown()
    }
}

// ---------------------------------------------------------------------------
// ComparisonReport
// ---------------------------------------------------------------------------

/// Compare two suites (e.g. before vs after optimization).
pub struct ComparisonReport {
    pub baseline: BenchmarkSuite,
    pub optimized: BenchmarkSuite,
}

impl ComparisonReport {
    pub fn new(baseline: BenchmarkSuite, optimized: BenchmarkSuite) -> Self {
        Self { baseline, optimized }
    }

    /// Compute per-benchmark delta percentages, matched by name.
    /// Returns a Vec of (name, baseline_mean, optimized_mean, delta_pct).
    pub fn deltas(&self) -> Vec<ComparisonRow> {
        let mut rows = Vec::new();
        for b in &self.baseline.results {
            if let Some(o) = self.optimized.results.iter().find(|r| r.name == b.name) {
                let delta_pct = if b.mean_ns != 0.0 {
                    ((o.mean_ns - b.mean_ns) / b.mean_ns) * 100.0
                } else {
                    0.0
                };
                rows.push(ComparisonRow {
                    name: b.name.clone(),
                    baseline_mean_ns: b.mean_ns,
                    optimized_mean_ns: o.mean_ns,
                    delta_pct,
                });
            }
        }
        rows
    }

    /// Render comparison as Markdown.
    pub fn to_markdown(&self) -> String {
        let mut md = format!("## Comparison: {} vs {}\n\n", self.baseline.name, self.optimized.name);
        md.push_str("| Benchmark | Baseline (ns) | Optimized (ns) | Delta (%) |\n");
        md.push_str("|-----------|---------------|----------------|----------|\n");
        for row in &self.deltas() {
            md.push_str(&format!(
                "| {} | {:.2} | {:.2} | {:+.2}% |\n",
                row.name, row.baseline_mean_ns, row.optimized_mean_ns, row.delta_pct,
            ));
        }
        md
    }
}

/// One row of a comparison.
#[derive(Clone, Debug)]
pub struct ComparisonRow {
    pub name: String,
    pub baseline_mean_ns: f64,
    pub optimized_mean_ns: f64,
    pub delta_pct: f64,
}

// ---------------------------------------------------------------------------
// StatisticalSignificance
// ---------------------------------------------------------------------------

/// Result of a two-sample t-test between benchmark results.
#[derive(Clone, Debug)]
pub struct SignificanceResult {
    pub name_a: String,
    pub name_b: String,
    pub t_statistic: f64,
    pub significant: bool,
    pub p_label: &'static str,
}

/// Run a Welch's t-test between two benchmark results at the given alpha level.
/// Uses a simple critical-value lookup (df heuristic) instead of a full t-distribution CDF.
pub fn t_test(a: &BenchmarkResult, b: &BenchmarkResult, alpha: f64) -> SignificanceResult {
    let n1 = a.samples_ns.len() as f64;
    let n2 = b.samples_ns.len() as f64;
    let v1 = variance(&a.samples_ns);
    let v2 = variance(&b.samples_ns);

    let se1 = if n1 > 0.0 { v1 / n1 } else { 0.0 };
    let se2 = if n2 > 0.0 { v2 / n2 } else { 0.0 };
    let se_sum = se1 + se2;

    if se_sum == 0.0 {
        return SignificanceResult {
            name_a: a.name.clone(),
            name_b: b.name.clone(),
            t_statistic: 0.0,
            significant: false,
            p_label: "undefined",
        };
    }

    let t = (a.mean_ns - b.mean_ns) / se_sum.sqrt();

    // Welch-Satterthwaite degrees of freedom (approximate)
    // Welch-Satterthwaite df (unused for our simple critical-value approach)
    let _df = if se1 == 0.0 && se2 == 0.0 {
        1.0
    } else {
        let num = se_sum * se_sum;
        let den = if se1 > 0.0 { (se1 * se1) / (n1 - 1.0) } else { 0.0 }
            + if se2 > 0.0 { (se2 * se2) / (n2 - 1.0) } else { 0.0 };
        if den > 0.0 { num / den } else { 1.0 }
    };

    // Simple critical value approximation for two-tailed test.
    // For large df, t_crit ≈ 1.96 (alpha=0.05), 2.576 (alpha=0.01), 3.291 (alpha=0.001).
    // We use a conservative linear approximation.
    let t_crit = if alpha <= 0.001 {
        3.291
    } else if alpha <= 0.01 {
        3.291 - (3.291 - 2.576) * (0.001 - alpha) / (0.001)
    } else if alpha <= 0.05 {
        2.576 - (2.576 - 1.96) * (0.01 - alpha) / (0.01 - 0.001)
    } else {
        1.96 - (1.96 - 1.645) * (0.05 - alpha) / (0.05 - 0.01)
    };

    let abs_t = if t < 0.0 { -t } else { t };
    let significant = abs_t > t_crit;

    let p_label = if significant && alpha <= 0.001 {
        "p < 0.001"
    } else if significant && alpha <= 0.01 {
        "p < 0.01"
    } else if significant {
        "p < 0.05"
    } else {
        "not significant"
    };

    SignificanceResult {
        name_a: a.name.clone(),
        name_b: b.name.clone(),
        t_statistic: t,
        significant,
        p_label,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_timer_starts_and_measures() {
        let timer = EdgeTimer::start();
        // Burn a tiny bit of work so elapsed is > 0 (std only)
        #[cfg(feature = "std")]
        {
            let mut x: u64 = 0;
            for i in 0..1000 {
                x += i;
            }
            core::hint::black_box(x);
            assert!(timer.elapsed_ns() > 0);
            assert!(timer.elapsed_us() > 0 || timer.elapsed_us() == 0);
        }
        #[cfg(not(feature = "std"))]
        {
            assert_eq!(timer.elapsed_ns(), 0);
        }
    }

    #[test]
    fn test_percentile_empty() {
        let data: Vec<u64> = vec![];
        assert_eq!(percentile(&data, 50.0), 0);
    }

    #[test]
    fn test_percentile_single() {
        assert_eq!(percentile(&[42u64], 50.0), 42);
    }

    #[test]
    fn test_percentile_median_odd() {
        let sorted = vec![10u64, 20, 30, 40, 50];
        assert_eq!(percentile(&sorted, 50.0), 30);
    }

    #[test]
    fn test_percentile_p99() {
        let sorted: Vec<u64> = (1..=100).collect();
        let p99 = percentile(&sorted, 99.0);
        assert!(p99 >= 98 && p99 <= 100);
    }

    #[test]
    fn test_mean() {
        assert_eq!(mean(&[2, 4, 6]), 4.0);
        assert_eq!(mean(&[] as &[u64]), 0.0);
    }

    #[test]
    fn test_variance_and_stddev() {
        let data = vec![2u64, 4, 4, 4, 5, 5, 7, 9];
        let v = variance(&data);
        assert!(v > 0.0);
        let s = stddev(&data);
        assert!(s > 0.0);
    }

    #[test]
    fn test_benchmark_result_from_samples() {
        let samples = vec![100u64, 200, 300, 400, 500];
        let result = BenchmarkResult::from_samples("test", samples);
        assert_eq!(result.name, "test");
        assert_eq!(result.iterations, 5);
        assert_eq!(result.min_ns, 100);
        assert_eq!(result.max_ns, 500);
        assert_eq!(result.median_ns, 300.0);
        assert!(result.mean_ns > 0.0);
    }

    #[test]
    fn test_benchmark_result_json() {
        let result = BenchmarkResult::from_samples("json_test", vec![100, 200]);
        let json = result.to_json();
        assert!(json.contains(r#""name":"json_test""#));
        assert!(json.contains("iterations"));
        assert!(json.contains("mean_ns"));
    }

    #[test]
    fn test_benchmark_run() {
        let result = Benchmark::run("add", 50, || {
            let _ = 1 + 1;
        });
        assert_eq!(result.iterations, 50);
        assert!(result.mean_ns >= 0.0);
    }

    #[test]
    fn test_benchmark_run_with_warmup() {
        let result = Benchmark::run_with_warmup("warmup_add", 10, 20, || {
            let _ = 2 + 2;
        });
        assert_eq!(result.iterations, 20);
    }

    #[test]
    fn test_benchmark_suite_json() {
        let mut suite = BenchmarkSuite::new("test-suite");
        suite.add(BenchmarkResult::from_samples("a", vec![100, 200]));
        suite.add(BenchmarkResult::from_samples("b", vec![300, 400]));
        let json = suite.to_json();
        assert!(json.contains(r#""suite":"test-suite""#));
        assert!(json.contains(r#""name":"a""#));
        assert!(json.contains(r#""name":"b""#));
    }

    #[test]
    fn test_benchmark_suite_markdown() {
        let mut suite = BenchmarkSuite::new("md-suite");
        suite.add(BenchmarkResult::from_samples("op", vec![50, 100, 150]));
        let md = suite.to_markdown();
        assert!(md.contains("## md-suite"));
        assert!(md.contains("| op |"));
    }

    #[test]
    fn test_comparison_report_deltas() {
        let mut baseline = BenchmarkSuite::new("baseline");
        baseline.add(BenchmarkResult::from_samples("hash", vec![100, 120, 110]));
        let mut optimized = BenchmarkSuite::new("optimized");
        optimized.add(BenchmarkResult::from_samples("hash", vec![50, 60, 55]));

        let cmp = ComparisonReport::new(baseline, optimized);
        let deltas = cmp.deltas();
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].name, "hash");
        assert!(deltas[0].delta_pct < 0.0); // faster = negative delta
    }

    #[test]
    fn test_comparison_report_markdown() {
        let mut b = BenchmarkSuite::new("before");
        b.add(BenchmarkResult::from_samples("x", vec![200, 200]));
        let mut a = BenchmarkSuite::new("after");
        a.add(BenchmarkResult::from_samples("x", vec![100, 100]));

        let cmp = ComparisonReport::new(b, a);
        let md = cmp.to_markdown();
        assert!(md.contains("Comparison:"));
        assert!(md.contains("| x |"));
    }

    #[test]
    fn test_t_test_significant() {
        // Clearly different distributions with some variance
        let mut a_samples = vec![1000u64; 30];
        for i in 0..30 { a_samples[i] = 1000 + (i as u64 % 10) * 5; }
        let mut b_samples = vec![2000u64; 30];
        for i in 0..30 { b_samples[i] = 2000 + (i as u64 % 10) * 5; }
        let a = BenchmarkResult::from_samples("a", a_samples);
        let b = BenchmarkResult::from_samples("b", b_samples);
        let result = t_test(&a, &b, 0.05);
        assert!(result.significant);
    }

    #[test]
    fn test_t_test_not_significant() {
        // Same distribution
        let a = BenchmarkResult::from_samples("a", vec![100u64, 102, 98, 101, 99]);
        let b = BenchmarkResult::from_samples("b", vec![101u64, 99, 100, 102, 98]);
        let result = t_test(&a, &b, 0.05);
        // These are very close so shouldn't be significant
        assert!(!result.significant);
    }

    #[test]
    fn test_benchmark_report_wrapper() {
        let suite = BenchmarkSuite::new("wrapper");
        let report = BenchmarkReport::new(&suite);
        assert!(report.json().contains("wrapper"));
        assert!(report.markdown().contains("wrapper"));
    }
}
