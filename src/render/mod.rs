//! HTML renderer entry points and render support modules.
//!
//! This module exposes the stable rendering API used by consumers:
//! [`render()`] and [`RenderOptions`].

use crate::error::MarcoError;

// The submodules are private: the public contract is this module's root plus
// the crate root. Every item below is re-exported, so `render::X` keeps
// resolving while `render::markdown::X` no longer does.

/// Base stylesheet used by preview rendering.
pub mod base_css;
/// Language normalization and metadata for code blocks.
pub(crate) mod code_languages;
/// Diagram rendering helpers (for example Mermaid).
#[cfg(feature = "render-diagrams")]
pub(crate) mod diagram;
/// Core Markdown AST to HTML renderer.
pub(crate) mod markdown;
/// Math rendering helpers.
#[cfg(feature = "render-math")]
pub(crate) mod math;
/// Public render configuration options.
pub(crate) mod options;
/// Platform mention rendering helpers.
pub(crate) mod plarform_mentions;
/// HTML document wrappers for preview pages.
pub(crate) mod preview_document;
/// Syntax highlighting support based on syntect.
#[cfg(feature = "render-syntax-highlighting")]
pub(crate) mod syntect_highlighter;
/// Theme metadata: parses `--theme-*` descriptive tokens from theme CSS.
pub(crate) mod theme_meta;

/// Re-export code language helpers.
pub use code_languages::*;
/// Re-export diagram rendering.
#[cfg(feature = "render-diagrams")]
pub use diagram::*;
/// Re-export core HTML render helpers.
pub use markdown::*;
/// Re-export render options.
pub use options::*;
/// Re-export platform mention helpers.
pub use plarform_mentions::*;
/// Re-export preview document helpers.
pub use preview_document::*;
/// Re-export syntax highlighter helpers.
#[cfg(feature = "render-syntax-highlighting")]
pub use syntect_highlighter::*;
/// Re-export theme metadata helpers.
pub use theme_meta::*;

use crate::parser::Document;

/// Render a parsed Markdown [`Document`] into HTML.
pub fn render(document: &Document, options: &RenderOptions) -> Result<String, MarcoError> {
    log::info!("Starting HTML render");
    let html = render_html(document, options)?;
    log::debug!("Generated {} bytes of HTML", html.len());
    Ok(html)
}
