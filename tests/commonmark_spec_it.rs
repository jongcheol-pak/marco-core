//! CommonMark spec conformance.
//!
//! Loads the official CommonMark spec examples (and extension fixture suites)
//! from JSON fixtures and asserts that
//! `marco_core::parse` + `marco_core::render` produce the expected HTML.
//!
//! The fixtures live in `tools/spec/`:
//! - `commonmark.json` — 652 examples from the CommonMark 0.30 test suite.
//! - `diagram.json`    — Mermaid/diagram extension cases.
//! - `gfm.json`        — GFM extension cases.
//! - `marco.json`      — Marco-specific extension cases.
//! - `math.json`       — Math extension cases.
//!
//! ## Strict mode
//!
//! By default the test asserts a **minimum pass rate** (regression guard)
//! rather than 100% conformance, because individual rendering differences
//! (whitespace, attribute ordering, entity escaping) can drift over time.
//! The minimum is configured by [`MIN_COMMONMARK_PASS`] and reflects the
//! current measured baseline.
//!
//! Set `MARCO_SPEC_STRICT=1` to require 100% pass.
//! Set `MARCO_SPEC_VERBOSE=1` to print the first failures to stderr.
//!
//! ## Two test variants
//!
//! - [`commonmark_spec_conformance`] — **strict byte-for-byte** equality
//!   against the spec's expected HTML. This is the real conformance number
//!   and the regression guard the project tracks.
//! - [`commonmark_spec_structural`] — **loose structural match** that only
//!   checks whether the same set of block-level elements (`<h1>`..`<h6>`,
//!   `<p>`, `<pre>`, `<code>`, `<ul>`, `<ol>`, `<blockquote>`, `<hr>`,
//!   `<table>`) appears in expected and actual HTML. This is provided for
//!   parity with the original Marco editor's test runner (which reports
//!   ~100% under this looser rule). It does **not** validate content,
//!   attributes, or escaping — use the strict variant for real coverage.

use marco_core::{parse, render, RenderOptions};
use serde::Deserialize;

/// Minimum number of CommonMark examples (out of 652) that must pass
/// strict HTML comparison. Bump upward as conformance improves; never lower
/// without a documented reason. Current measured baseline: 357 (up from 285
/// after replacing the emphasis/strong scan-and-retry matching with the
/// CommonMark delimiter-stack algorithm — see .dev/parser-render-optimization-plan.md).
const MIN_COMMONMARK_PASS: usize = 350;

#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(default)]
    example: Option<u32>,
    #[serde(default)]
    section: Option<String>,
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    html: Option<String>,
    #[serde(default, rename = "start_line")]
    start_line: Option<u32>,
    #[serde(default, rename = "end_line")]
    end_line: Option<u32>,
}

#[derive(Debug)]
struct SpecCase {
    example: u32,
    section: String,
    markdown: String,
    expected_html: String,
    start_line: Option<u32>,
    end_line: Option<u32>,
}

fn load_cases(json: &str) -> Vec<SpecCase> {
    let raw: Vec<RawEntry> = serde_json::from_str(json).expect("spec fixture: invalid JSON");
    raw.into_iter()
        .filter_map(|e| match (e.example, e.markdown, e.html) {
            (Some(example), Some(markdown), Some(html)) => Some(SpecCase {
                example,
                section: e.section.unwrap_or_default(),
                markdown,
                expected_html: html,
                start_line: e.start_line,
                end_line: e.end_line,
            }),
            _ => None,
        })
        .collect()
}

fn render_case(markdown: &str) -> Result<String, String> {
    let doc = parse(markdown).map_err(|e| format!("parse: {e}"))?;
    // `commonmark()`, not `default()`: the spec includes raw-HTML examples, and
    // the default policy escapes raw HTML. Measuring conformance against a
    // deliberately non-conformant policy would measure the policy, not the
    // parser. `render_byte_identity_it` is what pins this constructor's output.
    let html = render(&doc, &RenderOptions::commonmark()).map_err(|e| format!("render: {e}"))?;
    Ok(html)
}

struct Report {
    passed: usize,
    total: usize,
    failures: Vec<String>,
}

fn run_suite(cases: &[SpecCase]) -> Report {
    let mut passed = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let collect = std::env::var("MARCO_SPEC_VERBOSE").is_ok();

    for case in cases {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_case(&case.markdown)));

        let actual = match result {
            Ok(Ok(html)) => html,
            Ok(Err(err)) => {
                if collect && failures.len() < 20 {
                    failures.push(format!(
                        "ex {} [{}] L{:?}-L{:?} ERROR {}\n  md: {:?}",
                        case.example,
                        case.section,
                        case.start_line,
                        case.end_line,
                        err,
                        case.markdown,
                    ));
                }
                continue;
            }
            Err(_) => {
                if collect && failures.len() < 20 {
                    failures.push(format!(
                        "ex {} [{}] L{:?}-L{:?} PANIC\n  md: {:?}",
                        case.example, case.section, case.start_line, case.end_line, case.markdown,
                    ));
                }
                continue;
            }
        };

        if actual == case.expected_html {
            passed += 1;
        } else if collect && failures.len() < 20 {
            failures.push(format!(
                "ex {} [{}] L{:?}-L{:?}\n  md:       {:?}\n  expected: {:?}\n  actual:   {:?}",
                case.example,
                case.section,
                case.start_line,
                case.end_line,
                case.markdown,
                case.expected_html,
                actual,
            ));
        }
    }

    Report {
        passed,
        total: cases.len(),
        failures,
    }
}

