//! Heading parser - converts grammar output to AST nodes
//!
//! Handles conversion of both ATX headings (# Header) and Setext headings (underline style)
//! from grammar layer to parser AST representation.

use super::shared::{opt_span, opt_span_range, GrammarSpan};
use crate::parser::ast::{Node, NodeKind};

/// Parse an ATX heading (# Header) into an AST node.
///
/// # Arguments
/// * `level` - Heading level (1-6)
/// * `content` - The heading text content from grammar layer
///
/// # Returns
/// A Node with NodeKind::Heading
///
/// # Note
/// The span includes only the heading text content, not the # markers.
/// For full-line highlighting including markers, the intelligence layer should use
/// the full line span.
///
/// # Example
/// ```
/// use marco_core::parser::blocks::cm_heading_parser::parse_atx_heading;
/// use marco_core::parser::shared::GrammarSpan;
/// use marco_core::NodeKind;
///
/// let content = GrammarSpan::new("Hello World");
/// let node = parse_atx_heading(1, content);
/// assert!(matches!(node.kind, NodeKind::Heading { level: 1, .. }));
/// ```
pub fn parse_atx_heading(level: u8, content: GrammarSpan) -> Node {
    let span = opt_span(content);
    let (text, id) = split_extended_heading_id(content.fragment());
    let children = heading_inlines(content, &text);

    Node {
        kind: NodeKind::Heading { level, text, id },
        span,
        children,
    }
}

/// Parse a heading's raw content as inline content (CommonMark §4.2 / §4.3).
///
/// `text` is the heading content with any trailing `{#id}` suffix removed, and is
/// always a prefix of `content` once trailing whitespace is discounted, so it can
/// be sliced off `content` by byte length to keep nom_locate offsets intact.
fn heading_inlines(content: GrammarSpan, text: &str) -> Vec<Node> {
    use nom::Input;

    let inline_span = if text.len() < content.fragment().len() {
        content.take(text.len())
    } else {
        content
    };

    crate::parser::inlines::parse_inlines_from_span(inline_span).unwrap_or_else(|e| {
        log::warn!("Heading inline parsing failed, falling back to plain text: {e}");
        Vec::new()
    })
}

/// Parse a Setext heading (underline style) into an AST node.
///
/// # Arguments
/// * `level` - Heading level (1 for === underline, 2 for --- underline)
/// * `content` - The heading text content from grammar layer
///
/// # Returns
/// A Node with NodeKind::Heading
///
/// # Example
/// ```
/// use marco_core::grammar::setext_heading;
/// use marco_core::parser::blocks::cm_heading_parser::parse_setext_heading;
/// use marco_core::parser::shared::GrammarSpan;
/// use marco_core::NodeKind;
///
/// let start = GrammarSpan::new("Hello\n===\n");
/// let (rest, (level, content)) = setext_heading(start).unwrap();
/// let node = parse_setext_heading(level, content, start, rest);
/// assert!(matches!(node.kind, NodeKind::Heading { level: 1, .. }));
/// ```
pub fn parse_setext_heading(
    level: u8,
    content: GrammarSpan,
    full_start: GrammarSpan,
    full_end: GrammarSpan,
) -> Node {
    // NOTE:
    // - `content` is the heading text *without* the underline.
    // - `full_start..full_end` covers the entire setext construct including the underline,
    //   which is what we want for highlighting.
    let span = opt_span_range(full_start, full_end);
    let (text, id) = split_extended_heading_id(content.fragment());
    let children = heading_inlines(content, &text);

    Node {
        kind: NodeKind::Heading { level, text, id },
        span,
        children,
    }
}

