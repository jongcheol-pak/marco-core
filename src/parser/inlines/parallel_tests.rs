//! Regression tests for the `parallel-parse` feature (Phase 4): deferring
//! inline parsing of paragraphs, table cells, definition-list terms, and
//! footnote-definition bodies so they can be resolved in one shared,
//! order-preserving rayon batch instead of eagerly during block scanning.
//!
//! The whole file compiles to nothing without the feature, so a plain
//! `cargo test` (no `--features parallel-parse`) silently skips it — that's
//! correct, there's nothing to test in the default build since none of the
//! new code paths exist there.
//!
//! Each test independently recomputes its expected inline children by
//! calling the same, untouched, always-available inline-parsing primitives
//! (`parser::inlines::parse_inlines_from_span` / `parser::parse_inlines`)
//! directly on the same raw text — not a hardcoded golden AST literal, so
//! these aren't brittle against unrelated AST changes elsewhere. This is
//! the same pattern used for the Phase 3 `parallel-render` node-address-key
//! regression test.
//!
//! # Why this lives inside the crate
//!
//! It calls `parse_inlines_from_span`, whose argument is a `GrammarSpan` —
//! `nom_locate::LocatedSpan`. Re-exporting that to let an integration test
//! reach it would put `nom` in the public API and make swapping the parser
//! library a breaking change. A unit test is inside the privacy boundary and
//! needs no such exposure, so the test moved rather than the API widening.

use crate::parser::inlines::parse_inlines_from_span;
use crate::parser::shared::GrammarSpan;
use crate::parser::{parse, parse_with_options, Node, NodeKind, ParseOptions};

/// Recursively assert two nodes are structurally identical: same `kind`
/// (compared via `Debug`, since `NodeKind` has no `PartialEq`) and
/// recursively identical `children`.
///
/// Deliberately does *not* compare `span`: "expected" trees in this file
/// are built by re-parsing an extracted substring standalone (starting a
/// fresh `GrammarSpan` at offset 0), so their positions are relative to
/// that substring, not to the substring's real location in the full
/// document — an artifact of this file's comparison methodology, not
/// something under test here. Absolute-position correctness under
/// `parallel-parse` is covered separately by
/// `track_positions_false_produces_no_spans_anywhere_under_parallel_parse`
/// (which checks the one span property that actually matters for the
/// thread-local propagation risk: `None` stays `None`).
fn assert_nodes_equal(actual: &Node, expected: &Node, path: &str) {
    assert_eq!(
        format!("{:?}", actual.kind),
        format!("{:?}", expected.kind),
        "kind mismatch at {path}"
    );
    assert_children_equal(&actual.children, &expected.children, path);
}

fn assert_children_equal(actual: &[Node], expected: &[Node], path: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "children count mismatch at {path}: actual={actual:?} expected={expected:?}"
    );
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_nodes_equal(a, e, &format!("{path}/{i}"));
    }
}

/// Recursively collect every node in `root` (including `root` itself)
/// matching `pred`, in document order.
fn collect_matching<'a>(root: &'a Node, pred: &dyn Fn(&NodeKind) -> bool, out: &mut Vec<&'a Node>) {
    if pred(&root.kind) {
        out.push(root);
    }
    for child in &root.children {
        collect_matching(child, pred, out);
    }
}

fn expected_inline_children(text: &str) -> Vec<Node> {
    parse_inlines_from_span(GrammarSpan::new(text)).expect("expected-side inline parse failed")
}

#[test]
fn deferred_paragraph_matches_direct_inline_parse() {
    let input = "This has **bold**, *emphasis*, and a [link](https://example.com).\n";
    let doc = parse(input).expect("parse failed");

    let mut paragraphs = Vec::new();
    for node in &doc.children {
        collect_matching(node, &|k| matches!(k, NodeKind::Paragraph), &mut paragraphs);
    }
    assert_eq!(paragraphs.len(), 1, "expected exactly one paragraph");

    let expected = expected_inline_children(
        "This has **bold**, *emphasis*, and a [link](https://example.com).",
    );
    assert_children_equal(&paragraphs[0].children, &expected, "paragraph");
}

