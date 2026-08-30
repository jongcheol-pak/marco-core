//! Math display parser - converts grammar output to DisplayMath AST node
//!
//! Parses display math delimited by `$$...$$` using KaTeX.

use super::shared::{opt_span, GrammarSpan};
use crate::grammar::inlines::display_math;
use crate::parser::ast::{Node, NodeKind};
use nom::IResult;

/// Parse a display math expression `$$...$$` to a DisplayMath AST node.
///
/// # Example
/// ```ignore
/// use marco_core::parser::inlines::math_display_parser::parse_display_math;
/// use marco_core::parser::shared::GrammarSpan;
/// use marco_core::NodeKind;
///
/// let input = GrammarSpan::new("$$\\int x^2 dx$$ text");
/// let (_rest, node) = parse_display_math(input).unwrap();
/// assert!(matches!(node.kind, NodeKind::DisplayMath { content } if content == "\\int x^2 dx"));
/// ```
pub fn parse_display_math(input: GrammarSpan) -> IResult<GrammarSpan, Node> {
    let (rest, content_span) = display_math(input)?;

    let node = Node {
        kind: NodeKind::DisplayMath {
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
    fn smoke_test_parse_display_math() {
        let input = GrammarSpan::new("$$x^2 + y^2 = r^2$$ text");
        let result = parse_display_math(input);
        assert!(result.is_ok());

        let (_, node) = result.unwrap();
        match node.kind {
            NodeKind::DisplayMath { content } => {
                assert_eq!(content, "x^2 + y^2 = r^2");
            }
            _ => panic!("Expected DisplayMath node"),
        }
    }
}
