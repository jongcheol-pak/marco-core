# perf-lab

Standalone `publish = false` performance and regression benchmarking crate for
`marco-core`. Not part of the published library — developer tooling only.

See [`Documentation/TOOLS.md`](../../Documentation/TOOLS.md) for the full
reference (subcommands, options, BenchRecord schema, CI workflow, scripts).

## Quick start

```bash
# End-to-end benchmark (release build recommended)
./tools/perf-lab/scripts/run-bench.sh --release --mode e2e --iterations 30

# Cross-engine comparison: marco-core vs pulldown-cmark vs comrak
./tools/perf-lab/scripts/run-compare.sh --release --mode e2e --iterations 20

# Stress harness
./tools/perf-lab/scripts/run-stress.sh --release --mode e2e --loops 100

# Regression gate (compare two bench artifacts)
./tools/perf-lab/scripts/run-regression.sh --release \
  --baseline tools/perf-lab/output/baseline/bench-baseline.json \
  --current  tools/perf-lab/output/summary/bench-<timestamp>.json

# parallel-render / parallel-parse: on vs off (builds two binaries, diffs via `regression`)
./tools/perf-lab/scripts/run-parallel-compare.sh --release --mode e2e --iterations 20
```

## Subcommands

| Subcommand | Purpose |
|---|---|
| `bench` | Direct-timing benchmark; writes JSON/CSV/Markdown artifacts |
| `stress` | Repeated-loop harness; checks for panics and throughput drift |
| `compare` | In-process speedup table against pulldown-cmark and comrak |
| `report` | Re-renders an artifact or ingests a hyperfine JSON export |
| `regression` | Loads two `BenchRecord` JSON files; exits 1 on regression |

`parallel-render`/`parallel-parse` are compile-time Cargo features, so `bench`
alone can't toggle them per-run. `scripts/run-parallel-compare.sh` builds two
binaries (features off / on) and feeds their `bench` output into `regression`
as the diff engine — see the Quick start example above.

## Engine adapters

| ID | Modes |
|---|---|
| `marco-core` | parse / render / e2e / intelligence |
| `marco-core-raw` | parse / render / e2e (positions, math, diagrams disabled) |
| `pulldown-cmark` | parse / render / e2e |
| `comrak` | parse / render / e2e |
| `markdown-rs` | placeholder |
| `markdown-it-rs` | placeholder |

The `marco-core` adapters render with `RenderOptions::commonmark()`, not
`::default()`. The default applies marco-core's HTML and URL security policies,
so measuring it would fold escaping and scheme classification into the render
number — and would not be comparable with `pulldown-cmark`, which does no such
filtering. It would also silently break comparison against any baseline captured
before that default changed. Policy cost is worth measuring, but as its own
mode rather than mixed into this one.

## Build

```bash
cargo build --manifest-path tools/perf-lab/Cargo.toml --release
cargo test  --manifest-path tools/perf-lab/Cargo.toml
```
