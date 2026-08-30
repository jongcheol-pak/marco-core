//! Extension spec fixture conformance.
//!
//! Loads extension-focused fixtures from `tools/spec/*.json` and asserts
//! strict markdown-to-HTML equality for each case.

use marco_core::{parse, render, RenderOptions};
use serde::Deserialize;

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
    let raw: Vec<RawEntry> = serde_json::from_str(json).expect("fixture: invalid JSON");
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

fn run_fixture_suite(name: &str, json: &str) {
    let cases = load_cases(json);
    assert!(!cases.is_empty(), "{}: no executable fixture cases", name);

    let mut failures = Vec::new();

    for case in &cases {
        let doc = parse(&case.markdown).unwrap_or_else(|e| {
            panic!(
                "{} ex {} [{}] L{:?}-L{:?}: parse error: {}\nmarkdown: {:?}",
                name, case.example, case.section, case.start_line, case.end_line, e, case.markdown
            )
        });
        // `commonmark()`: fixtures assert exact HTML for the parser and
        // renderer, so they must not be filtered by the default security
        // policy. Today no extension fixture contains raw HTML, so `default()`
        // also passed — but only by luck, and the next fixture that does would
        // fail for a reason that has nothing to do with the extension.
        let actual = render(&doc, &RenderOptions::commonmark()).unwrap_or_else(|e| {
            panic!(
                "{} ex {} [{}] L{:?}-L{:?}: render error: {}\nmarkdown: {:?}",
                name, case.example, case.section, case.start_line, case.end_line, e, case.markdown
            )
        });

        if actual != case.expected_html {
            failures.push(format!(
                "{} ex {} [{}] L{:?}-L{:?}\n  markdown: {:?}\n  expected: {:?}\n  actual:   {:?}",
                name,
                case.example,
                case.section,
                case.start_line,
                case.end_line,
                case.markdown,
                case.expected_html,
                actual
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{}: {} of {} cases failed\n{}",
        name,
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

#[test]
#[cfg(feature = "render-diagrams")]
fn test_diagram_fixtures_match_expected_html() {
    run_fixture_suite("diagram", include_str!("../spec/diagram.json"));
}

#[test]
fn test_gfm_fixtures_match_expected_html() {
    run_fixture_suite("gfm", include_str!("../spec/gfm.json"));
}

#[test]
fn test_marco_fixtures_match_expected_html() {
    run_fixture_suite("marco", include_str!("../spec/marco.json"));
}

#[test]
#[cfg(feature = "render-math")]
fn test_math_fixtures_match_expected_html() {
    run_fixture_suite("math", include_str!("../spec/math.json"));
}

#[test]
fn test_combos_fixtures_match_expected_html() {
    run_fixture_suite("combos", include_str!("../spec/combos.json"));
}