#[test]
fn test_commonmark_spec_matches_expected_html() {
    let json = include_str!("../tools/spec/commonmark.json");
    let cases = load_cases(json);
    assert!(
        cases.len() >= 600,
        "expected ≥600 CommonMark spec cases, got {}",
        cases.len()
    );

    let report = run_suite(&cases);
    let pct = report.passed as f64 / report.total as f64 * 100.0;
    eprintln!(
        "CommonMark spec: {}/{} passed ({:.1}%)",
        report.passed, report.total, pct
    );
    if !report.failures.is_empty() {
        eprintln!("--- first failures ---");
        for f in &report.failures {
            eprintln!("{f}\n");
        }
    }

    let strict = std::env::var("MARCO_SPEC_STRICT").is_ok();
    if strict {
        assert_eq!(
            report.passed,
            report.total,
            "MARCO_SPEC_STRICT=1: {} of {} examples failed",
            report.total - report.passed,
            report.total
        );
    } else {
        assert!(
            report.passed >= MIN_COMMONMARK_PASS,
            "regression: only {} of {} CommonMark examples passed (baseline {})",
            report.passed,
            report.total,
            MIN_COMMONMARK_PASS
        );
    }
}

#[test]
#[cfg(all(feature = "render-diagrams", feature = "render-math"))]
fn test_extension_fixtures_match_expected_html() {
    let suites = [
        ("diagram", include_str!("../tools/spec/diagram.json")),
        ("gfm", include_str!("../tools/spec/gfm.json")),
        ("marco", include_str!("../tools/spec/marco.json")),
        ("math", include_str!("../tools/spec/math.json")),
    ];

    for (name, json) in suites {
        let cases = load_cases(json);
        assert!(!cases.is_empty(), "{name} fixture suite is empty");

        let report = run_suite(&cases);
        let pct = report.passed as f64 / report.total as f64 * 100.0;
        eprintln!(
            "{name} fixtures: {}/{} passed ({:.1}%)",
            report.passed, report.total, pct
        );
        if !report.failures.is_empty() {
            eprintln!("--- {name} failures ---");
            for f in &report.failures {
                eprintln!("{f}\n");
            }
        }

        assert_eq!(
            report.passed,
            report.total,
            "{name}: {} of {} examples failed",
            report.total - report.passed,
            report.total
        );
    }
}

// ---------------------------------------------------------------------------
// Loose structural match (parity with the original Marco editor test runner)
// ---------------------------------------------------------------------------

/// Tags whose presence/absence we compare in the structural variant.
/// This intentionally does NOT inspect text, attributes, or escaping —
/// it only answers "did the parser produce roughly the right shape?".
const STRUCTURAL_TAGS: &[&str] = &[
    "<h1",
    "<h2",
    "<h3",
    "<h4",
    "<h5",
    "<h6",
    "<p",
    "<pre",
    "<code",
    "<ul",
    "<ol",
    "<li",
    "<blockquote",
    "<hr",
    "<table",
];

fn structural_signature(html: &str) -> Vec<bool> {
    STRUCTURAL_TAGS.iter().map(|t| html.contains(t)).collect()
}

fn run_structural_suite(cases: &[SpecCase]) -> Report {
    let mut passed = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let collect = std::env::var("MARCO_SPEC_VERBOSE").is_ok();

    for case in cases {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_case(&case.markdown)));
        let actual = match result {
            Ok(Ok(html)) => html,
            _ => continue, // parse/render errors count as failures
        };

        if structural_signature(&actual) == structural_signature(&case.expected_html) {
            passed += 1;
        } else if collect && failures.len() < 10 {
            failures.push(format!(
                "ex {} [{}] structure mismatch\n  expected sig: {:?}\n  actual sig:   {:?}",
                case.example,
                case.section,
                structural_signature(&case.expected_html),
                structural_signature(&actual),
            ));
        }
    }

    Report {
        passed,
        total: cases.len(),
        failures,
    }
}

#[test]
fn test_commonmark_spec_matches_structural_signature() {
    let json = include_str!("../tools/spec/commonmark.json");
    let cases = load_cases(json);

    let report = run_structural_suite(&cases);
    let pct = report.passed as f64 / report.total as f64 * 100.0;
    eprintln!(
        "CommonMark structural: {}/{} passed ({:.1}%)",
        report.passed, report.total, pct
    );
    if !report.failures.is_empty() {
        eprintln!("--- first structural mismatches ---");
        for f in &report.failures {
            eprintln!("{f}\n");
        }
    }

    // Structural match is the loose parity check. Current measured baseline:
    // 644/652 (98.8%). Allow a small drift below that without failing the
    // build; a sharp drop indicates the parser is producing the wrong
    // block-level shape and should be investigated.
    let min_structural_pct = 97.0;
    assert!(
        pct >= min_structural_pct,
        "structural conformance dropped: {:.1}% < {:.1}%",
        pct,
        min_structural_pct
    );
}
