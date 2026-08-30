use crossterm::style::{Attribute, Color, ResetColor, SetAttribute, SetForegroundColor};
use marco_core::{Document, MarkdownIntelligenceProvider, Node, NodeKind, RenderOptions, SanitizeStats};
use serde::Serialize;
use std::time::{Duration, Instant};

use crate::ast_print::print_ast;
use crate::cli::{Args, OutputMode};

pub struct TimingInfo {
    pub sanitize: Duration,
    pub parse: Duration,
}

/// Result payload for logging.
pub struct RunPayload {
    pub ast: Option<String>,
    pub html: Option<String>,
    pub diagnostics_summary: Option<String>,
}

#[derive(Serialize)]
struct NonAsciiScalar {
    codepoint: String,
    display: String,
    byte_start: usize,
    byte_end: usize,
    char_index: usize,
}

#[derive(Serialize)]
struct JsonSpanSample {
    kind: String,
    byte_start: usize,
    byte_end: usize,
    char_start: Option<usize>,
    char_end: Option<usize>,
    preview: String,
}

#[derive(Serialize)]
struct JsonTiming {
    sanitize_us: u128,
    parse_us: u128,
    render_us: Option<u128>,
    intel_us: Option<u128>,
    total_us: u128,
}

#[derive(Serialize)]
struct JsonReport {
    mode: String,
    ast: Option<String>,
    html: Option<String>,
    diagnostics_count: usize,
    highlights_count: usize,
    diagnostics_summary: String,
    sanitize_stats: JsonSanitizeStats,
    timing: JsonTiming,
    non_ascii_scalars: Vec<NonAsciiScalar>,
    span_samples: Vec<JsonSpanSample>,
    render_error: Option<String>,
}

#[derive(Serialize)]
struct JsonSanitizeStats {
    original_bytes: usize,
    sanitized_bytes: usize,
    invalid_sequences: usize,
    null_bytes_removed: usize,
    control_chars_removed: usize,
    line_endings_normalized: usize,
    unicode_normalized: bool,
    was_valid: bool,
    had_issues: bool,
    summary: String,
}

impl From<&SanitizeStats> for JsonSanitizeStats {
    fn from(value: &SanitizeStats) -> Self {
        Self {
            original_bytes: value.original_bytes,
            sanitized_bytes: value.sanitized_bytes,
            invalid_sequences: value.invalid_sequences,
            null_bytes_removed: value.null_bytes_removed,
            control_chars_removed: value.control_chars_removed,
            line_endings_normalized: value.line_endings_normalized,
            unicode_normalized: value.unicode_normalized,
            was_valid: value.was_valid,
            had_issues: value.had_issues(),
            summary: value.summary(),
        }
    }
}

pub fn print_rule(use_color: bool) {
    if use_color {
        println!(
            "{}{}{}",
            SetForegroundColor(Color::DarkGrey),
            "═".repeat(50),
            ResetColor
        );
    } else {
        println!("{}", "═".repeat(50));
    }
}

fn print_section_header(title: &str, use_color: bool) {
    if use_color {
        println!(
            "{}{}═══ {title} {}{}",
            SetForegroundColor(Color::DarkGrey),
            SetAttribute(Attribute::Dim),
            "═".repeat(46usize.saturating_sub(title.len())),
            ResetColor,
        );
    } else {
        println!("═══ {title} {}", "═".repeat(46usize.saturating_sub(title.len())));
    }
}

// ── Stray delimiter collection ────────────────────────────────────────────────

struct StrayDelimiter {
    text: String,
    /// 1-based line number from the span, or 0 if unknown.
    line: usize,
    col: usize,
}

fn collect_stray_delimiters(nodes: &[Node], out: &mut Vec<StrayDelimiter>) {
    for node in nodes {
        if let NodeKind::Text(s) = &node.kind {
            let trimmed = s.trim();
            if !trimmed.is_empty()
                && trimmed
                    .chars()
                    .all(|c| matches!(c, '*' | '_' | '~' | '=' | '^' | '`'))
            {
                let (line, col) = node
                    .span
                    .map(|sp| (sp.start.line, sp.start.column))
                    .unwrap_or((0, 0));
                out.push(StrayDelimiter {
                    text: trimmed.to_string(),
                    line,
                    col,
                });
            }
        }
        collect_stray_delimiters(&node.children, out);
    }
}

