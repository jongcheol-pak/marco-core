//! Inline parser modules - convert grammar output to AST nodes
//!
//! This module contains specialized parsers that convert inline grammar elements
//! (from grammar/inlines) into AST nodes with proper position tracking.
//!
//! Phase 5: Inline parser module extraction

use crate::error::MarcoError;

// Shared utilities for all inline parsers
pub mod shared;

// Emphasis/strong delimiter-stack resolution (CommonMark spec 6.2, Appendix A).
mod emphasis;

// Regression tests for the `parallel-parse` feature. A unit test rather than an
// integration test because it calls `parse_inlines_from_span`, whose
// `GrammarSpan` argument is a `nom` type that must not reach the public API.
#[cfg(all(test, feature = "parallel-parse"))]
mod parallel_tests;

// Individual inline parser modules
pub mod cm_autolink_parser;
pub mod cm_backslash_escape_parser;
pub mod cm_code_span_parser;
pub mod cm_entity_reference_parser;
pub mod cm_image_parser;
pub mod cm_inline_html_parser;
pub mod cm_line_breaks_parser;
pub mod cm_link_parser;
pub mod cm_reference_link_parser;
pub mod gfm_autolink_literal_parser;
pub mod gfm_footnote_reference_parser;
pub mod gfm_strikethrough_parser;
pub mod marco_dash_strikethrough_parser;
pub mod marco_emoji_shortcode_parser;
pub mod marco_inline_footnote_parser;
pub mod marco_mark_parser;
pub mod marco_platform_mentions_parser;
pub mod marco_subscript_arrow_parser;
pub mod marco_subscript_parser;
pub mod marco_superscript_parser;
pub mod marco_task_checkbox_inline_parser;
pub mod math_display_parser;
pub mod math_inline_parser;
pub mod text_parser;

// Re-export parser functions for convenience
pub use cm_autolink_parser::parse_autolink;
pub use cm_backslash_escape_parser::parse_backslash_escape;
pub use cm_code_span_parser::parse_code_span;
pub use cm_entity_reference_parser::parse_entity_reference;
pub use cm_image_parser::parse_image;
pub use cm_inline_html_parser::parse_inline_html;
pub use cm_line_breaks_parser::{parse_hard_line_break, parse_soft_line_break};
pub use cm_link_parser::parse_link;
pub use cm_reference_link_parser::parse_reference_link;
pub use gfm_autolink_literal_parser::parse_gfm_autolink_literal;
pub use gfm_footnote_reference_parser::parse_footnote_reference;
pub use gfm_strikethrough_parser::parse_strikethrough;
pub use marco_dash_strikethrough_parser::parse_dash_strikethrough;
pub use marco_emoji_shortcode_parser::parse_emoji_shortcode;
pub use marco_inline_footnote_parser::parse_inline_footnote;
pub use marco_mark_parser::parse_mark;
pub use marco_platform_mentions_parser::parse_platform_mention;
pub use marco_subscript_arrow_parser::parse_subscript_arrow;
pub use marco_subscript_parser::parse_subscript;
pub use marco_superscript_parser::parse_superscript;
pub use marco_task_checkbox_inline_parser::parse_task_checkbox_inline;
pub use math_display_parser::parse_display_math;
pub use math_inline_parser::parse_inline_math;
pub use text_parser::{parse_special_as_text, parse_text};

use super::ast::Node;
use emphasis::{resolve_emphasis, tokenize_delimiter_run, Item};
use nom::bytes::complete::take;
use shared::GrammarSpan;

