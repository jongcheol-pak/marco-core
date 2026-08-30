use crossterm::style::{Attribute, Color, SetAttribute, SetForegroundColor, ResetColor};
use marco_core::Span;
use marco_core::{Document, Node, NodeKind};
use std::fmt::Write as FmtWrite;

/// Print the AST for the given document to stdout.
pub fn print_ast(
    doc: &Document,
    source: Option<&str>,
    use_color: bool,
    compact: bool,
    show_spans: bool,
    show_excerpts: bool,
) -> String {
    let mut buf = String::new();
    let n = doc.children.len();
    let noun = if n == 1 { "node" } else { "nodes" };

    if use_color {
        let _ = write!(
            buf,
            "{}{}Document{} ({n} top-level {noun})\n",
            SetAttribute(Attribute::Dim),
            SetForegroundColor(Color::White),
            ResetColor,
        );
    } else {
        let _ = writeln!(buf, "Document ({n} top-level {noun})");
    }

    for (i, child) in doc.children.iter().enumerate() {
        let is_last = i == n - 1;
        print_node(
            child,
            source,
            "",
            is_last,
            use_color,
            compact,
            show_spans,
            show_excerpts,
            &mut buf,
        );
    }

    buf
}

fn print_node(
    node: &Node,
    source: Option<&str>,
    prefix: &str,
    is_last: bool,
    use_color: bool,
    compact: bool,
    show_spans: bool,
    show_excerpts: bool,
    buf: &mut String,
) {
    let connector = if is_last { "└── " } else { "├── " };
    let (label, attrs) = format_kind(&node.kind);
    let span_info = if show_spans {
        format_span(node.span)
    } else {
        String::new()
    };
    let excerpt_info = if show_excerpts {
        format_excerpt(source, node.span)
    } else {
        String::new()
    };
    let color = kind_color(&node.kind);
    let is_stray = is_stray_delimiter(&node.kind);

    if use_color {
        if is_stray {
            // Red bold label + ⚠ warning glyph for unmatched delimiter fragments
            let _ = write!(
                buf,
                "{prefix}{connector}{}{}⚠ {label}{}",
                SetForegroundColor(Color::Red),
                SetAttribute(Attribute::Bold),
                ResetColor,
            );
        } else {
            let _ = write!(
                buf,
                "{prefix}{connector}{}{}{label}{}",
                SetForegroundColor(color),
                SetAttribute(Attribute::Bold),
                ResetColor,
            );
        }
        if !attrs.is_empty() {
            let attr_color = if is_stray { Color::Red } else { color };
            let _ = write!(
                buf,
                " {}{}{}{}",
                SetForegroundColor(attr_color),
                SetAttribute(Attribute::Dim),
                attrs,
                ResetColor,
            );
        }
        if !span_info.is_empty() {
            let _ = write!(
                buf,
                " {}{}{}{}",
                SetForegroundColor(Color::DarkGrey),
                SetAttribute(Attribute::Dim),
                span_info,
                ResetColor,
            );
        }
        if !excerpt_info.is_empty() {
            let _ = write!(
                buf,
                " {}{}{}{}",
                SetForegroundColor(Color::DarkGrey),
                SetAttribute(Attribute::Dim),
                excerpt_info,
                ResetColor,
            );
        }
        buf.push('\n');
    } else {
        let warn = if is_stray { "⚠ " } else { "" };
        let mut extras = String::new();
        if !attrs.is_empty() {
            extras.push_str(&attrs);
        }
        if !span_info.is_empty() {
            if !extras.is_empty() {
                extras.push(' ');
            }
            extras.push_str(&span_info);
        }
        if !excerpt_info.is_empty() {
            if !extras.is_empty() {
                extras.push(' ');
            }
            extras.push_str(&excerpt_info);
        }

        if extras.is_empty() {
            let _ = writeln!(buf, "{prefix}{connector}{warn}{label}");
        } else {
            let _ = writeln!(buf, "{prefix}{connector}{warn}{label} {extras}");
        }
    }

    if compact {
        return;
    }

    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
    let n = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        print_node(
            child,
            source,
            &child_prefix,
            i == n - 1,
            use_color,
            compact,
            show_spans,
            show_excerpts,
            buf,
        );
    }
}

fn format_span(span: Option<Span>) -> String {
    match span {
        Some(span) => format!(
            "[L{}:C{}..L{}:C{} bytes {}..{}]",
            span.start.line,
            span.start.column,
            span.end.line,
            span.end.column,
            span.start.offset,
            span.end.offset,
        ),
        None => "[span=none]".to_string(),
    }
}

