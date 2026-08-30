use crate::error::MarcoError;
use std::borrow::Cow;
use std::panic;

use mermaid_rs_renderer::{
    render_with_options as mermaid_render_with_opts, RenderOptions as MermaidRenderOptions,
    Theme as MermaidTheme,
};

const MERMAID_MAX_CHARS: usize = 100_000;

/// Create Mermaid theme configuration from the active preview mode hint.
pub(crate) fn create_mermaid_theme(theme_hint: &str) -> MermaidTheme {
    let normalized = theme_hint.trim().to_ascii_lowercase();
    let is_dark = normalized == "dark"
        || normalized.ends_with("-dark")
        || normalized.contains("theme-dark")
        || normalized.contains("dark");

    if is_dark {
        let mut theme = MermaidTheme::modern();

        theme.background = "transparent".to_string();
        theme.text_color = "#f0f6fc".to_string();
        theme.primary_text_color = "#f0f6fc".to_string();
        theme.pie_title_text_color = "#f0f6fc".to_string();
        theme.pie_section_text_color = "#f0f6fc".to_string();
        theme.pie_legend_text_color = "#f0f6fc".to_string();
        theme.git_commit_label_color = "#f0f6fc".to_string();
        theme.git_tag_label_color = "#f0f6fc".to_string();

        theme.primary_color = "#161b22".to_string();
        theme.secondary_color = "#0d1117".to_string();
        theme.tertiary_color = "#1f2937".to_string();

        theme.primary_border_color = "#30363d".to_string();
        theme.line_color = "#8b949e".to_string();
        theme.edge_label_background = "rgba(13, 17, 23, 0.92)".to_string();

        theme.cluster_background = "#0d1117".to_string();
        theme.cluster_border = "#30363d".to_string();

        theme.sequence_actor_fill = "#161b22".to_string();
        theme.sequence_actor_border = "#30363d".to_string();
        theme.sequence_actor_line = "#8b949e".to_string();
        theme.sequence_note_fill = "#1f2937".to_string();
        theme.sequence_note_border = "#30363d".to_string();
        theme.sequence_activation_fill = "#21262d".to_string();
        theme.sequence_activation_border = "#30363d".to_string();

        theme.git_commit_label_background = "rgba(13, 17, 23, 0.9)".to_string();
        theme.git_tag_label_background = "rgba(13, 17, 23, 0.9)".to_string();
        theme.git_tag_label_border = "#30363d".to_string();
        theme.pie_colors = [
            "#58a6ff", "#3fb950", "#d29922", "#f778ba", "#a371f7", "#39c5cf", "#ff7b72", "#9e6a03",
            "#7ee787", "#79c0ff", "#ffa657", "#c9d1d9",
        ]
        .map(|c| c.to_string());
        theme.pie_stroke_color = "#30363d".to_string();
        theme.pie_outer_stroke_color = "#30363d".to_string();
        theme.pie_stroke_width = 1.5;
        theme.pie_outer_stroke_width = 1.5;
        theme.pie_opacity = 0.95;

        theme
    } else {
        let mut theme = MermaidTheme::mermaid_default();

        theme.background = "transparent".to_string();
        theme.text_color = "#24292f".to_string();
        theme.primary_text_color = "#24292f".to_string();
        theme.pie_title_text_color = "#24292f".to_string();
        theme.pie_section_text_color = "#24292f".to_string();
        theme.pie_legend_text_color = "#24292f".to_string();
        theme.git_commit_label_color = "#24292f".to_string();
        theme.git_tag_label_color = "#24292f".to_string();

        theme.primary_color = "#f6f8fa".to_string();
        theme.secondary_color = "#ffffff".to_string();
        theme.tertiary_color = "#f6f8fa".to_string();
        theme.primary_border_color = "#d0d7de".to_string();
        theme.line_color = "#57606a".to_string();
        theme.edge_label_background = "rgba(255, 255, 255, 0.92)".to_string();
        theme.cluster_background = "#f6f8fa".to_string();
        theme.cluster_border = "#d0d7de".to_string();

        theme.sequence_actor_fill = "#f6f8fa".to_string();
        theme.sequence_actor_border = "#d0d7de".to_string();
        theme.sequence_actor_line = "#57606a".to_string();
        theme.sequence_note_fill = "#ffffff".to_string();
        theme.sequence_note_border = "#d0d7de".to_string();
        theme.sequence_activation_fill = "#f6f8fa".to_string();
        theme.sequence_activation_border = "#d0d7de".to_string();

        theme.git_commit_label_background = "rgba(255, 255, 255, 0.92)".to_string();
        theme.git_tag_label_background = "rgba(255, 255, 255, 0.92)".to_string();
        theme.git_tag_label_border = "#d0d7de".to_string();
        theme.pie_colors = [
            "#1f6feb", "#2da44e", "#bf8700", "#cf4f8b", "#8250df", "#0a7d89", "#cf222e", "#9a6700",
            "#1a7f37", "#0550ae", "#bc4c00", "#57606a",
        ]
        .map(|c| c.to_string());
        theme.pie_stroke_color = "#57606a".to_string();
        theme.pie_outer_stroke_color = "#57606a".to_string();
        theme.pie_stroke_width = 1.25;
        theme.pie_outer_stroke_width = 1.25;
        theme.pie_opacity = 0.92;

        theme
    }
}

