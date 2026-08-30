//! The public API must be *closed*: every type reachable from a public type
//! must itself be nameable from the crate root.
//!
//! Rust does not enforce this. A `pub` type buried in a `pub mod` compiles
//! perfectly well while being, in practice, unusable — a consumer can *read*
//! `node.span` but cannot write `fn f(s: Span)`. The defect is invisible from
//! inside the crate and surfaces only when someone downstream tries to build
//! against it.
//!
//! This test is the enforcement. It is an integration test on purpose:
//! `tests/*.rs` compile as separate crates, so they see exactly the API a
//! crates.io consumer sees. Everything below is imported from the crate root —
//! **no `marco_core::parser::…` paths** — so if a re-export goes missing, this
//! file stops compiling.
//!
//! Two failure modes are covered:
//!
//! 1. **A type stops being exported.** Caught by naming every type in a
//!    signature or an explicitly typed binding below.
//! 2. **A new `NodeKind` variant carries a type nobody exported.** Caught by
//!    the exhaustive match in `every_variant_payload_is_nameable`, which has no
//!    `_` arm — adding a variant breaks the build until someone writes an arm
//!    for it, which is the moment they notice its payload types.

use marco_core::{
    parse, render, AdmonitionKind, AdmonitionStyle, Document, HtmlPolicy, Node, NodeKind,
    ParseOptions, Position, ReferenceMap, RenderOptions, Span, TableAlignment, UrlPolicy,
};

/// Every exported type named in a position that requires the name to exist.
///
/// Type inference would hide a missing export, so each is annotated.
#[allow(dead_code, clippy::too_many_arguments)]
fn every_type_is_nameable(
    _document: Document,
    _node: Node,
    _kind: NodeKind,
    _span: Span,
    _position: Position,
    _references: ReferenceMap,
    _admonition_kind: AdmonitionKind,
    _admonition_style: AdmonitionStyle,
    _alignment: TableAlignment,
    _parse_options: ParseOptions,
    _render_options: RenderOptions,
    _html_policy: HtmlPolicy,
    _url_policy: UrlPolicy,
) {
}

#[cfg(feature = "intelligence-diagnostics")]
#[allow(dead_code)]
fn every_diagnostic_type_is_nameable(
    _diagnostic: marco_core::Diagnostic,
    _code: marco_core::DiagnosticCode,
    _severity: marco_core::DiagnosticSeverity,
    _options: marco_core::DiagnosticsOptions,
    _profile: marco_core::DiagnosticsProfile,
) {
}

/// The fields a consumer reaches through must be nameable, not merely readable.
#[test]
fn document_fields_are_nameable() {
    let document: Document = parse("# Title\n\n[ref]: https://example.com\n").expect("parse");

    let references: &ReferenceMap = &document.references;
    let children: &Vec<Node> = &document.children;

    assert!(!children.is_empty());
    let _ = references;

    let node: &Node = &document.children[0];
    let span: Option<&Span> = node.span.as_ref();
    if let Some(span) = span {
        let start: Position = span.start;
        let _ = start.line;
    }
}