/// Parse inline elements within text content
/// Takes a GrammarSpan to preserve position information
/// Returns a vector of inline nodes (Text, Emphasis, Strong, Link, CodeSpan)
pub fn parse_inlines_from_span(span: GrammarSpan) -> Result<Vec<Node>, MarcoError> {
    log::debug!(
        "Parsing inline elements in span at line {}: {:?}",
        span.location_line(),
        span.fragment()
    );

    // Precompute `[`/`]` bracket matches for this span once, up front, so
    // link/image/reference-link parsing never has to rescan overlapping
    // text on retry — see `bracket_match` module docs. Scoped to this call
    // (including any nested `parse_inlines_from_span` calls it makes, e.g.
    // for link text) via RAII; restored on return.
    let _bracket_guard = crate::grammar::inlines::bracket_match::BracketCacheGuard::install(
        span.location_offset(),
        span.fragment(),
    );

    let mut items: Vec<Item> = Vec::with_capacity(8);
    let mut remaining = span;

    // Safety: prevent infinite loops
    const MAX_ITERATIONS: usize = 1000;
    let mut iteration_count = 0;
    let mut last_offset = 0;

    while !remaining.fragment().is_empty() {
        iteration_count += 1;
        if iteration_count > MAX_ITERATIONS {
            log::error!("Inline parser exceeded MAX_ITERATIONS ({})", MAX_ITERATIONS);
            break;
        }

        let start_pos = remaining.location_offset();

        // Safety: ensure we're making progress
        if start_pos == last_offset && iteration_count > 1 {
            log::error!(
                "Inline parser not making progress at offset {}, forcing skip",
                start_pos
            );
            // Force skip one character
            let skip = remaining
                .fragment()
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            if let Ok((rest, _)) = take::<_, _, nom::error::Error<_>>(skip)(remaining) {
                remaining = rest;
                last_offset = remaining.location_offset();
                continue;
            } else {
                break;
            }
        }
        last_offset = start_pos;

        // ---------------------------------------------------------------
        // Fast path: skip all ~25 parser attempts for bytes that cannot
        // possibly start any special inline sequence.
        //
        // Special ASCII bytes (can open a parser):
        //   * _ ` [ < ! & \n \\ $ ^ ~ = -
        // Space (0x20) is usually plain text but "  \n" (2+ spaces + newline)
        // forms a hard line break — let full dispatch handle that case.
        // Any byte >= 0x80 may start a multi-byte character (e.g. ˅ U+02C5)
        // so we leave those for the full dispatch.
        // ---------------------------------------------------------------
        // SAFETY: the loop condition guarantees remaining is non-empty.
        let first_byte = remaining.fragment().as_bytes()[0];
        let is_non_special_ascii = first_byte < 0x80
            && !matches!(
                first_byte,
                b'*' | b'_'
                    | b'`'
                    | b'['
                    | b'<'
                    | b'!'
                    | b'&'
                    | b'\n'
                    | b'\\'
                    | b'$'
                    | b'^'
                    | b'~'
                    | b'='
                    | b'-'
            );
        // Guard spaces: "  \n" is a hard line break — let the full loop handle it.
        let safe_to_fast_path = is_non_special_ascii
            && if first_byte == b' ' {
                let frag = remaining.fragment().as_bytes();
                let sp = frag.iter().take_while(|&&b| b == b' ').count();
                !(sp >= 2 && frag.get(sp) == Some(&b'\n'))
            } else {
                true
            };
        if safe_to_fast_path {
            if let Ok((rest, node)) = parse_text(remaining) {
                items.push(Item::Node(node));
                remaining = rest;
                continue;
            }
            // parse_text failed: the position may start a GFM autolink literal,
            // an emoji shortcode, a platform mention, or trailing hard-break spaces.
            // Fall through to the full dispatch so those parsers get a chance.
        }

        // Try parsing code span first (highest priority to avoid conflicts)
        if let Ok((rest, node)) = parse_code_span(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing display math before inline math (avoid $$ being parsed as two $)
        if crate::parser::shared::parse_math_enabled() {
            if let Ok((rest, node)) = parse_display_math(remaining) {
                items.push(Item::Node(node));
                remaining = rest;
                continue;
            }

            // Try parsing inline math
            if let Ok((rest, node)) = parse_inline_math(remaining) {
                items.push(Item::Node(node));
                remaining = rest;
                continue;
            }
        }

        // Try parsing backslash escape (before other inline elements)
        if let Ok((rest, node)) = parse_backslash_escape(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Extension inlines (non-CommonMark): try these early so their delimiter
        // sequences aren't consumed as plain text.
        if let Ok((rest, node)) = parse_strikethrough(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        if let Ok((rest, node)) = parse_dash_strikethrough(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        if let Ok((rest, node)) = parse_mark(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Emphasis-family delimiter runs (`*`/`_`). Rather than resolving
        // immediately here, tokenize the run and defer matching to
        // `resolve_emphasis` once the whole span has been tokenized — see
        // the `emphasis` module for why (this is what makes nested/unbalanced
        // delimiter runs linear instead of super-linear, and it's also what
        // correctly implements the CommonMark left/right-flanking and
        // intraword-underscore rules instead of the ad hoc special case this
        // replaced).
        if first_byte == b'*' || first_byte == b'_' {
            let consumed_len = remaining.location_offset() - span.location_offset();
            let before = span.fragment()[..consumed_len].chars().next_back();
            let (delim, rest) = tokenize_delimiter_run(remaining, before);
            items.push(Item::Delim(delim));
            remaining = rest;
            continue;
        }

        // Extended syntax: inline footnotes `^[...]`.
        // Try before superscript since both start with '^'.
        if let Ok((rest, (ref_node, def_node))) = parse_inline_footnote(remaining) {
            items.push(Item::Node(ref_node));
            items.push(Item::Node(def_node));
            remaining = rest;
            continue;
        }

        if let Ok((rest, node)) = parse_superscript(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        if let Ok((rest, node)) = parse_subscript_arrow(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        if let Ok((rest, node)) = parse_subscript(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing GFM autolink literals (www/http(s)/email/protocol forms)
        if let Ok((rest, node)) = parse_gfm_autolink_literal(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing autolink (must come before link and inline HTML since syntax starts with <)
        if let Ok((rest, node)) = parse_autolink(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing GFM-style footnote references `[^label]`.
        // Must come before link parsing since it also starts with '['.
        if let Ok((rest, node)) = parse_footnote_reference(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Extended syntax: inline task checkbox markers mid-paragraph.
        // This must come before link parsing since it starts with '['.
        if is_task_checkbox_inline_start_boundary_ok(&items, remaining.fragment()) {
            if let Ok((rest, node)) = parse_task_checkbox_inline(remaining) {
                items.push(Item::Node(node));
                remaining = rest;
                continue;
            }
        }

        // Try parsing image (must come before link since syntax is similar but starts with !)
        if let Ok((rest, node)) = parse_image(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing link
        if let Ok((rest, node)) = parse_link(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing reference-style links (CommonMark)
        if let Ok((rest, node)) = parse_reference_link(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing inline HTML
        if let Ok((rest, node)) = parse_inline_html(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing hard line break (two spaces + newline, or backslash + newline)
        if let Ok((rest, node)) = parse_hard_line_break(remaining) {
            log::debug!(
                "Parsed hard line break at offset {}",
                remaining.location_offset()
            );
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing soft line break (regular newline)
        if let Ok((rest, node)) = parse_soft_line_break(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing entity references (e.g. &copy;, &#169;)
        if let Ok((rest, node)) = parse_entity_reference(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing emoji shortcodes (extended syntax), e.g. :joy:
        if let Ok((rest, node)) = parse_emoji_shortcode(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Try parsing platform mentions (extended syntax), e.g. @user[github](Name)
        if let Ok((rest, node)) = parse_platform_mention(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // No inline element matched - try parsing plain text
        if let Ok((rest, node)) = parse_text(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Special character that didn't parse as any inline element - consume as text
        if let Ok((rest, node)) = parse_special_as_text(remaining) {
            items.push(Item::Node(node));
            remaining = rest;
            continue;
        }

        // Safety check: if we reach here, we failed to parse anything
        // This should not happen if all parsers are working correctly
        log::error!(
            "Inline parser unable to make progress at offset {}",
            start_pos
        );
        break;
    }

    let nodes = resolve_emphasis(items);
    log::debug!("Parsed {} inline nodes", nodes.len());
    Ok(nodes)
}

fn is_task_checkbox_inline_start_boundary_ok(items: &[Item], fragment: &str) -> bool {
    if !fragment.starts_with('[') {
        return false;
    }

    // If the previous emitted character is alphanumeric/underscore, we do not
    // treat `[x]` / `[ ]` as a task marker (avoid matching `word[x]`).
    match last_emitted_char(items) {
        None => true,
        Some(prev) => !(prev.is_alphanumeric() || prev == '_'),
    }
}

fn last_emitted_char(items: &[Item]) -> Option<char> {
    items.iter().rev().find_map(|item| item.last_char())
}

/// Parse inline elements within text content (backward compatibility wrapper)
/// Creates a new span at position 0:0 - USE parse_inlines_from_span() for position-aware parsing
/// Returns a vector of inline nodes (Text, Emphasis, Strong, Link, CodeSpan)
pub fn parse_inlines(text: &str) -> Result<Vec<Node>, MarcoError> {
    parse_inlines_from_span(GrammarSpan::new(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_triple_delimiter_parses_as_single_node() {
        let nodes = parse_inlines("***hi***").expect("inline parse failed");
        assert_eq!(nodes.len(), 1);
        assert!(matches!(
            nodes[0].kind,
            crate::parser::ast::NodeKind::StrongEmphasis
        ));
    }

    #[test]
    fn smoke_test_extension_inlines_parse_mid_line() {
        let nodes = parse_inlines(
            "This is ^sup^ and ~sub~ and ˅sub2˅ and ==mark== and ~~del~~ and --del2--.",
        )
        .expect("inline parse failed");

        use crate::parser::ast::NodeKind;

        assert!(nodes
            .iter()
            .any(|n| matches!(n.kind, NodeKind::Superscript)));
        assert!(nodes.iter().any(|n| matches!(n.kind, NodeKind::Subscript)));
        assert!(nodes.iter().any(|n| matches!(n.kind, NodeKind::Mark)));
        assert!(nodes
            .iter()
            .any(|n| matches!(n.kind, NodeKind::Strikethrough)));
    }
}