fn format_excerpt(source: Option<&str>, span: Option<Span>) -> String {
    let (Some(source), Some(span)) = (source, span) else {
        return String::new();
    };
    let Some(slice) = source.get(span.start.offset..span.end.offset) else {
        return String::from("«<non-boundary slice>»");
    };
    if slice.is_empty() {
        return String::new();
    }
    let normalized = slice.replace('\n', "↵");
    let mut short = String::new();
    for (count, ch) in normalized.chars().enumerate() {
        if count >= 40 {
            short.push('…');
            break;
        }
        short.push(ch);
    }
    format!("«{}»", short)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.replace('\n', "↵")
    } else {
        format!("{}…", &s[..max].replace('\n', "↵"))
    }
}

fn format_kind(kind: &NodeKind) -> (String, String) {
    match kind {
        NodeKind::Heading { level, text, id } => {
            let label = format!("Heading(h{level})");
            let mut a = format!("\"{}\"", truncate(text, 60));
            if let Some(id) = id {
                a.push_str(&format!(" {{#{id}}}"));
            }
            (label, a)
        }
        NodeKind::Paragraph => ("Paragraph".into(), String::new()),
        NodeKind::CodeBlock { language, .. } => {
            let lang = language.as_deref().unwrap_or("");
            ("CodeBlock".into(), format!("[lang={lang}]"))
        }
        NodeKind::ThematicBreak => ("ThematicBreak".into(), String::new()),
        NodeKind::List { ordered, tight, start } => {
            let s = if *ordered {
                format!("[ordered, start={}, tight={tight}]", start.unwrap_or(1))
            } else {
                format!("[unordered, tight={tight}]")
            };
            ("List".into(), s)
        }
        NodeKind::ListItem => ("ListItem".into(), String::new()),
        NodeKind::DefinitionList => ("DefinitionList".into(), String::new()),
        NodeKind::DefinitionTerm => ("DefinitionTerm".into(), String::new()),
        NodeKind::DefinitionDescription => ("DefinitionDescription".into(), String::new()),
        NodeKind::TaskCheckbox { checked } => {
            let mark = if *checked { "x" } else { " " };
            ("TaskCheckbox".into(), format!("[{mark}]"))
        }
        NodeKind::TaskCheckboxInline { checked } => {
            let mark = if *checked { "x" } else { " " };
            ("TaskCheckboxInline".into(), format!("[{mark}]"))
        }
        NodeKind::Blockquote => ("Blockquote".into(), String::new()),
        NodeKind::Admonition { kind, title, .. } => {
            let mut a = format!("({kind:?})");
            if let Some(t) = title {
                a.push_str(&format!(" \"{t}\""));
            }
            ("Admonition".into(), a)
        }
        NodeKind::TabGroup => ("TabGroup".into(), String::new()),
        NodeKind::TabItem { title } => ("TabItem".into(), format!("\"{}\"", truncate(title, 40))),
        NodeKind::SliderDeck { timer_seconds } => {
            let a = match timer_seconds {
                Some(t) => format!("[timer={t}s]"),
                None => String::new(),
            };
            ("SliderDeck".into(), a)
        }
        NodeKind::Slide { vertical } => {
            let a = if *vertical { "[vertical]".into() } else { String::new() };
            ("Slide".into(), a)
        }
        NodeKind::Table { alignments } => {
            ("Table".into(), format!("[cols={}]", alignments.len()))
        }
        NodeKind::TableRow { header } => {
            let a = if *header { "[header]" } else { "" };
            ("TableRow".into(), a.into())
        }
        NodeKind::TableCell { header, alignment } => {
            let mut a: String = if *header { "[header".into() } else { "[".into() };
            a.push_str(&format!(", align={alignment:?}]"));
            ("TableCell".into(), a)
        }
        NodeKind::HtmlBlock { .. } => ("HtmlBlock".into(), String::new()),
        NodeKind::FootnoteDefinition { label } => {
            ("FootnoteDef".into(), format!("\"[^{label}]\""))
        }
        NodeKind::Text(s) => ("Text".into(), format!("\"{}\"", truncate(s, 60))),
        NodeKind::Emphasis => ("Emphasis".into(), String::new()),
        NodeKind::Strong => ("Strong".into(), String::new()),
        NodeKind::StrongEmphasis => ("StrongEmphasis".into(), String::new()),
        NodeKind::Strikethrough => ("Strikethrough".into(), String::new()),
        NodeKind::Mark => ("Mark".into(), String::new()),
        NodeKind::Superscript => ("Superscript".into(), String::new()),
        NodeKind::Subscript => ("Subscript".into(), String::new()),
        NodeKind::Link { url, title } => {
            let mut a = format!("\"{}\"", truncate(url, 60));
            if let Some(t) = title {
                a.push_str(&format!(" title=\"{}\"", truncate(t, 40)));
            }
            ("Link".into(), a)
        }
        NodeKind::LinkReference { label, .. } => {
            ("LinkRef".into(), format!("\"[{label}]\""))
        }
        NodeKind::FootnoteReference { label } => {
            ("FootnoteRef".into(), format!("\"[^{label}]\""))
        }
        NodeKind::Image { url, alt } => {
            ("Image".into(), format!("\"{}\" alt=\"{}\"", truncate(url, 60), truncate(alt, 40)))
        }
        NodeKind::CodeSpan(s) => ("CodeSpan".into(), format!("\"{}\"", truncate(s, 60))),
        NodeKind::InlineHtml(s) => ("InlineHtml".into(), format!("\"{}\"", truncate(s, 40))),
        NodeKind::HardBreak => ("HardBreak".into(), String::new()),
        NodeKind::SoftBreak => ("SoftBreak".into(), String::new()),
        NodeKind::PlatformMention { username, platform, display } => {
            let mut a = format!("@{username}[{platform}]");
            if let Some(d) = display {
                a.push_str(&format!("({d})"));
            }
            ("PlatformMention".into(), a)
        }
        NodeKind::InlineMath { content } => {
            ("InlineMath".into(), format!("\"{}\"", truncate(content, 40)))
        }
        NodeKind::DisplayMath { content } => {
            ("DisplayMath".into(), format!("\"{}\"", truncate(content, 40)))
        }
        NodeKind::MermaidDiagram { .. } => ("MermaidDiagram".into(), String::new()),
    }
}