/// Normalize and validate Mermaid source before rendering.
fn normalize_mermaid_source(diagram: &str) -> Result<Cow<'_, str>, MarcoError> {
    let trimmed = diagram.trim();
    if trimmed.is_empty() {
        return Err(MarcoError::diagram("Empty Mermaid diagram"));
    }

    if trimmed.len() > MERMAID_MAX_CHARS {
        return Err(MarcoError::diagram(format!(
            "Mermaid diagram too large ({} chars > max {MERMAID_MAX_CHARS})",
            trimmed.len(),
        )));
    }

    if let Some(first_line_end) = trimmed.find('\n') {
        let first_line = trimmed[..first_line_end].trim();
        if first_line.starts_with("```") || first_line.starts_with("~~~") {
            let fence = if first_line.starts_with("~~~") {
                "~~~"
            } else {
                "```"
            };
            let after_fence = first_line[fence.len()..].trim().to_ascii_lowercase();
            if after_fence.starts_with("mermaid") {
                let body = &trimmed[first_line_end + 1..];
                if let Some(last_nl) = body.rfind('\n') {
                    let last_line = body[last_nl + 1..].trim();
                    if last_line == fence {
                        let inner = body[..last_nl].trim();
                        if inner.is_empty() {
                            return Err(MarcoError::diagram("Empty Mermaid diagram"));
                        }
                        if inner.len() > MERMAID_MAX_CHARS {
                            return Err(MarcoError::diagram(format!(
                                "Mermaid diagram too large ({} chars > max {MERMAID_MAX_CHARS})",
                                inner.len(),
                            )));
                        }
                        return Ok(Cow::Owned(inner.to_string()));
                    }
                }
            }
        }
    }

    Ok(Cow::Borrowed(trimmed))
}

/// Remove Mermaid interaction directives from diagram source.
///
/// A `click` or `link` directive makes the renderer emit
/// `<a href="…" xlink:href="…">` around a node, with **no filtering of the
/// URL** — `click A "javascript:alert(1)"` produces a live `javascript:` anchor
/// inside the SVG, which the HTML renderer then splices into the page verbatim.
/// `mermaid-rs-renderer` 0.3 exposes no option to disable this; its
/// `RenderOptions` carries only a theme and a layout.
///
/// The directives are dropped from the *source* rather than filtered out of the
/// rendered SVG. Rewriting the output would mean re-parsing SVG whose shape this
/// crate does not control, and filtering the URL in place would mean
/// duplicating the upstream quoted-token tokenizer — a parser differential of
/// exactly the kind that produces bypasses. Matching the directive *line* needs
/// no such duplication: this keys on the same prefixes
/// `mermaid_rs_renderer::parser::parse_click_line` keys on, so the two cannot
/// disagree about which lines are interaction directives.
///
/// The cost is that a legitimate `click A "https://example.com"` is also
/// dropped under a non-permissive policy. That is deliberate: diagram links are
/// a rarely used feature and the alternative is a URL filter that can drift out
/// of step with the parser it is guarding.
///
/// `linkStyle` is untouched — it has no space after `link` and carries no URL.
pub(crate) fn strip_interaction_directives(source: &str) -> Cow<'_, str> {
    let is_directive = |line: &str| {
        let lowered = line.trim_start().to_ascii_lowercase();
        lowered.starts_with("click ") || lowered.starts_with("link ")
    };

    if !source.lines().any(is_directive) {
        return Cow::Borrowed(source);
    }

    let kept: Vec<&str> = source.lines().filter(|line| !is_directive(line)).collect();
    Cow::Owned(kept.join("\n"))
}