/// Split a heading's text from an optional extended id suffix.
///
/// Supported syntax (Markdown Guide "extended" style):
/// - `### Title {#custom-id}`
///
/// Rules (intentionally strict):
/// - The suffix must be at the end of the heading.
/// - The opening must be exactly `{#` (no whitespace between).
/// - The id must be non-empty and contain no whitespace.
/// - There must be at least one whitespace character before the `{`.
fn split_extended_heading_id(input: &str) -> (String, Option<String>) {
    let trimmed = input.trim_end();
    if !trimmed.ends_with('}') {
        return (input.to_string(), None);
    }

    let start = match trimmed.rfind("{#") {
        Some(pos) => pos,
        None => return (input.to_string(), None),
    };

    // Require at least one whitespace char right before the `{`.
    if start == 0 {
        return (input.to_string(), None);
    }
    let before = &trimmed[..start];
    if !before.chars().last().is_some_and(|c| c.is_whitespace()) {
        return (input.to_string(), None);
    }

    let id = &trimmed[start + 2..trimmed.len() - 1];
    if id.is_empty()
        || id
            .chars()
            .any(|c| c.is_whitespace() || c == '{' || c == '}')
    {
        return (input.to_string(), None);
    }

    let text = before.trim_end().to_string();
    (text, Some(id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::blocks as grammar;

    #[test]
    fn smoke_test_parse_atx_heading_level_1() {
        let content = GrammarSpan::new("Hello World");
        let node = parse_atx_heading(1, content);

        if let NodeKind::Heading { level, text, id } = node.kind {
            assert_eq!(level, 1);
            assert_eq!(text, "Hello World");
            assert!(id.is_none());
        } else {
            panic!("Expected Heading node");
        }
    }

    #[test]
    fn smoke_test_parse_atx_heading_level_6() {
        let content = GrammarSpan::new("Small heading");
        let node = parse_atx_heading(6, content);

        if let NodeKind::Heading { level, text, id } = node.kind {
            assert_eq!(level, 6);
            assert_eq!(text, "Small heading");
            assert!(id.is_none());
        } else {
            panic!("Expected Heading node");
        }
    }

    #[test]
    fn smoke_test_parse_setext_heading_level_1() {
        let start = GrammarSpan::new("Main Title\n===\n");
        let (rest, (level, content)) = grammar::setext_heading(start).unwrap();
        let node = parse_setext_heading(level, content, start, rest);

        if let NodeKind::Heading { level, text, id } = node.kind {
            assert_eq!(level, 1);
            assert_eq!(text, "Main Title");
            assert!(id.is_none());
        } else {
            panic!("Expected Heading node");
        }
    }

    #[test]
    fn smoke_test_parse_setext_heading_level_2() {
        let start = GrammarSpan::new("Subtitle\n---\n");
        let (rest, (level, content)) = grammar::setext_heading(start).unwrap();
        let node = parse_setext_heading(level, content, start, rest);

        if let NodeKind::Heading { level, text, id } = node.kind {
            assert_eq!(level, 2);
            assert_eq!(text, "Subtitle");
            assert!(id.is_none());
        } else {
            panic!("Expected Heading node");
        }
    }

    #[test]
    fn smoke_test_setext_heading_span_includes_underline_line() {
        let start = GrammarSpan::new("Title\n===\nNext\n");
        let (rest, (level, content)) = grammar::setext_heading(start).unwrap();
        let node = parse_setext_heading(level, content, start, rest);

        let span = node.span.expect("setext heading should have span");
        // Should span across at least 2 lines (content + underline).
        assert_eq!(span.start.line, 1);
        assert!(
            span.end.line >= 2,
            "expected underline line to be included in span"
        );
    }

    #[test]
    fn smoke_test_heading_span_tracking() {
        let content = GrammarSpan::new("Test");
        let node = parse_atx_heading(3, content);

        assert!(node.span.is_some());
        let span = node.span.unwrap();
        assert_eq!(span.start.line, 1);
        assert_eq!(span.start.column, 1);
    }

    /// Headings carry their content as inline children (CommonMark §4.2 / §4.3).
    /// This previously asserted `children.is_empty()`, which encoded the bug that
    /// heading content was never parsed as inlines.
    #[test]
    fn smoke_test_heading_parses_inline_children() {
        let content = GrammarSpan::new("Test");
        let node = parse_atx_heading(2, content);

        assert_eq!(node.children.len(), 1);
        assert!(matches!(node.children[0].kind, NodeKind::Text(ref t) if t == "Test"));
    }

    #[test]
    fn regression_atx_heading_parses_emphasis() {
        let node = parse_atx_heading(1, GrammarSpan::new("foo *bar*"));

        // `text` stays the raw content: the heading id is still derived from it.
        match &node.kind {
            NodeKind::Heading { text, .. } => assert_eq!(text, "foo *bar*"),
            _ => panic!("expected Heading"),
        }
        assert!(
            node.children
                .iter()
                .any(|c| matches!(c.kind, NodeKind::Emphasis)),
            "expected an Emphasis child, got {:?}",
            node.children.iter().map(|c| &c.kind).collect::<Vec<_>>()
        );
    }

    #[test]
    fn regression_setext_heading_parses_emphasis() {
        let start = GrammarSpan::new("Foo *bar*\n===\n");
        let (rest, (level, content)) = grammar::setext_heading(start).unwrap();
        let node = parse_setext_heading(level, content, start, rest);

        assert_eq!(level, 1);
        assert!(
            node.children
                .iter()
                .any(|c| matches!(c.kind, NodeKind::Emphasis)),
            "expected an Emphasis child in a setext heading"
        );
    }

    /// The `{#id}` suffix must be excluded from the inline content, not parsed
    /// into it.
    #[test]
    fn regression_extended_id_suffix_excluded_from_inlines() {
        let node = parse_atx_heading(3, GrammarSpan::new("Title *x* {#custom-id}"));

        match &node.kind {
            NodeKind::Heading { id, .. } => assert_eq!(id.as_deref(), Some("custom-id")),
            _ => panic!("expected Heading"),
        }
        let rendered: String = collect_text(&node.children);
        assert!(
            !rendered.contains("custom-id"),
            "id suffix leaked into inline content: {rendered:?}"
        );
    }

    fn collect_text(nodes: &[Node]) -> String {
        nodes
            .iter()
            .map(|n| match &n.kind {
                NodeKind::Text(t) => t.clone(),
                _ => collect_text(&n.children),
            })
            .collect()
    }

    #[test]
    fn smoke_test_heading_empty_text() {
        let content = GrammarSpan::new("");
        let node = parse_atx_heading(1, content);

        if let NodeKind::Heading { text, .. } = node.kind {
            assert_eq!(text, "");
        } else {
            panic!("Expected Heading node");
        }
    }

    #[test]
    fn smoke_test_parse_extended_heading_id_suffix() {
        let content = GrammarSpan::new("Title {#custom-id}");
        let node = parse_atx_heading(3, content);

        match node.kind {
            NodeKind::Heading { level, text, id } => {
                assert_eq!(level, 3);
                assert_eq!(text, "Title");
                assert_eq!(id.as_deref(), Some("custom-id"));
            }
            _ => panic!("Expected Heading node"),
        }
    }
}