/// Exhaustive on purpose: **no `_` arm**.
///
/// Adding a `NodeKind` variant fails this test to compile, which forces whoever
/// adds it to look at what its payload carries. If that payload names a type
/// that is not exported from the crate root, this file cannot be written — and
/// that is the defect being prevented.
///
/// Unit variants are grouped; only payload-carrying variants are bound, since
/// the payloads are the whole point.
#[test]
fn every_variant_payload_is_nameable() {
    fn inspect(kind: &NodeKind) {
        match kind {
            NodeKind::Heading { level, text, id } => {
                let _: &u8 = level;
                let _: &String = text;
                let _: &Option<String> = id;
            }
            NodeKind::CodeBlock { language, code } => {
                let _: &Option<String> = language;
                let _: &String = code;
            }
            NodeKind::List {
                ordered,
                start,
                tight,
            } => {
                let _: &bool = ordered;
                let _: &Option<u32> = start;
                let _: &bool = tight;
            }
            NodeKind::TaskCheckbox { checked } => {
                let _: &bool = checked;
            }
            NodeKind::Admonition {
                kind,
                title,
                icon,
                style,
            } => {
                let _: &AdmonitionKind = kind;
                let _: &Option<String> = title;
                let _: &Option<String> = icon;
                let _: &AdmonitionStyle = style;
            }
            NodeKind::TabItem { title } => {
                let _: &String = title;
            }
            NodeKind::SliderDeck { timer_seconds } => {
                let _: &Option<u32> = timer_seconds;
            }
            NodeKind::Slide { vertical } => {
                let _: &bool = vertical;
            }
            NodeKind::Table { alignments } => {
                let _: &Vec<TableAlignment> = alignments;
            }
            NodeKind::TableRow { header } => {
                let _: &bool = header;
            }
            NodeKind::TableCell { header, alignment } => {
                let _: &bool = header;
                let _: &TableAlignment = alignment;
            }
            NodeKind::HtmlBlock { html } => {
                let _: &String = html;
            }
            NodeKind::FootnoteDefinition { label } => {
                let _: &String = label;
            }
            NodeKind::Text(text) => {
                let _: &String = text;
            }
            NodeKind::TaskCheckboxInline { checked } => {
                let _: &bool = checked;
            }
            NodeKind::Link { url, title } => {
                let _: &String = url;
                let _: &Option<String> = title;
            }
            NodeKind::LinkReference { label, suffix } => {
                let _: &String = label;
                let _: &String = suffix;
            }
            NodeKind::FootnoteReference { label } => {
                let _: &String = label;
            }
            NodeKind::Image { url, alt } => {
                let _: &String = url;
                let _: &String = alt;
            }
            NodeKind::CodeSpan(code) => {
                let _: &String = code;
            }
            NodeKind::InlineHtml(html) => {
                let _: &String = html;
            }
            NodeKind::PlatformMention {
                username,
                platform,
                display,
            } => {
                let _: &String = username;
                let _: &String = platform;
                let _: &Option<String> = display;
            }
            NodeKind::InlineMath { content } => {
                let _: &String = content;
            }
            NodeKind::DisplayMath { content } => {
                let _: &String = content;
            }
            NodeKind::MermaidDiagram { content } => {
                let _: &String = content;
            }

            // Payload-free variants: nothing to name.
            NodeKind::Paragraph
            | NodeKind::ThematicBreak
            | NodeKind::ListItem
            | NodeKind::DefinitionList
            | NodeKind::DefinitionTerm
            | NodeKind::DefinitionDescription
            | NodeKind::Blockquote
            | NodeKind::TabGroup
            | NodeKind::Emphasis
            | NodeKind::Strong
            | NodeKind::StrongEmphasis
            | NodeKind::Strikethrough
            | NodeKind::Mark
            | NodeKind::Superscript
            | NodeKind::Subscript
            | NodeKind::HardBreak
            | NodeKind::SoftBreak => {}
        }
    }

    let document = parse("# Title\n\nSome **text** and a [link](https://example.com).\n")
        .expect("parse should succeed");

    fn walk(node: &Node, f: &fn(&NodeKind)) {
        f(&node.kind);
        for child in &node.children {
            walk(child, f);
        }
    }

    let f: fn(&NodeKind) = inspect;
    for node in &document.children {
        walk(node, &f);
    }
}

/// The render path must be usable from the crate root alone.
#[test]
fn rendering_is_reachable_from_the_crate_root() {
    let document = parse("# Title\n").expect("parse");

    for options in [
        RenderOptions::commonmark(),
        RenderOptions::new(),
        RenderOptions::untrusted(),
        RenderOptions::new()
            .with_html(HtmlPolicy::Strip)
            .with_urls(UrlPolicy::SafeSchemesOnly),
    ] {
        let html: String = render(&document, &options).expect("render");
        assert!(html.contains("Title"));
    }
}
