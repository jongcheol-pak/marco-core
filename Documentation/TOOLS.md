# Tools

Developer tooling for `marco-core`. None are published to crates.io.

## Quick start

```bash
# AST inspection
cargo run --manifest-path tools/marco-ast/Cargo.toml -- --text "# Title" --mode ast

# Quick local benchmark, no extra tooling — see "Root-level cargo bench" below
cargo bench

# Full benchmark (release recommended for fair results)
./tools/perf-lab/scripts/run-bench.sh --release --mode e2e --iterations 30

# Cross-engine comparison
./tools/perf-lab/scripts/run-compare.sh --release --mode e2e --iterations 20

# Stress test
./tools/perf-lab/scripts/run-stress.sh --release --mode e2e --loops 100

# Regression gate — needs a baseline you captured first, see below
./tools/perf-lab/scripts/run-regression.sh --release \
  --baseline tools/perf-lab/output/baseline/bench-baseline.json \
  --current  tools/perf-lab/output/summary/bench-<timestamp>.json
```

**`bench-baseline.json` does not exist until you make it.** Timings are
machine-specific, so baselines are gitignored and never shipped — a fresh clone
has none. Capture one before the regression gate is usable; the script prints
the exact command if you forget.

## Root-level `cargo bench`

`benches/core_benchmarks.rs` (`[[bench]] name = "core_benchmarks"` in the
root `Cargo.toml`) is a small Criterion suite for quick local feedback —
`cargo bench` works directly on the crate, no separate tool invocation
needed. It benchmarks `marco_core::parse`/`render` in `parse`/`render`/`e2e`
groups against `small`/`medium`/`large` plus the two headline pathological
fixtures (`star-pyramid`, `unbalanced-brackets`), reusing the exact same
files under `tools/perf-lab/fixtures/` via `include_str!` rather than
duplicating content. Results land in `target/criterion/` (gitignored,
standard Criterion HTML reports).

```bash
cargo bench                          # all groups
cargo bench --bench core_benchmarks -- parse/small   # filter by name
```

This is deliberately minimal and is **not** the source of truth for this
project's performance tracking — it has no cross-engine comparison, no full
spec-suite corpus, no stress mode, and no CI regression gate. For all of
that, use `tools/perf-lab` (below), which is what `ci-perf.yml` and
`.dev/parser-render-optimization-plan.md`'s numbers are built on.

## marco-ast

AST inspection CLI. Parses a Markdown file and prints the parse tree, HTML
output, diagnostics, or highlights. Useful for debugging grammar and parser
changes.

**Modes:** `ast`, `html`, `both`, `intel`

**Input:** File path or `--text "markdown"` or stdin

**Flags:**
- `--spans` — print line/column/byte ranges
- `--excerpts` — print source excerpts next to nodes
- `--utf8` — print UTF-8 and byte-vs-char diagnostics
- `--time` — print timing summary (sanitize/parse/render/intel)
- `--json` — emit machine-readable JSON report
- `--interactive` — REPL mode (`:mode`, `:clear`, `:help`, `:quit`)

**Example:**
```bash
cargo run --manifest-path tools/marco-ast/Cargo.toml -- \
  path/to/file.md --mode both --spans --time
```

## perf-lab

Performance and regression benchmarking harness. Measures parse, render, and
e2e latency. Compares `marco-core` against `pulldown-cmark` and `comrak`.

**Configuration:** `tools/perf-lab/perf-lab.ron` (workload discovery, output paths, profiles).

**Workloads:** Discovered from `tools/spec/*.json` and `tools/perf-lab/fixtures/*.md`.

### Subcommands

#### `bench` — Direct-timing benchmark

Runs iterations of a workload and computes `mean / median / p95 / stdev`.
Writes JSON/CSV/Markdown artifacts to `output/summary/`.

```bash
perf-lab bench [OPTIONS]
  --engine ENGINE         Engine to bench (default: marco-core)
  --workload ID           Single workload (default: all)
  --mode MODE             parse | render | e2e | intelligence (default: e2e)
  --iterations N          Samples per workload (default: 1)
  --criterion             Use Criterion harness instead of direct timing
  --manifest-out PATH     Write workload manifest
  --check-manifest-drift  Fail if workloads changed vs baseline
  --list                  Print available workloads and exit
```