// ── Source context helpers ────────────────────────────────────────────────────

/// Return the source line at `line` (1-based), capped to `max` chars.
fn source_line(source: &str, line: usize) -> Option<&str> {
    if line == 0 {
        return None;
    }
    source.lines().nth(line.saturating_sub(1))
}

fn caret_line(col: usize, len: usize) -> String {
    let indent = col.saturating_sub(1);
    let width = len.max(1);
    format!("{}{}", " ".repeat(indent), "^".repeat(width))
}

fn char_index_at_byte(source: &str, offset: usize) -> Option<usize> {
    source.get(..offset).map(|prefix| prefix.chars().count())
}

fn collect_non_ascii_scalars(source: &str, limit: usize) -> Vec<NonAsciiScalar> {
    let mut out = Vec::new();
    for (byte_idx, ch) in source.char_indices() {
        if !ch.is_ascii() {
            out.push(NonAsciiScalar {
                codepoint: format!("U+{:04X}", ch as u32),
                display: ch.to_string(),
                byte_start: byte_idx,
                byte_end: byte_idx + ch.len_utf8(),
                char_index: char_index_at_byte(source, byte_idx).unwrap_or(0),
            });
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

fn slice_preview(source: &str, start: usize, end: usize) -> String {
    match source.get(start..end) {
        Some(slice) => {
            let normalized = slice.replace('\n', "↵");
            let mut out = String::new();
            for (count, ch) in normalized.chars().enumerate() {
                if count >= 40 {
                    out.push('…');
                    break;
                }
                out.push(ch);
            }
            out
        }
        None => "<non-boundary slice>".to_string(),
    }
}

fn kind_name(kind: &NodeKind) -> &'static str {
    match kind {
        NodeKind::Heading { .. } => "Heading",
        NodeKind::Paragraph => "Paragraph",
        NodeKind::CodeBlock { .. } => "CodeBlock",
        NodeKind::ThematicBreak => "ThematicBreak",
        NodeKind::List { .. } => "List",
        NodeKind::ListItem => "ListItem",
        NodeKind::DefinitionList => "DefinitionList",
        NodeKind::DefinitionTerm => "DefinitionTerm",
        NodeKind::DefinitionDescription => "DefinitionDescription",
        NodeKind::TaskCheckbox { .. } => "TaskCheckbox",
        NodeKind::TaskCheckboxInline { .. } => "TaskCheckboxInline",
        NodeKind::Blockquote => "Blockquote",
        NodeKind::Admonition { .. } => "Admonition",
        NodeKind::TabGroup => "TabGroup",
        NodeKind::TabItem { .. } => "TabItem",
        NodeKind::SliderDeck { .. } => "SliderDeck",
        NodeKind::Slide { .. } => "Slide",
        NodeKind::Table { .. } => "Table",
        NodeKind::TableRow { .. } => "TableRow",
        NodeKind::TableCell { .. } => "TableCell",
        NodeKind::HtmlBlock { .. } => "HtmlBlock",
        NodeKind::FootnoteDefinition { .. } => "FootnoteDef",
        NodeKind::Text(_) => "Text",
        NodeKind::Emphasis => "Emphasis",
        NodeKind::Strong => "Strong",
        NodeKind::StrongEmphasis => "StrongEmphasis",
        NodeKind::Strikethrough => "Strikethrough",
        NodeKind::Mark => "Mark",
        NodeKind::Superscript => "Superscript",
        NodeKind::Subscript => "Subscript",
        NodeKind::Link { .. } => "Link",
        NodeKind::LinkReference { .. } => "LinkRef",
        NodeKind::FootnoteReference { .. } => "FootnoteRef",
        NodeKind::Image { .. } => "Image",
        NodeKind::CodeSpan(_) => "CodeSpan",
        NodeKind::InlineHtml(_) => "InlineHtml",
        NodeKind::HardBreak => "HardBreak",
        NodeKind::SoftBreak => "SoftBreak",
        NodeKind::PlatformMention { .. } => "PlatformMention",
        NodeKind::InlineMath { .. } => "InlineMath",
        NodeKind::DisplayMath { .. } => "DisplayMath",
        NodeKind::MermaidDiagram { .. } => "MermaidDiagram",
    }
}

fn collect_span_samples<'a>(nodes: &'a [Node], out: &mut Vec<(&'static str, &'a Node)>, limit: usize) {
    for node in nodes {
        if out.len() >= limit {
            return;
        }
        if node.span.is_some() {
            out.push((kind_name(&node.kind), node));
        }
        collect_span_samples(&node.children, out, limit);
        if out.len() >= limit {
            return;
        }
    }
}

fn collect_span_samples_json(doc: &Document, source: &str, limit: usize) -> Vec<JsonSpanSample> {
    let mut samples = Vec::new();
    collect_span_samples(&doc.children, &mut samples, limit);
    samples
        .into_iter()
        .filter_map(|(name, node)| {
            let span = node.span?;
            Some(JsonSpanSample {
                kind: name.to_string(),
                byte_start: span.start.offset,
                byte_end: span.end.offset,
                char_start: char_index_at_byte(source, span.start.offset),
                char_end: char_index_at_byte(source, span.end.offset),
                preview: slice_preview(source, span.start.offset, span.end.offset),
            })
        })
        .collect()
}

fn print_utf8_section(doc: &Document, source: &str, stats: &SanitizeStats, use_color: bool) {
    print_section_header("UTF-8", use_color);

    let char_count = source.chars().count();
    let line_count = source.lines().count().max(1);
    println!(
        "Input: {} raw bytes -> {} sanitized bytes, {} Unicode scalar(s), {} line(s)",
        stats.original_bytes, stats.sanitized_bytes, char_count, line_count
    );
    println!("Sanitizer: {}", stats.summary());

    let mut non_ascii = Vec::new();
    for (byte_idx, ch) in source.char_indices() {
        if !ch.is_ascii() {
            non_ascii.push((byte_idx, ch));
        }
        if non_ascii.len() >= 8 {
            break;
        }
    }

    if non_ascii.is_empty() {
        println!("Non-ASCII scalars: none");
    } else {
        println!("Non-ASCII scalars (first {}):", non_ascii.len());
        for (byte_idx, ch) in non_ascii {
            let char_idx = char_index_at_byte(source, byte_idx).unwrap_or(0);
            println!(
                "  U+{:04X} {:?} byte {}..{} char {}",
                ch as u32,
                ch,
                byte_idx,
                byte_idx + ch.len_utf8(),
                char_idx,
            );
        }
    }

    let mut samples = Vec::new();
    collect_span_samples(&doc.children, &mut samples, 8);
    if samples.is_empty() {
        println!("Span samples: none");
        return;
    }

    println!("Span samples (byte slice vs char range):");
    for (name, node) in samples {
        let Some(span) = node.span else {
            continue;
        };
        let char_start = char_index_at_byte(source, span.start.offset);
        let char_end = char_index_at_byte(source, span.end.offset);
        let preview = slice_preview(source, span.start.offset, span.end.offset);

        match (char_start, char_end) {
            (Some(char_start), Some(char_end)) => println!(
                "  {name:<16} bytes {:>5}..{:<5} chars {:>5}..{:<5} {:?}",
                span.start.offset,
                span.end.offset,
                char_start,
                char_end,
                preview,
            ),
            _ => println!(
                "  {name:<16} bytes {:>5}..{:<5} chars <non-boundary> {:?}",
                span.start.offset,
                span.end.offset,
                preview,
            ),
        }
    }
}

fn print_timing_section(
    timings: &TimingInfo,
    render_time: Option<Duration>,
    intel_time: Option<Duration>,
    use_color: bool,
) {
    print_section_header("TIMING", use_color);
    println!("sanitize: {} us", timings.sanitize.as_micros());
    println!("parse:    {} us", timings.parse.as_micros());
    if let Some(render_time) = render_time {
        println!("render:   {} us", render_time.as_micros());
    }
    if let Some(intel_time) = intel_time {
        println!("intel:    {} us", intel_time.as_micros());
    }
    let total = timings.sanitize
        + timings.parse
        + render_time.unwrap_or_default()
        + intel_time.unwrap_or_default();
    println!("total:    {} us", total.as_micros());
}

// ── Severity helpers ─────────────────────────────────────────────────────────

fn sev_color(severity: &str) -> Color {
    match severity {
        "Error" => Color::Red,
        "Warning" => Color::Yellow,
        "Information" => Color::Cyan,
        "Hint" => Color::DarkGrey,
        _ => Color::White,
    }
}

// ── Main diagnostics printer ─────────────────────────────────────────────────

/// Runs intelligence diagnostics + stray-delimiter scan and prints a
/// compiler-style DIAGNOSTICS section.  Always called from ast/html/both modes.
/// Returns a summary string for the log payload.
fn print_diagnostics_section(doc: &Document, source: &str, use_color: bool) -> String {
    let mut provider = MarkdownIntelligenceProvider::new();
    provider.update_document(doc.clone());
    let diagnostics = provider.diagnostics();

    // Stray delimiter warnings from the AST
    let mut strays: Vec<StrayDelimiter> = Vec::new();
    collect_stray_delimiters(&doc.children, &mut strays);

    let total = diagnostics.len() + strays.len();

    if total == 0 {
        // Clean — show a brief green footer
        if use_color {
            println!(
                "{}{}✓ no issues{}",
                SetForegroundColor(Color::Green),
                SetAttribute(Attribute::Dim),
                ResetColor,
            );
        } else {
            println!("✓ no issues");
        }
        return "0 issue(s)".to_string();
    }

    println!();
    if use_color {
        println!(
            "{}{}─── DIAGNOSTICS ({total}) {}{}",
            SetForegroundColor(Color::DarkGrey),
            SetAttribute(Attribute::Dim),
            "─".repeat(36usize.saturating_sub(total.to_string().len())),
            ResetColor,
        );
    } else {
        println!(
            "─── DIAGNOSTICS ({total}) {}",
            "─".repeat(36usize.saturating_sub(total.to_string().len()))
        );
    }

    // Intelligence diagnostics
    for d in &diagnostics {
        let severity = format!("{:?}", d.severity);
        let color = sev_color(&severity);
        let line = d.span.start.line;
        let col = d.span.start.column;

        if use_color {
            println!(
                "  {}{}[{severity}]{} {}  (line {line}, col {col})",
                SetAttribute(Attribute::Bold),
                SetForegroundColor(color),
                ResetColor,
                d.message,
            );
        } else {
            println!("  [{severity}] {}  (line {line}, col {col})", d.message);
        }

        // Source line + caret
        if let Some(src_line) = source_line(source, line) {
            let span_len = d.span.end.offset.saturating_sub(d.span.start.offset);
            let display_len = span_len.min(src_line.len().saturating_sub(col.saturating_sub(1)));
            if use_color {
                println!("    {} │ {src_line}", line);
                println!(
                    "    {} │ {}{}{}",
                    " ".repeat(line.to_string().len()),
                    SetForegroundColor(color),
                    caret_line(col, display_len.max(1)),
                    ResetColor,
                );
            } else {
                println!("    {line} │ {src_line}");
                println!("    {} │ {}", " ".repeat(line.to_string().len()), caret_line(col, display_len.max(1)));
            }
        }
    }

    // Stray delimiter warnings
    for s in &strays {
        let color = Color::Yellow;
        if use_color {
            println!(
                "  {}{}[Warning]{} unmatched delimiter {:?}  (line {}, col {})",
                SetAttribute(Attribute::Bold),
                SetForegroundColor(color),
                ResetColor,
                s.text,
                s.line,
                s.col,
            );
        } else {
            println!(
                "  [Warning] unmatched delimiter {:?}  (line {}, col {})",
                s.text, s.line, s.col,
            );
        }

        if s.line > 0 {
            if let Some(src_line) = source_line(source, s.line) {
                if use_color {
                    println!("    {} │ {src_line}", s.line);
                    println!(
                        "    {} │ {}{}{}",
                        " ".repeat(s.line.to_string().len()),
                        SetForegroundColor(color),
                        caret_line(s.col, s.text.len()),
                        ResetColor,
                    );
                } else {
                    println!("    {} │ {src_line}", s.line);
                    println!(
                        "    {} │ {}",
                        " ".repeat(s.line.to_string().len()),
                        caret_line(s.col, s.text.len()),
                    );
                }
            }
        }
    }

    format!("{total} issue(s)")
}

fn run_ast_mode_inner(
    doc: &Document,
    source: &str,
    stats: &SanitizeStats,
    timings: &TimingInfo,
    args: &Args,
    include_extra_sections: bool,
) -> RunPayload {
    print_section_header("AST", !args.no_color);
    let tree = print_ast(
        doc,
        Some(source),
        !args.no_color,
        args.compact,
        args.spans,
        args.excerpts,
    );
    print!("{tree}");
    let diag_summary = print_diagnostics_section(doc, source, !args.no_color);
    if include_extra_sections {
        if args.utf8 {
            println!();
            print_utf8_section(doc, source, stats, !args.no_color);
        }
        if args.time {
            println!();
            print_timing_section(timings, None, None, !args.no_color);
        }
    }
    RunPayload {
        ast: Some(tree),
        html: None,
        diagnostics_summary: Some(diag_summary),
    }
}

pub fn run_ast_mode(
    doc: &Document,
    source: &str,
    stats: &SanitizeStats,
    timings: &TimingInfo,
    args: &Args,
) -> RunPayload {
    run_ast_mode_inner(doc, source, stats, timings, args, true)
}

fn run_html_mode_inner(
    doc: &Document,
    source: &str,
    stats: &SanitizeStats,
    timings: &TimingInfo,
    args: &Args,
    include_extra_sections: bool,
) -> RunPayload {
    // `commonmark()` rather than `default()`: this tool exists to show what the
    // renderer does, so it must not silently apply a security policy the user
    // did not ask for. Matches marco-core 1.x output.
    let options = RenderOptions::commonmark().with_syntax_highlighting(args.syntax);
    let render_started = Instant::now();
    match marco_core::render(doc, &options) {
        Ok(html) => {
            let render_time = render_started.elapsed();
            print_section_header("HTML", !args.no_color);
            println!("{html}");
            let diag_summary = print_diagnostics_section(doc, source, !args.no_color);
            if include_extra_sections {
                if args.utf8 {
                    println!();
                    print_utf8_section(doc, source, stats, !args.no_color);
                }
                if args.time {
                    println!();
                    print_timing_section(timings, Some(render_time), None, !args.no_color);
                }
            }
            RunPayload {
                ast: None,
                html: Some(html),
                diagnostics_summary: Some(diag_summary),
            }
        }
        Err(e) => {
            eprintln!("render error: {e}");
            RunPayload { ast: None, html: None, diagnostics_summary: None }
        }
    }
}

pub fn run_html_mode(
    doc: &Document,
    source: &str,
    stats: &SanitizeStats,
    timings: &TimingInfo,
    args: &Args,
) -> RunPayload {
    run_html_mode_inner(doc, source, stats, timings, args, true)
}

pub fn run_intel_mode(
    doc: &Document,
    source: &str,
    stats: &SanitizeStats,
    timings: &TimingInfo,
    args: &Args,
) -> RunPayload {
    print_section_header("INTELLIGENCE", !args.no_color);

    let started = Instant::now();
    let mut provider = MarkdownIntelligenceProvider::new();
    provider.update_document(doc.clone());

    let diagnostics = provider.diagnostics();
    let highlights = provider.highlights(source);
    let intel_time = started.elapsed();

    let use_color = !args.no_color;

    if diagnostics.is_empty() {
        if use_color {
            println!(
                "{}{}(no diagnostics){}",
                SetForegroundColor(Color::DarkGrey),
                SetAttribute(Attribute::Dim),
                ResetColor,
            );
        } else {
            println!("(no diagnostics)");
        }
    } else {
        for d in &diagnostics {
            let severity = format!("{:?}", d.severity);
            let color = sev_color(&severity);
            if use_color {
                println!(
                    "[{}{}{}] {}  ({}:{})",
                    SetForegroundColor(color),
                    severity,
                    ResetColor,
                    d.message,
                    d.span.start.line,
                    d.span.start.column,
                );
            } else {
                println!(
                    "[{severity}] {}  ({}:{})",
                    d.message,
                    d.span.start.line,
                    d.span.start.column,
                );
            }
        }
    }

    println!();

    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for h in &highlights {
        *counts.entry(format!("{:?}", h.tag)).or_insert(0) += 1;
    }
    let mut sorted: Vec<_> = counts.iter().collect();
    sorted.sort_by_key(|(k, _)| k.as_str());

    println!("Highlight spans ({} total):", highlights.len());
    for (tag, count) in sorted {
        println!("  {tag}: {count}");
    }

    let summary = format!(
        "{} diagnostic(s), {} highlight span(s)",
        diagnostics.len(),
        highlights.len()
    );

    if args.utf8 {
        println!();
        print_utf8_section(doc, source, stats, !args.no_color);
    }
    if args.time {
        println!();
        print_timing_section(timings, None, Some(intel_time), !args.no_color);
    }

    RunPayload {
        ast: None,
        html: None,
        diagnostics_summary: Some(summary),
    }
}

pub fn run_both_mode(
    doc: &Document,
    source: &str,
    stats: &SanitizeStats,
    timings: &TimingInfo,
    args: &Args,
) -> RunPayload {
    let ast_payload = run_ast_mode_inner(doc, source, stats, timings, args, false);
    print_rule(!args.no_color);
    let html_payload = run_html_mode_inner(doc, source, stats, timings, args, true);
    RunPayload {
        ast: ast_payload.ast,
        html: html_payload.html,
        diagnostics_summary: ast_payload.diagnostics_summary,
    }
}

pub fn run_json_mode(
    doc: &Document,
    source: &str,
    stats: &SanitizeStats,
    timings: &TimingInfo,
    mode: &OutputMode,
    args: &Args,
) -> RunPayload {
    let ast = if matches!(mode, OutputMode::Ast | OutputMode::Both) {
        Some(print_ast(
            doc,
            Some(source),
            false,
            args.compact,
            args.spans,
            args.excerpts,
        ))
    } else {
        None
    };

    let (html, render_us, render_error) = if matches!(mode, OutputMode::Html | OutputMode::Both) {
        let started = Instant::now();
        match marco_core::render(
            doc,
            &RenderOptions::commonmark().with_syntax_highlighting(args.syntax),
        ) {
            Ok(html) => (Some(html), Some(started.elapsed().as_micros()), None),
            Err(err) => (None, Some(started.elapsed().as_micros()), Some(err.to_string())),
        }
    } else {
        (None, None, None)
    };

    let intel_started = Instant::now();
    let mut provider = MarkdownIntelligenceProvider::new();
    provider.update_document(doc.clone());
    let diagnostics = provider.diagnostics();
    let highlights = provider.highlights(source);
    let intel_us = if matches!(mode, OutputMode::Intel) {
        Some(intel_started.elapsed().as_micros())
    } else {
        None
    };

    let diagnostics_summary = format!(
        "{} diagnostic(s), {} highlight span(s)",
        diagnostics.len(),
        highlights.len()
    );

    let report = JsonReport {
        mode: mode.to_string(),
        ast: ast.clone(),
        html: html.clone(),
        diagnostics_count: diagnostics.len(),
        highlights_count: highlights.len(),
        diagnostics_summary: diagnostics_summary.clone(),
        sanitize_stats: JsonSanitizeStats::from(stats),
        timing: JsonTiming {
            sanitize_us: timings.sanitize.as_micros(),
            parse_us: timings.parse.as_micros(),
            render_us,
            intel_us,
            total_us: timings.sanitize.as_micros()
                + timings.parse.as_micros()
                + render_us.unwrap_or(0)
                + intel_us.unwrap_or(0),
        },
        non_ascii_scalars: if args.utf8 {
            collect_non_ascii_scalars(source, 16)
        } else {
            Vec::new()
        },
        span_samples: if args.spans || args.utf8 || args.excerpts {
            collect_span_samples_json(doc, source, 16)
        } else {
            Vec::new()
        },
        render_error: render_error.clone(),
    };

    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(err) => eprintln!("json serialization error: {err}"),
    }

    RunPayload {
        ast,
        html,
        diagnostics_summary: Some(diagnostics_summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_index_tracks_utf8_boundaries() {
        let source = "Tëst 🎨";
        assert_eq!(char_index_at_byte(source, 0), Some(0));
        assert_eq!(char_index_at_byte(source, 1), Some(1));
        assert_eq!(char_index_at_byte(source, 3), Some(2));
        assert_eq!(char_index_at_byte(source, 10), Some(6));
        assert_eq!(char_index_at_byte(source, 7), None);
    }
}
