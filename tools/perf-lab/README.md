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

# What the security policies cost (render/e2e only)
./tools/perf-lab/scripts/run-compare.sh --release --mode render --iterations 100 \
  --engine marco-core --engine marco-core-safe --engine marco-core-untrusted
```

## Baselines

**`bench-baseline.json` does not exist until you create it.** Timings are
machine-specific, so baselines are gitignored and never shipped — comparing your
laptop against someone else's CI runner measures the hardware, not the change.

```bash
./tools/perf-lab/scripts/run-bench.sh --release --mode e2e --iterations 30
cp "$(ls -1t tools/perf-lab/output/summary/bench-*.json | head -1)" \
   tools/perf-lab/output/baseline/bench-baseline.json
```

`run-regression.sh` prints this command if the baseline is missing, along with
any named baselines already on disk.

Matching is exact on `(engine, workload_id, mode)`, and non-matching records are
**skipped silently**. A single-mode baseline compared against a multi-mode run
therefore checks almost nothing while still reporting "gate passed". Merge modes
first if you benchmarked more than one:

```bash
jq -s add e2e.json parse.json render.json > merged.json
```

`output/baseline/workload-manifest-v1.json` is the exception to the gitignore:
it is reference data, not a timing, and `perf-lab.ron` points `baseline_manifest`
at it so runs report workload drift. Regenerate it when fixtures change:

```bash
perf-lab bench --engine marco-core --mode parse --iterations 1 \
  --manifest-out tools/perf-lab/output/baseline/workload-manifest-v1.json
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

`marco-core` and `marco-core-raw` render with `RenderOptions::commonmark()`, not
`::default()`. The default applies the HTML and URL security policies, so
measuring it would fold escaping and scheme classification into the render
number, would not be comparable with `pulldown-cmark` (which does no such
filtering), and would silently break comparison against any baseline captured
before that default changed.

`marco-core-safe` and `marco-core-untrusted` exist to measure that cost
deliberately. They support **render and e2e only** — render policy does not
affect parsing or analysis, so they reject `parse` and `intelligence` rather
than reporting `marco-core`'s number under another name.

⚠️ **`spec:*` workloads are the wrong tool for measuring policy cost.**
perf-lab feeds each fixture file to the parser whole, as Markdown, so
`spec:gfm` is a JSON file that parses into one paragraph of text — with no
`Link`, `Image`, `HtmlBlock` or `InlineHtml` nodes for a policy to act on. For
that measurement use `cargo bench --bench core_benchmarks -- policy`, whose
`policy-heavy` corpus is built from the constructs the policies actually touch.

## Build

```bash
cargo build --manifest-path tools/perf-lab/Cargo.toml --release
cargo test  --manifest-path tools/perf-lab/Cargo.toml
```