**Output:** `BenchRecord` JSON with `mean_ns`, `median_ns`, `p95_ns`, `stdev_ns`,
`throughput_bytes_s`, `run_id`, `git_sha`, `engine`, `profile`, etc.

#### `stress` — Repeated-loop harness

Loops many times per workload, hunting for panics, leaks, and throughput
degradation.

```bash
perf-lab stress [OPTIONS]
  --engine ENGINE
  --workload ID
  --mode MODE             (default: e2e)
  --loops N               Iterations per workload (default: 10)
  --continue-on-error     Record errors and continue
  --list
```

#### `compare` — In-process speedup table

Runs multiple engines against the same workloads. First engine is baseline (1.00×).

```bash
perf-lab compare [OPTIONS]
  --engine ENGINE         Repeatable; can specify multiple
  --workload ID
  --mode MODE             (default: e2e)
  --iterations N          Samples per engine (default: 10)
  --list
```

Defaults to `marco-core`, `pulldown-cmark`, `comrak`.

#### `report` — Artifact re-render

Converts a bench artifact to a different format or ingests a hyperfine JSON
export.

```bash
perf-lab report [OPTIONS]
  --input PATH           perf-lab JSON artifact
  --hyperfine-input PATH hyperfine JSON export
  --output PATH          Write to file instead of stdout
  --format FORMAT        json | csv | markdown (default: markdown)
```

#### `regression` — Regression gate

Compares baseline vs current artifacts. Exits 1 if median regresses beyond
`--fail-threshold`.

```bash
perf-lab regression [OPTIONS]
  --baseline PATH        Baseline BenchRecord JSON (required)
  --current PATH         Current run BenchRecord JSON (required)
  --warn-threshold PCT   Warn if > this % regression (default: 10)
  --fail-threshold PCT   Fail if > this % regression (default: 20)
  --min-failures N       Min records to fail on (default: 2)
  --critical-only        Gate only on "critical" workloads — id/profile
                          containing "critical", plus spec:commonmark and
                          fixture:pathological:* explicitly
  --mode MODE            Only compare records in this mode (parse / render /
                          e2e / intelligence); default compares every mode
                          present in both files
```

Matching is exact on `(engine, workload_id, mode)` — a baseline file that
only contains one mode's records will never match a current run in a
different mode, so any script feeding this command two artifacts must make
sure both cover the same mode(s). `ci-perf.yml` merges its e2e and parse
runs into one artifact per side for exactly this reason (see below).

### Engine adapters

| ID | Modes | Notes |
|---|---|---|
| `marco-core` | parse / render / e2e / intelligence | Native; uses full features |
| `marco-core-raw` | parse / render / e2e | Marco-core with positions/math/diagrams disabled |
| `pulldown-cmark` | parse / render / e2e | Uses `Options::all()` for GFM |
| `comrak` | parse / render / e2e | Uses arena allocator |
| `markdown-rs` | — | Placeholder |
| `markdown-it-rs` | — | Placeholder |

### Scripts

All scripts accept `--release` as the first argument.

```bash
./tools/perf-lab/scripts/run-bench.sh [--release] [bench options]
./tools/perf-lab/scripts/run-stress.sh [--release] [stress options]
./tools/perf-lab/scripts/run-compare.sh [--release] [compare options]
./tools/perf-lab/scripts/run-hyperfine.sh [--release] [options]
./tools/perf-lab/scripts/run-regression.sh [--release] [regression options]
./tools/perf-lab/scripts/run-parallel-compare.sh [--release] [bench options] [-- regression options]
```