#[test]
fn deferred_paragraph_with_checkbox_markers_matches_sequential_shape() {
    // Task-checkbox markers split the paragraph into literal
    // TaskCheckboxInline nodes interleaved with inline-parsed text spans —
    // the trickiest case for the paragraph `Segment` design.
    let input = "[ ] first *item*\n[x] second **item**\n";
    let doc = parse(input).expect("parse failed");

    let mut paragraphs = Vec::new();
    for node in &doc.children {
        collect_matching(node, &|k| matches!(k, NodeKind::Paragraph), &mut paragraphs);
    }
    assert_eq!(paragraphs.len(), 1);
    let children = &paragraphs[0].children;

    // TaskCheckboxInline, text("first "), Emphasis("item"), TaskCheckboxInline, text("second "), Strong("item")
    assert!(matches!(
        children[0].kind,
        NodeKind::TaskCheckboxInline { checked: false }
    ));
    let checkbox_positions: Vec<usize> = children
        .iter()
        .enumerate()
        .filter(|(_, n)| matches!(n.kind, NodeKind::TaskCheckboxInline { .. }))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(checkbox_positions.len(), 2, "expected two checkbox markers");
    assert!(matches!(
        children[checkbox_positions[1]].kind,
        NodeKind::TaskCheckboxInline { checked: true }
    ));

    // The text between/after the markers should match direct inline parses
    // of the same raw segments.
    let after_first = expected_inline_children("first *item*\n");
    assert_children_equal(
        &children[checkbox_positions[0] + 1..checkbox_positions[1]],
        &after_first,
        "segment after first checkbox",
    );
    let after_second = expected_inline_children("second **item**");
    assert_children_equal(
        &children[checkbox_positions[1] + 1..],
        &after_second,
        "segment after second checkbox",
    );
}

#[test]
fn deferred_paragraph_inside_blockquote_matches_direct_inline_parse() {
    let input = "> This has **bold** inside a blockquote.\n";
    let doc = parse(input).expect("parse failed");

    let mut paragraphs = Vec::new();
    collect_matching(
        &doc.children[0],
        &|k| matches!(k, NodeKind::Paragraph),
        &mut paragraphs,
    );
    assert_eq!(paragraphs.len(), 1);

    let expected = expected_inline_children("This has **bold** inside a blockquote.");
    assert_children_equal(&paragraphs[0].children, &expected, "blockquote paragraph");
}

#[test]
fn deferred_paragraph_inside_nested_list_item_matches_direct_inline_parse() {
    let input = "- outer\n  - inner with *emphasis* and `code`\n";
    let doc = parse(input).expect("parse failed");

    let mut paragraphs = Vec::new();
    for node in &doc.children {
        collect_matching(node, &|k| matches!(k, NodeKind::Paragraph), &mut paragraphs);
    }
    assert_eq!(
        paragraphs.len(),
        2,
        "expected an outer and an inner paragraph"
    );

    // Match by rendered text content rather than assuming a fixed index,
    // since tight-list AST shape isn't the thing under test here.
    let inner = paragraphs
        .iter()
        .find(|p| {
            p.children
                .iter()
                .any(|c| matches!(&c.kind, NodeKind::Text(t) if t.contains("inner with ")))
        })
        .expect("inner paragraph not found");

    let expected = expected_inline_children("inner with *emphasis* and `code`");
    assert_children_equal(&inner.children, &expected, "nested list item paragraph");
}

#[test]
fn deferred_table_cells_match_direct_inline_parse() {
    let input = "| a | b |\n|---|---|\n| **bold** | [link](https://example.com) |\n";
    let doc = parse(input).expect("parse failed");

    assert_eq!(doc.children.len(), 1);
    let table = &doc.children[0];
    assert!(matches!(table.kind, NodeKind::Table { .. }));
    // header row + 1 body row
    assert_eq!(table.children.len(), 2);
    let body_row = &table.children[1];
    assert_eq!(body_row.children.len(), 2);

    assert_children_equal(
        &body_row.children[0].children,
        &expected_inline_children("**bold**"),
        "cell 0",
    );
    assert_children_equal(
        &body_row.children[1].children,
        &expected_inline_children("[link](https://example.com)"),
        "cell 1",
    );
}