/// Whether rendered SVG still carries a link, after directives were stripped.
///
/// A post-condition on [`strip_interaction_directives`], not a second filter.
/// If this ever fires, some path produced an anchor that stripping did not
/// anticipate — a new upstream feature, or a directive form this crate does not
/// recognise — and the caller degrades to the code-block fallback rather than
/// emitting markup it cannot vouch for.
pub(crate) fn svg_contains_link(svg: &str) -> bool {
    let lowered = svg.to_ascii_lowercase();
    lowered.contains("<a ") || lowered.contains("href=")
}

/// Render Mermaid diagram to SVG.
///
/// When `allow_interaction` is false, `click`/`link` directives are stripped
/// before rendering and the result is checked for stray anchors; a diagram that
/// still contains one is rejected so the caller can fall back to rendering the
/// source as a code block.
pub fn render_mermaid_diagram(
    diagram: &str,
    theme_hint: &str,
    allow_interaction: bool,
) -> Result<String, MarcoError> {
    let normalized = normalize_mermaid_source(diagram)?;

    let source = if allow_interaction {
        normalized
    } else {
        match strip_interaction_directives(&normalized) {
            Cow::Borrowed(_) => normalized,
            Cow::Owned(stripped) => Cow::Owned(stripped),
        }
    };

    let diagram_owned = source.into_owned();
    let theme = create_mermaid_theme(theme_hint);
    let mut options = MermaidRenderOptions::modern();
    options.theme = theme;

    let result = panic::catch_unwind(|| mermaid_render_with_opts(&diagram_owned, options));

    match result {
        Ok(Ok(svg)) => {
            if !allow_interaction && svg_contains_link(&svg) {
                return Err(MarcoError::diagram(
                    "Mermaid output contains a link after interaction directives were stripped",
                ));
            }
            Ok(svg)
        }
        // The upstream error is stringified rather than carried: it is an
        // `anyhow::Error` from a feature-gated dependency, and putting it in a
        // variant would make the public API depend on `render-diagrams`.
        Ok(Err(e)) => Err(MarcoError::diagram(e.to_string())),
        Err(_) => Err(MarcoError::diagram(
            "Mermaid rendering panicked (likely invalid diagram syntax)",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test_create_mermaid_theme_light_dark() {
        let light = create_mermaid_theme("light");
        let dark = create_mermaid_theme("dark");

        assert_eq!(light.background, "transparent");
        assert_eq!(dark.background, "transparent");
        assert_eq!(light.text_color, "#24292f");
        assert_eq!(dark.text_color, "#f0f6fc");
        assert_eq!(light.pie_colors[0], "#1f6feb");
        assert_eq!(dark.pie_colors[0], "#58a6ff");
        assert_ne!(light.line_color, dark.line_color);
    }

    #[test]
    fn smoke_test_render_mermaid_diagram_mode_sensitive() {
        let diagram = "flowchart TD\nA[Start] --> B[End]";

        let svg_light =
            render_mermaid_diagram(diagram, "light", true).expect("light mermaid render");
        let svg_dark = render_mermaid_diagram(diagram, "dark", true).expect("dark mermaid render");

        assert!(svg_light.contains("<svg"));
        assert!(svg_dark.contains("<svg"));
        assert!(
            svg_light.contains("transparent")
                || svg_light.contains("rgba(255, 255, 255, 0.92)")
                || svg_light.contains("#24292f")
        );
        assert!(
            svg_dark.contains("transparent")
                || svg_dark.contains("rgba(34, 40, 49, 0.85)")
                || svg_dark.contains("#e6edf3")
                || svg_dark.contains("#f0f6fc")
        );
    }

    /// Asserts the variant, not the message. Message wording is not a contract;
    /// the variant a caller matches on is.
    #[test]
    fn smoke_test_render_mermaid_diagram_rejects_empty() {
        let err =
            render_mermaid_diagram("   \n\t", "dark", true).expect_err("expected empty rejection");
        assert!(matches!(err, MarcoError::Diagram { .. }), "got {err:?}");
        assert!(err.to_string().contains("Empty Mermaid diagram"));
    }

    #[test]
    fn smoke_test_render_mermaid_diagram_rejects_too_large() {
        let oversized = "A".repeat(MERMAID_MAX_CHARS + 1);
        let err =
            render_mermaid_diagram(&oversized, "light", true).expect_err("expected size rejection");
        assert!(matches!(err, MarcoError::Diagram { .. }), "got {err:?}");
        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn smoke_test_render_mermaid_diagram_accepts_fenced_input() {
        let fenced = "```mermaid\nflowchart LR\nA[Start] --> B[End]\n```";
        let svg =
            render_mermaid_diagram(fenced, "light", true).expect("fenced mermaid should render");
        assert!(svg.contains("<svg"));
    }

    /// B5: a `click` directive makes the renderer emit an unfiltered anchor.
    /// This is the vector, pinned so a dependency upgrade cannot silently
    /// change what `allow_interaction: true` means.
    #[test]
    fn click_directive_emits_an_unfiltered_anchor_when_interaction_is_allowed() {
        let diagram =
            "graph TD\n  A[Click me] --> B[Other]\n  click A \"javascript:alert(1)\" \"tip\"";
        let svg = render_mermaid_diagram(diagram, "light", true).expect("should render");

        assert!(
            svg.contains("javascript:alert(1)"),
            "upstream stopped emitting the anchor; strip_interaction_directives may be obsolete"
        );
        assert!(svg_contains_link(&svg));
    }

    #[test]
    fn stripping_removes_the_anchor_and_keeps_the_diagram() {
        let diagram =
            "graph TD\n  A[Click me] --> B[Other]\n  click A \"javascript:alert(1)\" \"tip\"";
        let svg = render_mermaid_diagram(diagram, "light", false).expect("should still render");

        assert!(svg.contains("<svg"), "the diagram itself must survive");
        assert!(!svg.contains("javascript:"));
        assert!(!svg_contains_link(&svg));
    }

    #[test]
    fn stripping_covers_every_directive_form_upstream_accepts() {
        for line in [
            "click A \"javascript:alert(1)\"",
            "click A href \"javascript:alert(1)\"",
            "click A \"javascript:alert(1)\" \"title\"",
            "click A \"javascript:alert(1)\" _blank",
            "CLICK A \"javascript:alert(1)\"",
            "  click A \"javascript:alert(1)\"",
            "link A \"javascript:alert(1)\"",
            "LINK A href \"javascript:alert(1)\" \"t\" _blank",
        ] {
            let diagram = format!("graph TD\n  A[X] --> B[Y]\n  {line}");
            let svg = render_mermaid_diagram(&diagram, "light", false)
                .unwrap_or_else(|e| panic!("{line:?} should render: {e}"));
            assert!(!svg_contains_link(&svg), "anchor survived for {line:?}");
        }
    }

    /// `linkStyle` is not an interaction directive and must survive stripping.
    #[test]
    fn link_style_is_not_stripped() {
        let source = "graph TD\n  A --> B\n  linkStyle 0 stroke:#f00\n";
        assert!(matches!(
            strip_interaction_directives(source),
            Cow::Borrowed(_)
        ));
    }

    #[test]
    fn stripping_is_borrow_free_when_there_is_nothing_to_strip() {
        assert!(matches!(
            strip_interaction_directives("graph TD\n  A --> B"),
            Cow::Borrowed(_)
        ));
    }
}