**`run-parallel-compare.sh`** — `parallel-render`/`parallel-parse` are
compile-time Cargo features, so no single perf-lab binary can toggle them
per-run (`bench` has no `--features` flag). Both are on by default, so
this script builds perf-lab twice — once with
`--no-default-features` plus every other default feature re-enabled
explicitly (the "sequential" binary), once with plain crate defaults (the
"parallel" binary, since that's what defaults already give you) — runs the
same `bench` invocation against each binary, and feeds the two
`BenchRecord` artifacts into `regression` as the diff engine, which already
matches records by `(engine, workload_id, mode)` and prints a %-change table
(no new report format needed). By default the gate thresholds are set high enough
that it never fails/warns — it's a report, not a CI gate, since most
workloads are expected to be ~unchanged (`parallel-parse` only defers
`depth == 0` top-level content; `parallel-render` only helps
code-block-heavy documents) and a tight threshold would flag ordinary noise.
Pass your own thresholds after `--` to opt into gating:

```bash
./tools/perf-lab/scripts/run-parallel-compare.sh --release --mode e2e --iterations 20
./tools/perf-lab/scripts/run-parallel-compare.sh --release --mode parse --iterations 20 \
  --workload fixture:large:paragraph-heavy.md
./tools/perf-lab/scripts/run-parallel-compare.sh --release --mode e2e --iterations 20 \
  -- --warn-threshold 15 --fail-threshold 30 --min-failures 1
```

Labeled artifacts (`parallel-compare-<timestamp>-{sequential,parallel}.json`)
are kept under `output/summary/` for reproducibility.

**`run-hyperfine.sh`** requires `hyperfine >= 1.18`:
```bash
# 20 runs, 5 warmups, release build
./scripts/run-hyperfine.sh --release --runs 20 --warmup 5
```

Outputs to `tools/perf-lab/output/hyperfine/` and auto-invokes `perf-lab report`
to produce Markdown summary.

### Typical workflow

```bash
# Capture a baseline — required once before any regression comparison, since
# baselines are gitignored and a fresh clone has none
./tools/perf-lab/scripts/run-bench.sh --release --mode e2e --iterations 30
cp $(ls -1t tools/perf-lab/output/summary/bench-*.json | head -1) \
   tools/perf-lab/output/baseline/bench-baseline.json

# Later, after changes, compare
./tools/perf-lab/scripts/run-bench.sh --release --mode e2e --iterations 30
./tools/perf-lab/scripts/run-regression.sh --release \
  --baseline tools/perf-lab/output/baseline/bench-baseline.json \
  --current  $(ls -1t tools/perf-lab/output/summary/bench-*.json | head -1)
```

## Building & testing

```bash
# marco-ast
cargo build --manifest-path tools/marco-ast/Cargo.toml
cargo test  --manifest-path tools/marco-ast/Cargo.toml

# root-level cargo bench (see "Root-level cargo bench" above)
cargo bench

# perf-lab
cargo build --manifest-path tools/perf-lab/Cargo.toml --release
cargo test  --manifest-path tools/perf-lab/Cargo.toml

# Extension spec conformance (declared in the root Cargo.toml, not perf-lab's —
# the [[test]] entry points at tools/tests/extension_spec_it.rs)
cargo test --test extension_spec_it
```

## CI — `.github/workflows/ci-perf.yml`

Two-stage performance workflow:

- **Push to `main`:** Run `bench --mode e2e --iterations 30` and
  `bench --mode parse --iterations 30`, then merge both runs' JSON with
  `jq -s add` into one artifact (30-day retention, keyed by commit SHA).
  Merging matters because `regression` matches records by
  `(engine, workload_id, mode)` exactly — a baseline containing only one
  mode would silently fail to match a current run in a different mode.
- **Pull request:** Run `bench --iterations 20` in both modes on the PR
  branch, merge the same way, download the base-SHA artifact, then run
  **two** gates:
  - the broad gate — `regression --warn-threshold 10 --fail-threshold 20
    --min-failures 2` — fails if 2+ workloads (any of them, any mode)
    regress past 20%.
  - the critical-workload gate — `regression --critical-only --mode parse
    --warn-threshold 10 --fail-threshold 20 --min-failures 1` — fails if
    `spec:commonmark` or any `fixture:pathological:*` workload regresses
    past 20% in parse time *on its own*. These are the workloads
    `.dev/parser-render-optimization-plan.md` brought down from
    ~70-212x slower than comparable engines to within a few x; this gate
    is what actually satisfies that plan's Phase 5 goal ("CI fails a PR
    that regresses spec:commonmark or fixture:pathological parse time by
    >20%") — the broad gate alone doesn't, since it needs a second,
    unrelated regression to also trip.

  Both gates skip gracefully if no baseline artifact exists (first run, or
  after the 30-day artifact TTL expires).