fn kind_color(kind: &NodeKind) -> Color {
    match kind {
        NodeKind::Heading { .. }
        | NodeKind::Paragraph
        | NodeKind::Blockquote
        | NodeKind::ThematicBreak
        | NodeKind::HtmlBlock { .. }
        | NodeKind::FootnoteDefinition { .. } => Color::Blue,

        NodeKind::List { .. }
        | NodeKind::ListItem
        | NodeKind::TaskCheckbox { .. }
        | NodeKind::TaskCheckboxInline { .. }
        | NodeKind::DefinitionList
        | NodeKind::DefinitionTerm
        | NodeKind::DefinitionDescription => Color::Cyan,

        NodeKind::Text(_)
        | NodeKind::Emphasis
        | NodeKind::Strong
        | NodeKind::StrongEmphasis
        | NodeKind::Strikethrough
        | NodeKind::Mark
        | NodeKind::Superscript
        | NodeKind::Subscript
        | NodeKind::Link { .. }
        | NodeKind::LinkReference { .. }
        | NodeKind::FootnoteReference { .. }
        | NodeKind::Image { .. }
        | NodeKind::InlineHtml(_)
        | NodeKind::SoftBreak
        | NodeKind::HardBreak
        | NodeKind::PlatformMention { .. } => Color::Green,

        NodeKind::CodeBlock { .. }
        | NodeKind::CodeSpan(_)
        | NodeKind::InlineMath { .. }
        | NodeKind::DisplayMath { .. } => Color::Yellow,

        NodeKind::Table { .. }
        | NodeKind::TableRow { .. }
        | NodeKind::TableCell { .. }
        | NodeKind::TabGroup
        | NodeKind::TabItem { .. }
        | NodeKind::SliderDeck { .. }
        | NodeKind::Slide { .. }
        | NodeKind::Admonition { .. }
        | NodeKind::MermaidDiagram { .. } => Color::Magenta,
    }
}

/// Returns true when a `Text` node contains only characters that look like
/// unmatched inline delimiters (`*`, `_`, `~`, `=`, `^`, `` ` ``).
/// These are the leftover fragments the parser emits when emphasis/strong
/// delimiters cannot be matched.
fn is_stray_delimiter(kind: &NodeKind) -> bool {
    if let NodeKind::Text(s) = kind {
        let trimmed = s.trim();
        !trimmed.is_empty()
            && trimmed
                .chars()
                .all(|c| matches!(c, '*' | '_' | '~' | '=' | '^' | '`'))
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use marco_core::{Position, Span};

    #[test]
    fn print_ast_includes_span_metadata_when_enabled() {
        let doc = Document {
            children: vec![Node {
                kind: NodeKind::Text("abc".to_string()),
                span: Some(Span::new(
                    Position::new(1, 1, 0),
                    Position::new(1, 4, 3),
                )),
                children: Vec::new(),
            }],
            references: Default::default(),
        };

        let printed = print_ast(&doc, Some("abc"), false, false, true, false);
        assert!(printed.contains("[L1:C1..L1:C4 bytes 0..3]"));
    }

    #[test]
    fn print_ast_includes_excerpt_when_enabled() {
        let doc = Document {
            children: vec![Node {
                kind: NodeKind::Text("abc".to_string()),
                span: Some(Span::new(
                    Position::new(1, 1, 0),
                    Position::new(1, 4, 3),
                )),
                children: Vec::new(),
            }],
            references: Default::default(),
        };

        let printed = print_ast(&doc, Some("abc"), false, false, false, true);
        assert!(printed.contains("«abc»"));
    }
}
