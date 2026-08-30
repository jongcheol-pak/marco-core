//! Byte-identity baseline for `RenderOptions::commonmark()`.
//!
//! The security policies are opt-in by shape: `commonmark()` must keep emitting
//! exactly what marco-core 1.3.3 emitted, so the safe default can be shown to be
//! *only* a default change rather than a change in what the renderer is capable
//! of. This test renders every deterministic spec fixture and compares the
//! result against a committed baseline, byte for byte.
//!
//! It is not a conformance test. The baseline records what this crate *does*,
//! including where that differs from the CommonMark spec — `commonmark_spec_it`
//! is what measures the gap. A diff here means rendering changed, nothing more
//! and nothing less.
//!
//! # Regenerating
//!
//! ```sh
//! MARCO_UPDATE_BASELINE=1 cargo test --test render_byte_identity_it
//! ```
//!
//! Only regenerate when a rendering change is *intended*, and review the diff:
//! every changed line is a change in what consumers receive.
//!
//! # What is excluded, and why
//!
//! - **`diagram.json`** — Mermaid SVG embeds computed text metrics
//!   (`width="331.19427"`), which depend on the fonts available on the machine.
//!   A committed baseline would fail on a different host for reasons unrelated
//!   to this crate. Diagram output is pinned by unit tests in
//!   `render::diagram` instead.
//! - **Non-default feature sets** — the baseline is generated with syntax
//!   highlighting and math enabled, so the test compiles only when those
//!   features are on.

#![cfg(all(feature = "render-syntax-highlighting", feature = "render-math"))]

use marco_core::{parse, render, RenderOptions};
use serde::{Deserialize, Serialize};

const BASELINE_PATH: &str = "tests/golden/commonmark_baseline.json";

/// Fixture suites whose rendered output is deterministic across machines.
const SUITES: &[(&str, &str)] = &[
    ("commonmark", include_str!("../tools/spec/commonmark.json")),
    ("gfm", include_str!("../tools/spec/gfm.json")),
    ("marco", include_str!("../tools/spec/marco.json")),
    ("combos", include_str!("../tools/spec/combos.json")),
    ("math", include_str!("../tools/spec/math.json")),
];

#[derive(Debug, Deserialize)]
struct RawEntry {
    #[serde(default)]
    example: Option<u32>,
    #[serde(default)]
    markdown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BaselineEntry {
    suite: String,
    example: u32,
    /// `Ok` HTML, or the rendered error message. Both are part of the contract:
    /// a fixture that fails to parse today must keep failing the same way.
    outcome: String,
}

fn render_all() -> Vec<BaselineEntry> {
    let options = RenderOptions::commonmark();
    let mut entries = Vec::new();

    for (suite, json) in SUITES {
        let raw: Vec<RawEntry> =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("{suite}: invalid JSON: {e}"));

        for entry in raw {
            let (Some(example), Some(markdown)) = (entry.example, entry.markdown) else {
                continue; // comment/metadata entries carry neither
            };

            let outcome = match parse(&markdown) {
                Ok(doc) => match render(&doc, &options) {
                    Ok(html) => html,
                    Err(e) => format!("<render error: {e}>"),
                },
                Err(e) => format!("<parse error: {e}>"),
            };

            entries.push(BaselineEntry {
                suite: (*suite).to_string(),
                example,
                outcome,
            });
        }
    }

    entries
}

#[test]
fn commonmark_output_matches_the_committed_baseline() {
    let current = render_all();

    if std::env::var("MARCO_UPDATE_BASELINE").is_ok() {
        let encoded = serde_json::to_string_pretty(&current).expect("baseline should serialize");
        std::fs::create_dir_all("tests/golden").expect("create tests/golden");
        std::fs::write(BASELINE_PATH, encoded).expect("write baseline");
        eprintln!("baseline updated: {} entries", current.len());
        return;
    }

    let raw = std::fs::read_to_string(BASELINE_PATH).unwrap_or_else(|e| {
        panic!(
            "{BASELINE_PATH} is missing ({e}). \
             Generate it with MARCO_UPDATE_BASELINE=1 cargo test --test render_byte_identity_it"
        )
    });
    let baseline: Vec<BaselineEntry> =
        serde_json::from_str(&raw).expect("baseline should be valid JSON");

    assert_eq!(
        baseline.len(),
        current.len(),
        "fixture count changed ({} -> {}); regenerate the baseline if that was intended",
        baseline.len(),
        current.len()
    );

    let mut drifted = Vec::new();
    for (expected, actual) in baseline.iter().zip(current.iter()) {
        assert_eq!(
            (&expected.suite, expected.example),
            (&actual.suite, actual.example),
            "baseline is out of order; regenerate it"
        );

        if expected.outcome != actual.outcome {
            drifted.push((expected, actual));
        }
    }

    if drifted.is_empty() {
        return;
    }

    let mut report = format!(
        "{} of {} fixtures render differently under RenderOptions::commonmark().\n\
         This constructor must stay byte-identical to marco-core 1.3.3.\n\n",
        drifted.len(),
        current.len()
    );

    // Three is enough to diagnose; the count above conveys the scale.
    for (expected, actual) in drifted.iter().take(3) {
        report.push_str(&format!(
            "--- {}/{} ---\nexpected: {:?}\nactual:   {:?}\n\n",
            expected.suite, expected.example, expected.outcome, actual.outcome
        ));
    }

    if drifted.len() > 3 {
        report.push_str(&format!("... and {} more\n", drifted.len() - 3));
    }

    panic!("{report}");
}

/// The baseline is worthless if it was generated from an options value that
/// already had a policy applied, so pin what it was generated with.
#[test]
fn baseline_is_generated_with_permissive_policies() {
    use marco_core::{HtmlPolicy, UrlPolicy};

    let options = RenderOptions::commonmark();
    assert_eq!(options.html, HtmlPolicy::Allow);
    assert_eq!(options.urls, UrlPolicy::Allow);
}

/// A canary: raw HTML and a `javascript:` link must still pass through under
/// `commonmark()`. If either stops, the baseline comparison would still pass
/// only because the baseline was regenerated — this fails loudly instead.
#[test]
fn commonmark_still_emits_raw_html_and_unfiltered_urls() {
    let doc = parse("<b>bold</b> and [x](javascript:alert(1))\n").expect("parse");
    let html = render(&doc, &RenderOptions::commonmark()).expect("render");

    assert!(html.contains("<b>bold</b>"), "raw HTML was altered: {html}");
    assert!(
        html.contains("javascript:alert(1"),
        "URL was filtered: {html}"
    );
}
