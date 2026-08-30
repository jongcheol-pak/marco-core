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

/// What the security policies cost.
///
/// The three presets differ only in what the renderer does with raw HTML and
/// with URLs, so running one document through each isolates that cost. Two
/// corpora on purpose:
///
/// - the shared fixtures, which contain little raw HTML and few links, so the
///   policy code barely runs — this measures the *dispatch* overhead;
/// - `POLICY_HEAVY` below, dense in exactly what the policies act on, which
///   measures the work itself.
///
/// Measuring only the first would understate the cost; only the second would
/// overstate it for a typical document.
const POLICY_HEAVY: &str = concat!(
    "<div class=\"note\">A block of raw HTML</div>\n\n",
    "Text with <b>inline</b> <i>html</i> and <span data-x=\"1\">spans</span>.\n\n",
    "[a link](https://example.com/one) and [another](https://example.com/two)\n\n",
    "![img](https://example.com/a.png) ![img2](data:image/png;base64,iVBORw0KGgo=)\n\n",
    "[relative](./local/path) [fragment](#anchor) [mail](mailto:x@example.com)\n\n",
);

fn policy_benches(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy");

    // Repeat so the document is big enough to measure above timer resolution.
    let heavy = POLICY_HEAVY.repeat(200);

    for (corpus_name, input) in [("fixture-large", LARGE), ("policy-heavy", heavy.as_str())] {
        let doc = marco_core::parse(input).expect("parse failed");

        for (policy_name, options) in [
            ("commonmark", marco_core::RenderOptions::commonmark()),
            ("default", marco_core::RenderOptions::new()),
            ("untrusted", marco_core::RenderOptions::untrusted()),
        ] {
            group.bench_function(format!("{corpus_name}/{policy_name}"), |b| {
                b.iter(|| marco_core::render(black_box(&doc), &options).expect("render failed"));
            });
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    parse_benches,
    render_benches,
    e2e_benches,
    policy_benches
);
criterion_main!(benches);
