//! `marco-core` is a pure-Rust Markdown engine with editor intelligence.
//!
//! Typical flow:
//! 1. Parse input text into an AST with [`parse`].
//! 2. Render the AST to HTML with [`render()`].
//! 3. Optionally run editor analysis via [`MarkdownIntelligenceProvider`].
//!
//! # Example
//! ```rust
//! let doc = marco_core::parse("# Hello")?;
//! let html = marco_core::render(&doc, &marco_core::RenderOptions::default())?;
//! assert!(html.contains("<h1"));
//! assert!(html.contains("Hello"));
//! # Ok::<(), marco_core::MarcoError>(())
//! ```

/// The crate version from `Cargo.toml`, exposed for downstream UIs and diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The crate's error type.
pub mod error;
/// Low-level Markdown grammar components (block and inline parsers).
pub mod grammar;
/// Editor intelligence APIs such as diagnostics, highlights, completions, and TOC.
pub mod intelligence;
/// Utility logic such as UTF-8 sanitization and text helpers.
pub mod logic;
/// AST definitions and parser entry points.
pub mod parser;
/// HTML rendering and rendering options.
pub mod render;

/// The error returned by [`parse`] and [`render`].
///
/// `Send + Sync`, so it converts into `anyhow::Error` with `?` and crosses an
/// `.await` boundary — neither of which was true of the `Box<dyn Error>` it
/// replaced in 1.x.
pub use error::MarcoError;
/// Main intelligence provider used by downstream editor integrations.
pub use intelligence::MarkdownIntelligenceProvider;
/// Diagnostic reporting types.
///
/// [`MarkdownIntelligenceProvider::diagnostics`] returns `Vec<Diagnostic>` and
/// [`MarkdownIntelligenceProvider::diagnostics_with_options`] takes a
/// [`DiagnosticsOptions`], so a consumer cannot use either without these.
#[cfg(feature = "intelligence-diagnostics")]
pub use intelligence::{
    Diagnostic, DiagnosticCode, DiagnosticSeverity, DiagnosticsOptions, DiagnosticsProfile,
};
/// Parse Markdown text into a [`Document`] AST using default options.
pub use parser::parse;
/// Parse Markdown with explicit runtime options (position tracking, math, diagrams).
pub use parser::parse_with_options;
/// Runtime parse configuration; pass to [`parse_with_options`] to skip expensive work.
pub use parser::ParseOptions;
/// The AST. [`parse`] produces a [`Document`]; [`render`] consumes one.
///
/// Exported as one set because it is one unit: [`Node`] carries a [`Span`],
/// [`Document`] carries a [`ReferenceMap`], and [`NodeKind`] variants carry
/// [`AdmonitionKind`], [`AdmonitionStyle`] and [`TableAlignment`]. Walking the
/// AST means naming all of them, so exporting only some of them would leave a
/// consumer able to *see* a field it cannot *name*.
///
/// Adding a variant that carries a new type means exporting that type here too.
/// `tests/public_api_closure_it.rs` fails to compile if that is forgotten.
pub use parser::{
    AdmonitionKind, AdmonitionStyle, Document, Node, NodeKind, Position, ReferenceMap, Span,
    TableAlignment,
};
/// Eagerly warm `parallel-render`'s thread pool and, for each given
/// language, its syntax highlighter — call at application startup with your
/// expected languages to move most of that one-time cost off the first
/// render. A no-op when `parallel-render` is not compiled in, so it's always
/// safe to call. See [`render::warm_render_thread_pool`] for details,
/// including which part of the cost this can and can't eliminate.
pub use render::warm_render_thread_pool;
/// Render a parsed [`Document`] into HTML using [`RenderOptions`].
pub use render::{render, RenderOptions};
/// Render-time security policies. [`HtmlPolicy`] governs author-written raw
/// HTML, [`UrlPolicy`] governs link and image URLs; both are carried on
/// [`RenderOptions`].
pub use render::{HtmlPolicy, UrlPolicy};

/// UTF-8 sanitization API and related types.
pub use logic::utf8::{sanitize_input, sanitize_input_with_stats, InputSource, SanitizeStats};