#[test]
fn deferred_headerless_table_cells_match_direct_inline_parse() {
    let input = "|:--|--:|\n| *a* | **b** |\n";
    let doc = parse(input).expect("parse failed");

    assert_eq!(doc.children.len(), 1);
    let table = &doc.children[0];
    assert!(matches!(table.kind, NodeKind::Table { .. }));
    assert_eq!(table.children.len(), 1); // no header row
    let row = &table.children[0];

    assert_children_equal(
        &row.children[0].children,
        &expected_inline_children("*a*"),
        "cell 0",
    );
    assert_children_equal(
        &row.children[1].children,
        &expected_inline_children("**b**"),
        "cell 1",
    );
}

#[test]
fn deferred_definition_term_matches_direct_inline_parse() {
    let input = "A **bold** term\n: A definition.\n";
    let doc = parse(input).expect("parse failed");

    assert_eq!(doc.children.len(), 1);
    let dl = &doc.children[0];
    assert!(matches!(dl.kind, NodeKind::DefinitionList));
    let term = &dl.children[0];
    assert!(matches!(term.kind, NodeKind::DefinitionTerm));

    assert_children_equal(
        &term.children,
        &expected_inline_children("A **bold** term"),
        "definition term",
    );
}

#[test]
fn deferred_footnote_definition_body_matches_direct_inline_parse() {
    let input =
        "See[^a] for details.\n\n[^a]: A note with **bold** and a [link](https://example.com).\n";
    let doc = parse(input).expect("parse failed");

    let mut footnote_defs = Vec::new();
    for node in &doc.children {
        collect_matching(
            node,
            &|k| matches!(k, NodeKind::FootnoteDefinition { .. }),
            &mut footnote_defs,
        );
    }
    assert_eq!(footnote_defs.len(), 1);
    let def = footnote_defs[0];
    assert_eq!(def.children.len(), 1);
    let paragraph = &def.children[0];
    assert!(matches!(paragraph.kind, NodeKind::Paragraph));

    // Footnote-definition bodies are parsed via the owned-`&str` variant
    // (`parser::parse_inlines`), not `parse_inlines_from_span` — content is
    // hand-assembled, not a contiguous span. Same expected output either
    // way for this single-line case.
    let expected =
        crate::parser::parse_inlines("A note with **bold** and a [link](https://example.com).")
            .expect("expected-side inline parse failed");
    assert_children_equal(&paragraph.children, &expected, "footnote definition body");
}

/// Root cause this guards against: `ParseOptions` (`track_positions` /
/// `parse_math` / `parse_diagrams`) are propagated via thread-locals set
/// once on the calling thread. A rayon worker thread that never explicitly
/// re-installs them would otherwise silently see the compiled-in defaults
/// (all `true`) regardless of what the caller asked for — meaning spans
/// would incorrectly be computed here if the `ParseOptionsGuard`
/// re-installation in `parallel_inline::resolve_pending_batch` were ever
/// dropped in a future edit.
#[test]
fn track_positions_false_produces_no_spans_anywhere_under_parallel_parse() {
    let input = "# Heading\n\nA paragraph with **bold** and a [link](https://example.com).\n\n\
                 | a | b |\n|---|---|\n| *x* | *y* |\n\n\
                 Term\n: A definition with `code`.\n\n\
                 See[^a].\n\n[^a]: Note with **bold**.\n";

    let opts = ParseOptions {
        track_positions: false,
        ..ParseOptions::default()
    };
    let doc = parse_with_options(input, opts).expect("parse failed");

    fn assert_no_spans(node: &Node) {
        assert!(
            node.span.is_none(),
            "expected span: None with track_positions: false, found {:?} on {:?}",
            node.span,
            node.kind
        );
        for child in &node.children {
            assert_no_spans(child);
        }
    }

    for node in &doc.children {
        assert_no_spans(node);
    }
}
