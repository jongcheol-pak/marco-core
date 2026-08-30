//! Rendering options and configuration.

use serde::{Deserialize, Serialize};

/// How the renderer treats author-written raw HTML.
///
/// CommonMark specifies that raw HTML passes through to the output. That is
/// correct when the document author and the document reader are the same
/// person, and it is an author-to-reader code execution channel when they are
/// not.
///
/// # Comments are a special case
///
/// HTML comments are removed under **both** [`Escape`](Self::Escape) and
/// [`Strip`](Self::Strip). Escaping one would invert its meaning: an invisible
/// `<!-- internal note -->` would become visible text in the reader's face.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum HtmlPolicy {
    /// Emit raw HTML verbatim. CommonMark-conformant, and unsafe for untrusted
    /// input.
    Allow,
    /// Emit raw HTML as escaped text, so the reader sees the markup the author
    /// wrote. The honest choice for a comment field: nothing is lost, and an
    /// author who wrote HTML by accident gets feedback.
    #[default]
    Escape,
    /// Remove raw HTML.
    ///
    /// The effect differs by node kind, and both behaviours are intended.
    /// Inline HTML carries only the tag, so `<b>bold</b>` becomes `bold` —
    /// markup lost, **content kept**. An HTML *block* carries its text too, so
    /// `<div>Note</div>` is removed **including the word "Note"**, leaving
    /// `<!-- raw HTML omitted -->` in its place so the loss is greppable.
    Strip,
}

/// How the renderer treats author-supplied URLs in links and images.
///
/// Both filtering policies read the same normalised scheme — see
/// [`crate::logic::url_scheme`], where the decoding that makes either of them
/// trustworthy actually happens.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum UrlPolicy {
    /// Emit every URL unchanged. CommonMark behaviour, and unsafe for untrusted
    /// input: a `javascript:` link executes on click.
    Allow,
    /// Block the schemes known to carry active content — `javascript`,
    /// `vbscript`, `file`, `data` — exempting inert base64 raster images on
    /// image nodes. Matches cmark, comrak, goldmark and markdown-it.
    ///
    /// **Unknown schemes pass.** That is what makes this a denylist, and it is
    /// the reason [`SafeSchemesOnly`](Self::SafeSchemesOnly) exists.
    #[default]
    BlockDangerous,
    /// Emit only known-safe schemes and relative URLs; block everything else,
    /// including schemes nobody has thought of yet. The right choice for a
    /// public comment field.
    SafeSchemesOnly,
}

/// Configuration options for HTML rendering.
///
/// Construct with [`RenderOptions::new`], [`RenderOptions::commonmark`] or
/// [`RenderOptions::untrusted`] and refine with the `with_*` builders. The type
/// is `#[non_exhaustive]`, so a struct literal will not compile from outside
/// this crate — which is what lets a future security option be a minor bump
/// rather than a major one.
///
/// # Choosing a constructor
///
/// | Constructor | `html` | `urls` | For |
/// |---|---|---|---|
/// | [`commonmark`](Self::commonmark) | `Allow` | `Allow` | trusted single-author input; spec conformance |
/// | [`new`](Self::new) / `default` | `Escape` | `BlockDangerous` | a desktop editor |
/// | [`untrusted`](Self::untrusted) | `Escape` | `SafeSchemesOnly` | a server rendering one user's Markdown to other readers |
#[non_exhaustive]
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub struct RenderOptions {
    /// Enable syntax highlighting for fenced code blocks when possible.
    pub syntax_highlighting: bool,
    /// Show line numbers in rendered code blocks.
    pub line_numbers: bool,
    /// Theme name used by renderer and highlighter integrations.
    pub theme: String,
    /// How author-written raw HTML is treated.
    ///
    /// Defaults to [`HtmlPolicy::Escape`] when absent from serialized data, so
    /// configuration written by marco-core 1.x loads with the safe policy
    /// rather than failing.
    #[serde(default)]
    pub html: HtmlPolicy,
    /// How author-supplied URLs in links and images are treated.
    ///
    /// Defaults to [`UrlPolicy::BlockDangerous`] when absent from serialized
    /// data, for the same reason as [`html`](Self::html).
    #[serde(default)]
    pub urls: UrlPolicy,
}

impl RenderOptions {
    /// Safe defaults: [`HtmlPolicy::Escape`] and [`UrlPolicy::BlockDangerous`].
    ///
    /// Equivalent to [`Default::default`]. Matches what cmark, comrak, goldmark
    /// and markdown-it do by default.
    pub fn new() -> Self {
        Self {
            syntax_highlighting: true,
            line_numbers: false,
            theme: "github".to_string(),
            html: HtmlPolicy::Escape,
            urls: UrlPolicy::BlockDangerous,
        }
    }

    /// CommonMark-conformant: [`HtmlPolicy::Allow`] and [`UrlPolicy::Allow`].
    ///
    /// Output is byte-identical to marco-core 1.x. Use this for input you
    /// trust — a desktop editor rendering the file its own user is editing —
    /// and for CommonMark conformance testing.
    ///
    /// Do not use it for input written by one person and displayed to another.
    pub fn commonmark() -> Self {
        Self::new()
            .with_html(HtmlPolicy::Allow)
            .with_urls(UrlPolicy::Allow)
    }

