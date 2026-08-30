//! `cargo bench` entry point for marco-core itself.
//!
//! This is a small, self-contained Criterion suite for quick local feedback
//! (`cargo bench`, no extra tooling). For cross-engine comparison against
//! pulldown-cmark/comrak, the full CommonMark/GFM/Marco spec-suite corpus,
//! stress testing, and CI regression gating, use `tools/perf-lab` instead
//! (see `Documentation/TOOLS.md`) — that crate is the source of truth for
//! this project's performance tracking. The fixtures below are the same
//! files `tools/perf-lab` benchmarks, reused here (not duplicated) so the
//! two stay comparable.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

const SMALL: &str = include_str!("../tools/perf-lab/fixtures/small/generated-synthetic.md");
const MEDIUM: &str = include_str!("../tools/perf-lab/fixtures/medium/generated-synthetic.md");
const LARGE: &str = include_str!("../tools/perf-lab/fixtures/large/generated-synthetic.md");
const STAR_PYRAMID: &str = include_str!("../tools/perf-lab/fixtures/pathological/star-pyramid.md");
const UNBALANCED_BRACKETS: &str =
    include_str!("../tools/perf-lab/fixtures/pathological/unbalanced-brackets.md");

fn parse_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse");
    for (name, input) in [
        ("small", SMALL),
        ("medium", MEDIUM),
        ("large", LARGE),
        ("pathological/star-pyramid", STAR_PYRAMID),
        ("pathological/unbalanced-brackets", UNBALANCED_BRACKETS),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| marco_core::parse(black_box(input)).expect("parse failed"));
        });
    }
    group.finish();
}

fn render_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("render");
    for (name, input) in [("small", SMALL), ("medium", MEDIUM), ("large", LARGE)] {
        let doc = marco_core::parse(input).expect("parse failed");
        // `commonmark()`, not `default()`: the default applies the security
        // policies, so benchmarking it would measure escaping and URL
        // classification alongside rendering — and would silently stop being
        // comparable with every result recorded before the default flipped.
        // Policy cost belongs in its own benchmark, not folded into this one.
        let options = marco_core::RenderOptions::commonmark();
        group.bench_function(name, |b| {
            b.iter(|| marco_core::render(black_box(&doc), &options).expect("render failed"));
        });
    }
    group.finish();
}

fn e2e_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("e2e");
    // See `render_benches`: `commonmark()` keeps this comparable across the
    // default flip.
    let options = marco_core::RenderOptions::commonmark();
    for (name, input) in [("small", SMALL), ("medium", MEDIUM), ("large", LARGE)] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let doc = marco_core::parse(black_box(input)).expect("parse failed");
                marco_core::render(&doc, &options).expect("render failed")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, parse_benches, render_benches, e2e_benches);
criterion_main!(benches);
