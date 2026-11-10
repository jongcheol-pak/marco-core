//! Inline math grammar - LaTeX math delimited by `$...$`
//!
//! This module parses inline math expressions (e.g. `$E = mc^2$`) for
//! rendering with KaTeX.

use super::Span;
use nom::{
    bytes::complete::{tag, take_until},
    IResult,
};

/// Parse an inline math span delimited by single `$`
///
/// # Grammar
/// `$` + content + `$`
///
/// # Returns
/// The LaTeX content span (without the `$` delimiters).
///
/// # Example
/// ```
/// use marco_core::grammar::{inline_math, Span};
///
/// let input = Span::new("$x^2 + y^2 = r^2$ text");
/// let (rest, content) = inline_math(input).unwrap();
/// assert_eq!(*content.fragment(), "x^2 + y^2 = r^2");
/// assert_eq!(*rest.fragment(), " text");
/// ```
pub fn inline_math(input: Span) -> IResult<Span, Span> {
    log::debug!("Parsing inline math at: {:?}", input.fragment());

    // Opening delimiter
    let (input, _) = tag("$")(input)?;

    // Parse content until closing $
    // Allow anything except $$ (which would be display math)
    let content_str = input.fragment();

    // Check for immediate second $ (would be display math)
    if content_str.starts_with('$') || opens_like_text(content_str) {
        return not_math(input);
    }

    // Find the closing delimiter
    let (input, content) = take_until("$")(input)?;
    if content
        .fragment()
        .chars()
        .last()
        .is_some_and(char::is_whitespace)
    {
        return not_math(input);
    }

    // Consume closing delimiter
    let (input, _) = tag("$")(input)?;
    if input.fragment().starts_with(|c: char| c.is_ascii_digit()) {
        return not_math(input);
    }

    log::debug!("Inline math content: {:?}", content.fragment());
    Ok((input, content))
}

fn not_math(at: Span) -> IResult<Span, Span> {
    Err(nom::Err::Error(nom::error::Error::new(
        at,
        nom::error::ErrorKind::Tag,
    )))
}

fn opens_like_text(after_dollar: &str) -> bool {
    if after_dollar.starts_with(char::is_whitespace) {
        return true;
    }
    if !after_dollar.starts_with(|c: char| c.is_ascii_digit()) {
        return false;
    }
    let number_end = after_dollar
        .find(|c: char| !(c.is_ascii_digit() || c == ',' || c == '.'))
        .unwrap_or(after_dollar.len());
    after_dollar[number_end..]
        .chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || matches!(c, '!' | ')' | ':' | ';' | '?'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_inline_math_basic() {
        let input = Span::new("$E = mc^2$ text");
        let result = inline_math(input);
        assert!(result.is_ok());
        let (rest, content) = result.unwrap();
        assert_eq!(*content.fragment(), "E = mc^2");
        assert_eq!(*rest.fragment(), " text");
    }

    #[test]
    fn smoke_test_inline_math_with_special_chars() {
        let input = Span::new("$\\frac{a}{b}$");
        let result = inline_math(input);
        assert!(result.is_ok());
        let (_, content) = result.unwrap();
        assert_eq!(*content.fragment(), "\\frac{a}{b}");
    }

    fn parses_as_math(text: &str) -> Option<String> {
        inline_math(Span::new(text))
            .ok()
            .map(|(_, content)| content.fragment().to_string())
    }

    #[test]
    fn prices_are_not_math() {
        for text in [
            "$5 and $10",
            "$10.",
            "$5 + tax$ is the cost.",
            "$5, and f(x)",
            "$10; $5",
            "$5 + 2$ typed",
            "$HOME and $PATH stay",
            "$PATH; $HOME",
            "$ x$",
            "$a$5",
        ] {
            assert_eq!(parses_as_math(text), None, "{text:?} must stay text");
        }
    }

    #[test]
    fn math_still_parses() {
        for (text, math) in [
            ("$x^2$ for", "x^2"),
            ("$n$th term", "n"),
            ("$5+2$, and", "5+2"),
            ("$!x$.", "!x"),
            ("$2x$ more", "2x"),
            ("$x^*$s", "x^*"),
        ] {
            assert_eq!(parses_as_math(text).as_deref(), Some(math), "{text:?}");
        }
    }

    #[test]
    fn smoke_test_inline_math_not_display() {
        let input = Span::new("$$x^2$$");
        let result = inline_math(input);
        assert!(result.is_err()); // Should fail - this is display math
    }
}