    /// Strictest: [`HtmlPolicy::Escape`] and [`UrlPolicy::SafeSchemesOnly`].
    ///
    /// For a server rendering Markdown written by one user and displayed to
    /// another. This constructor may gain further protections in future minor
    /// versions without any consumer changing a line — which is the point of
    /// having it rather than a documented combination of flags.
    pub fn untrusted() -> Self {
        Self::new()
            .with_html(HtmlPolicy::Escape)
            .with_urls(UrlPolicy::SafeSchemesOnly)
    }

    /// Set the raw-HTML policy.
    pub fn with_html(mut self, policy: HtmlPolicy) -> Self {
        self.html = policy;
        self
    }

    /// Set the URL policy.
    pub fn with_urls(mut self, policy: UrlPolicy) -> Self {
        self.urls = policy;
        self
    }

    /// Set the theme name used by the renderer and highlighter.
    pub fn with_theme(mut self, theme: impl Into<String>) -> Self {
        self.theme = theme.into();
        self
    }

    /// Enable or disable syntax highlighting for fenced code blocks.
    pub fn with_syntax_highlighting(mut self, enabled: bool) -> Self {
        self.syntax_highlighting = enabled;
        self
    }

    /// Show or hide line numbers in rendered code blocks.
    pub fn with_line_numbers(mut self, enabled: bool) -> Self {
        self.line_numbers = enabled;
        self
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_safe() {
        let opts = RenderOptions::default();
        assert_eq!(opts.html, HtmlPolicy::Escape);
        assert_eq!(opts.urls, UrlPolicy::BlockDangerous);
    }

    #[test]
    fn new_matches_default() {
        let a = RenderOptions::new();
        let b = RenderOptions::default();
        assert_eq!(a.html, b.html);
        assert_eq!(a.urls, b.urls);
        assert_eq!(a.theme, b.theme);
        assert_eq!(a.syntax_highlighting, b.syntax_highlighting);
        assert_eq!(a.line_numbers, b.line_numbers);
    }

    #[test]
    fn commonmark_preserves_1_x_behaviour() {
        let opts = RenderOptions::commonmark();
        assert_eq!(opts.html, HtmlPolicy::Allow);
        assert_eq!(opts.urls, UrlPolicy::Allow);
        // The non-security defaults must match 1.3.3 exactly, or "byte-identical
        // output" would not hold for reasons unrelated to policy.
        assert!(opts.syntax_highlighting);
        assert!(!opts.line_numbers);
        assert_eq!(opts.theme, "github");
    }

    #[test]
    fn untrusted_is_the_strictest_combination() {
        let opts = RenderOptions::untrusted();
        assert_eq!(opts.html, HtmlPolicy::Escape);
        assert_eq!(opts.urls, UrlPolicy::SafeSchemesOnly);
    }

    #[test]
    fn builders_compose_and_do_not_disturb_other_fields() {
        let opts = RenderOptions::commonmark()
            .with_theme("dracula")
            .with_line_numbers(true)
            .with_syntax_highlighting(false)
            .with_html(HtmlPolicy::Strip);

        assert_eq!(opts.theme, "dracula");
        assert!(opts.line_numbers);
        assert!(!opts.syntax_highlighting);
        assert_eq!(opts.html, HtmlPolicy::Strip);
        // Untouched by the builders above.
        assert_eq!(opts.urls, UrlPolicy::Allow);
    }

    /// Configuration serialized by marco-core 1.x has neither policy field.
    /// It must load rather than fail, and it must load *safe*.
    #[test]
    fn legacy_serialized_options_deserialize_to_safe_policies() {
        let legacy = r#"(syntax_highlighting: true, line_numbers: false, theme: "github")"#;
        let opts: RenderOptions =
            ron::from_str(legacy).expect("1.x configuration should still load");

        assert_eq!(opts.html, HtmlPolicy::Escape);
        assert_eq!(opts.urls, UrlPolicy::BlockDangerous);
        assert_eq!(opts.theme, "github");
    }

    #[test]
    fn policies_round_trip_through_serde() {
        let opts = RenderOptions::untrusted().with_html(HtmlPolicy::Strip);
        let encoded = ron::to_string(&opts).expect("serialize");
        let decoded: RenderOptions = ron::from_str(&encoded).expect("deserialize");

        assert_eq!(decoded.html, HtmlPolicy::Strip);
        assert_eq!(decoded.urls, UrlPolicy::SafeSchemesOnly);
    }

    /// The mermaid render cache keys on `RenderOptions`, so `Hash` must stay
    /// derivable and must actually distinguish the policies.
    #[test]
    fn hash_distinguishes_policies() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let hash_of = |opts: &RenderOptions| {
            let mut hasher = DefaultHasher::new();
            opts.hash(&mut hasher);
            hasher.finish()
        };

        assert_ne!(
            hash_of(&RenderOptions::commonmark()),
            hash_of(&RenderOptions::untrusted())
        );
    }
}
