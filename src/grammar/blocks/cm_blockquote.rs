// CommonMark Blockquote Grammar
// Parses blockquotes (lines starting with >)
//
// Per CommonMark spec:
// - Blockquote = lines starting with '>' (with 0-3 leading spaces)
// - Optional space after '>' is consumed
// - Lazy continuation: non-blank lines without '>' continue blockquote
// - Blank lines end blockquote
// - ATX headings and thematic breaks interrupt blockquote
// - Fenced code blocks prevent lazy continuation

use crate::grammar::blocks::cm_thematic_break::thematic_break;
use crate::grammar::shared::Span;
use nom::{IResult, Input};

/// Parse a blockquote (lines starting with >)
///
/// Examples:
/// - `"> Quote line"`
/// - `"> First\n> Second"`
/// - `"> Quote\nLazy continuation"`
///
/// # Arguments
/// * `input` - The input span to parse
///
/// # Returns
/// * `Ok((remaining, content_span))` - Success with remaining input and blockquote content
/// * `Err(_)` - Parse failure
pub fn blockquote(input: Span) -> IResult<Span, Span> {
    log::debug!(
        "Parsing blockquote from: {:?}",
        crate::logic::logger::safe_preview(input.fragment(), 40)
    );

    let start = input;
    let start_offset = start.location_offset();
    let mut remaining = input;
    let mut last_content_offset = start_offset;

    // Safety: prevent infinite loops
    const MAX_LINES: usize = 10000;
    let mut line_count = 0;
    let mut has_parsed_line = false;
    let mut last_line_opened_fence = false; // Track if previous line opened fenced code block

    loop {
        line_count += 1;
        if line_count > MAX_LINES {
            log::warn!("Blockquote exceeded MAX_LINES");
            break;
        }

        // Check if we've reached the end
        if remaining.fragment().is_empty() {
            break;
        }

        // Check leading spaces
        let leading_spaces = remaining
            .fragment()
            .chars()
            .take_while(|&c| c == ' ')
            .count();

        // Try to match '>' marker
        let after_spaces = if leading_spaces > 0 && leading_spaces < remaining.fragment().len() {
            &remaining.fragment()[leading_spaces..]
        } else if leading_spaces > 0 {
            ""
        } else {
            remaining.fragment()
        };

        // Check if this line has a > marker
        let has_marker = after_spaces.starts_with('>');

        // If line has > marker, it can only have 0-3 leading spaces
        if has_marker && leading_spaces > 3 {
            // Too much indentation before >, not valid blockquote line
            break;
        }

        if !has_marker {
            // No '>' marker
            // Lazy continuation: if we already have content, non-blank lines can continue
            if has_parsed_line {
                // Check if line is blank
                let line_end = after_spaces.find('\n').unwrap_or(after_spaces.len());
                let line = &after_spaces[..line_end];

                if line.trim().is_empty() {
                    // Blank line ends blockquote
                    break;
                }

                // Check if this could be another block element starting
                // (ATX heading, fenced code, etc.)
                if line.starts_with('#') {
                    // ATX heading - stop blockquote
                    break;
                }

                // CRITICAL: If previous line opened a fenced code block, lazy continuation is NOT allowed
                if last_line_opened_fence {
                    log::debug!("Blockquote stopping: previous line opened fenced code, lazy continuation not allowed");
                    break;
                }

                // Check for actual thematic break using parser (not just "---" prefix)
                let offset_in_remaining = leading_spaces;
                let line_span = remaining.take_from(offset_in_remaining).take(line_end);
                if thematic_break(line_span).is_ok() {
                    // This is a thematic break, stop blockquote
                    break;
                }

                // Lazy continuation - include this line
                let skip_len = if line_end < after_spaces.len() {
                    leading_spaces + line_end + 1 // Include newline
                } else {
                    leading_spaces + line_end
                };

                if skip_len <= remaining.fragment().len() {
                    let new_remaining = remaining.take_from(skip_len);
                    last_content_offset = new_remaining.location_offset();
                    remaining = new_remaining;
                    last_line_opened_fence = false; // Reset flag after consuming lazy continuation
                    continue;
                } else {
                    break;
                }
            } else {
                // Haven't parsed any blockquote lines yet, this is not a blockquote
                return Err(nom::Err::Error(nom::error::Error::new(
                    start,
                    nom::error::ErrorKind::Tag,
                )));
            }
        }

        // We have a '>' marker
        has_parsed_line = true;

        // Skip the '>' and optional space after it
        let after_marker = &after_spaces[1..];
        let after_optional_space = after_marker.strip_prefix(' ').unwrap_or(after_marker);

        // Get the rest of the line
        let line_end = after_optional_space
            .find('\n')
            .unwrap_or(after_optional_space.len());
        let line_content = &after_optional_space[..line_end];

        // Check if this line opens a fenced code block
        let line_trimmed = line_content.trim_start();
        last_line_opened_fence = line_trimmed.starts_with("```") || line_trimmed.starts_with("~~~");

        // Calculate how much to skip (leading spaces + '>' + optional space + line content + newline)
        let skip_len = if line_end < after_optional_space.len() {
            leading_spaces + 1 + (after_marker.len() - after_optional_space.len()) + line_end + 1
        } else {
            leading_spaces + 1 + (after_marker.len() - after_optional_space.len()) + line_end
        };

        // `skip_len` is a byte length, so slice by bytes via `Input::take_from`
        // rather than nom's `take()` combinator, which counts characters on `&str`.
        if skip_len <= remaining.fragment().len() {
            let new_remaining = remaining.take_from(skip_len);
            last_content_offset = new_remaining.location_offset();
            remaining = new_remaining;
        } else {
            // Couldn't consume, this shouldn't happen but break to be safe
            log::warn!("Failed to consume blockquote line");
            break;
        }
    }

    // Calculate content length
    let content_len = last_content_offset.saturating_sub(start_offset);

    if content_len == 0 || !has_parsed_line {
        return Err(nom::Err::Error(nom::error::Error::new(
            start,
            nom::error::ErrorKind::Tag,
        )));
    }

    // `content_len` is a byte length (from `location_offset()`), so slice by
    // bytes via `Input::take` rather than nom's `take()` combinator, which
    // counts characters on `&str`.
    let content_span = start.take(content_len);

    log::debug!("Blockquote parsed: {} bytes", content_span.fragment().len());

    Ok((remaining, content_span))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_blockquote_single_line() {
        let input = Span::new("> Quote");
        let result = blockquote(input);
        assert!(result.is_ok());
        let (_, content) = result.unwrap();
        assert!(content.fragment().contains("Quote"));
    }

    #[test]
    fn smoke_test_blockquote_multiline() {
        let input = Span::new("> First line\n> Second line");
        let result = blockquote(input);
        assert!(result.is_ok());
        let (_, content) = result.unwrap();
        assert!(content.fragment().contains("First line"));
        assert!(content.fragment().contains("Second line"));
    }

    #[test]
    fn smoke_test_blockquote_with_space_after_marker() {
        let input = Span::new(">  Content with spaces");
        let result = blockquote(input);
        assert!(result.is_ok());
        let (_, content) = result.unwrap();
        assert!(content.fragment().contains("Content with spaces"));
    }

    #[test]
    fn smoke_test_blockquote_lazy_continuation() {
        let input = Span::new("> First line\nLazy continuation");
        let result = blockquote(input);
        assert!(result.is_ok());
        let (_, content) = result.unwrap();
        assert!(content.fragment().contains("Lazy continuation"));
    }

    #[test]
    fn smoke_test_blockquote_ends_at_blank() {
        let input = Span::new("> Quote\n\nAfter blank");
        let result = blockquote(input);
        assert!(result.is_ok());
        let (remaining, content) = result.unwrap();
        assert!(content.fragment().contains("Quote"));
        assert!(remaining.fragment().trim_start().starts_with("After blank"));
    }

    #[test]
    fn smoke_test_blockquote_interrupted_by_heading() {
        let input = Span::new("> Quote\n# Heading");
        let result = blockquote(input);
        assert!(result.is_ok());
        let (remaining, _) = result.unwrap();
        assert!(remaining.fragment().starts_with("# Heading"));
    }

    #[test]
    fn smoke_test_blockquote_with_leading_spaces() {
        let input = Span::new("  > Indented quote");
        let result = blockquote(input);
        assert!(result.is_ok());
        let (_, content) = result.unwrap();
        assert!(content.fragment().contains("Indented quote"));
    }

    #[test]
    fn smoke_test_blockquote_fails_with_too_many_spaces() {
        let input = Span::new("    > Too indented");
        let result = blockquote(input);
        // Should fail because 4+ spaces before > is not valid
        assert!(result.is_err());
    }

    /// Regression: byte offsets must not be fed to nom's `take()` combinator,
    /// which counts *characters* on `&str`. Multi-byte content used to make the
    /// blockquote over-consume by one char per extra UTF-8 byte, swallowing the
    /// following block (or failing to match at all).
    #[test]
    fn regression_blockquote_non_ascii_does_not_overconsume() {
        for (input, quoted, rest) in [
            ("> Note.\n\npara2", "Note.", "para2"),
            ("> Nóte.\n\npara2", "Nóte.", "para2"),
            ("> Nóóte.\n\npara2", "Nóóte.", "para2"),
            ("> Nóóóte.\n\npara2", "Nóóóte.", "para2"),
            ("> 日本語テキスト\n\npara2", "日本語テキスト", "para2"),
            ("> Grüße 😀\n\npara2", "Grüße 😀", "para2"),
        ] {
            let (remaining, content) =
                blockquote(Span::new(input)).unwrap_or_else(|e| panic!("{input:?} failed: {e:?}"));
            assert!(
                content.fragment().contains(quoted),
                "{input:?}: content {:?} missing {quoted:?}",
                content.fragment()
            );
            assert!(
                remaining.fragment().trim_start().starts_with(rest),
                "{input:?}: remaining {:?} should start with {rest:?}",
                remaining.fragment()
            );
        }
    }

    /// Regression: a thematic break after a non-ASCII blockquote must survive
    /// intact rather than being partially eaten by the quote.
    #[test]
    fn regression_blockquote_non_ascii_leaves_thematic_break_intact() {
        for input in [
            "> Note.\n\n---\n\npara",
            "> Nóte.\n\n---\n\npara",
            "> Nóóte.\n\n---\n\npara",
            "> Nóóóte.\n\n---\n\npara",
        ] {
            let (remaining, _) =
                blockquote(Span::new(input)).unwrap_or_else(|e| panic!("{input:?} failed: {e:?}"));
            assert!(
                remaining.fragment().trim_start().starts_with("---"),
                "{input:?}: remaining {:?} should still hold the full ---",
                remaining.fragment()
            );
        }
    }
}
