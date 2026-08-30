//! Math inline parser - converts grammar output to InlineMath AST node
//!
//! Parses inline math delimited by `$...$` using KaTeX.

use super::shared::{opt_span, GrammarSpan};
use crate::grammar::inlines::inline_math;
use crate::parser::ast::{Node, NodeKind};
use nom::IResult;

/// Parse an inline math expression `$...$` to an InlineMath AST node.
///
/// # Example
/// ```ignore
/// use marco_core::parser::inlines::math_inline_parser::parse_inline_math;
/// use marco_core::parser::shared::GrammarSpan;
/// use marco_core::NodeKind;
///
/// let input = GrammarSpan::new("$E = mc^2$ text");
/// let (_rest, node) = parse_inline_math(input).unwrap();
/// assert!(matches!(node.kind, NodeKind::InlineMath { content } if content == "E = mc^2"));
/// ```
pub fn parse_inline_math(input: GrammarSpan) -> IResult<GrammarSpan, Node> {
    let (rest, content_span) = inline_math(input)?;

    let node = Node {
        kind: NodeKind::InlineMath {
            content: content_span.fragment().to_string(),
        },
        span: opt_span(content_span),
        children: Vec::new(),
    };

    Ok((rest, node))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_parse_inline_math() {
        let input = GrammarSpan::new("$E = mc^2$ text");
        let result = parse_inline_math(input);
        assert!(result.is_ok());

        let (_, node) = result.unwrap();
        match node.kind {
            NodeKind::InlineMath { content } => {
                assert_eq!(content, "E = mc^2");
            }
            _ => panic!("Expected InlineMath node"),
        }
    }
}
