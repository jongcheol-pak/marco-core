#!/usr/bin/env bash
# run-regression.sh — compare a current perf-lab JSON artifact against a baseline.
#
# Usage:
#   ./scripts/run-regression.sh --baseline PATH --current PATH [options]
#
# Options:
#   --baseline PATH     Baseline perf-lab JSON artifact (required)
#   --current PATH      Current run perf-lab JSON artifact (required)
#   --warn PCT          Warn threshold % (default: 10)
#   --fail PCT          Fail threshold % (default: 20)
#   --min-failures N    Min regressions to hard-fail (default: 2)
#   --critical-only     Gate only on critical workloads (spec:commonmark,
#                        fixture:pathological:*, plus anything tagged
#                        "critical" in its id/profile)
#   --mode MODE         Only compare records in this mode
#                        (parse|render|e2e|intelligence); default compares
#                        every mode present in both files
#   --release           Build in release mode
#   -h, --help          Show this help
#
# Note: `--baseline`/`--current` must both cover the same mode(s) — matching
# is exact on (engine, workload_id, mode), so a baseline missing a mode the
# current run has (or vice versa) silently skips those records rather than
# comparing them. If you benchmarked in more than one mode, merge the JSON
# arrays first (e.g. `jq -s add e2e.json parse.json > merged.json`) — this is
# what ci-perf.yml does before calling this same regression check.
#
# Quick start (run a fresh bench first, then compare to stored baseline):
#   ./scripts/run-bench.sh --release --mode e2e --iterations 30
#   LATEST=$(ls -1t tools/perf-lab/output/summary/bench-*.json | head -1)
#   ./scripts/run-regression.sh \
#     --baseline tools/perf-lab/output/baseline/bench-baseline.json \
#     --current "$LATEST"
#
# Reproduce the CI critical-workload gate locally (parse mode only, fails on
# a single regressed spec:commonmark/fixture:pathological:* workload):
#   ./scripts/run-regression.sh \
#     --baseline tools/perf-lab/output/baseline/bench-baseline.json \
#     --current "$LATEST" \
#     --critical-only --mode parse --min-failures 1
#
# Capture a new baseline:
#   cp "$LATEST" tools/perf-lab/output/baseline/bench-baseline.json

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../" && pwd)"
cd "$ROOT"

BASELINE=""
CURRENT=""
WARN="10"
FAIL_T="20"
MIN_FAILURES="2"
CRITICAL_ONLY=""
MODE=""
RELEASE_FLAG=""
PROFILE="debug"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline)     BASELINE="$2";    shift 2 ;;
    --current)      CURRENT="$2";     shift 2 ;;
    --warn)         WARN="$2";        shift 2 ;;
    --fail)         FAIL_T="$2";      shift 2 ;;
    --min-failures) MIN_FAILURES="$2"; shift 2 ;;
    --critical-only) CRITICAL_ONLY="--critical-only"; shift ;;
    --mode)         MODE="$2";        shift 2 ;;
    --release)      RELEASE_FLAG="--release"; PROFILE="release"; shift ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \?//'
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$BASELINE" || -z "$CURRENT" ]]; then
  echo "ERROR: --baseline and --current are both required." >&2
  echo "Run with -h for usage." >&2
  exit 1
fi

# Check the files exist before building. A baseline is not shipped -- it is
# captured locally from a run you trust -- so a missing one is the expected
# first-time experience, and deserves a better message than a serde error
# after a release build.
if [[ ! -f "$BASELINE" ]]; then
  echo "ERROR: baseline not found: $BASELINE" >&2
  echo >&2
  echo "Baselines are generated, not checked in (see tools/perf-lab/output/.gitignore)." >&2
  echo "Capture one from a run you trust:" >&2
  echo >&2
  echo "  ./tools/perf-lab/scripts/run-bench.sh --release --mode e2e --iterations 30" >&2
  echo "  cp \"\$(ls -1t tools/perf-lab/output/summary/bench-*.json | head -1)\" \\" >&2
  echo "     $BASELINE" >&2
  echo >&2
  echo "Existing local baselines:" >&2
  ls -1 tools/perf-lab/output/baseline/*.json 2>/dev/null | sed 's/^/  /' >&2 || echo "  (none)" >&2
  exit 1
fi

if [[ ! -f "$CURRENT" ]]; then
  echo "ERROR: current run not found: $CURRENT" >&2
  echo "Produce one with: ./tools/perf-lab/scripts/run-bench.sh --release --mode e2e" >&2
  exit 1
fi

MODE_FLAG=""
[[ -n "$MODE" ]] && MODE_FLAG="--mode $MODE"

cargo build --manifest-path tools/perf-lab/Cargo.toml $RELEASE_FLAG 2>&1 | tail -2

"tools/perf-lab/target/${PROFILE}/perf-lab" regression \
  --baseline "$BASELINE" \
  --current  "$CURRENT" \
  --warn-threshold "$WARN" \
  --fail-threshold "$FAIL_T" \
  --min-failures   "$MIN_FAILURES" \
  $CRITICAL_ONLY \
  $MODE_FLAG
