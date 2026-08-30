//! marco-core adapter.
//!
//! Renders with `RenderOptions::commonmark()` rather than `::default()`. Two
//! reasons: the default applies the security policies, so measuring it would
//! fold escaping and URL classification into the render number and silently
//! break comparability with every baseline recorded before that default
//! flipped; and the crates compared against here — `pulldown-cmark` in
//! particular — do no such filtering, so the permissive path is the one that
//! matches them. Policy cost is worth measuring, but as its own mode.

use super::{EngineAdapter, EngineRun};
use crate::cli::Mode;
use std::time::Instant;

#[derive(Default)]
pub struct MarcoCoreAdapter;

impl EngineAdapter for MarcoCoreAdapter {
    fn id(&self) -> &'static str {
        "marco-core"
    }

    fn run_mode(&self, mode: Mode, input: &str) -> Result<EngineRun, String> {
        match mode {
            Mode::Parse => {
                let started = Instant::now();
                let _doc = marco_core::parse(input).map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            Mode::Render => {
                let doc = marco_core::parse(input).map_err(|e| e.to_string())?;
                let started = Instant::now();
                let _html = marco_core::render(&doc, &marco_core::RenderOptions::commonmark())
                    .map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            Mode::E2e => {
                let started = Instant::now();
                let doc = marco_core::parse(input).map_err(|e| e.to_string())?;
                let _html = marco_core::render(&doc, &marco_core::RenderOptions::commonmark())
                    .map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            Mode::Intelligence => {
                let doc = marco_core::parse(input).map_err(|e| e.to_string())?;
                let started = Instant::now();
                let mut provider = marco_core::MarkdownIntelligenceProvider::new();
                provider.update_document(doc);
                let diagnostics = provider.diagnostics();
                let highlights = provider.highlights(input);
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: diagnostics.len(),
                    highlights_count: highlights.len(),
                })
            }
        }
    }
}

/// Raw adapter: uses `parse_with_options` with all runtime cost-savers enabled
/// (no position tracking, no math parsing, no diagram parsing).
/// Used to measure the minimum parser hot-path cost.
#[derive(Default)]
pub struct MarcoCoreRawAdapter;

impl EngineAdapter for MarcoCoreRawAdapter {
    fn id(&self) -> &'static str {
        "marco-core-raw"
    }

    fn run_mode(&self, mode: Mode, input: &str) -> Result<EngineRun, String> {
        let raw_opts = marco_core::ParseOptions {
            track_positions: false,
            parse_math: false,
            parse_diagrams: false,
        };

        match mode {
            Mode::Parse => {
                let started = Instant::now();
                let _doc = marco_core::parse_with_options(input, raw_opts)
                    .map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            Mode::Render => {
                let doc = marco_core::parse_with_options(input, raw_opts)
                    .map_err(|e| e.to_string())?;
                let started = Instant::now();
                let _html = marco_core::render(&doc, &marco_core::RenderOptions::commonmark())
                    .map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            Mode::E2e => {
                let started = Instant::now();
                let doc = marco_core::parse_with_options(input, raw_opts)
                    .map_err(|e| e.to_string())?;
                let _html = marco_core::render(&doc, &marco_core::RenderOptions::commonmark())
                    .map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            // Intelligence not meaningful without position tracking
            Mode::Intelligence => Err(
                "marco-core-raw does not support intelligence mode (track_positions=false)"
                    .to_string(),
            ),
        }
    }
}

/// Adapter parameterised by render policy, so the cost of the security
/// policies is measurable rather than assumed.
///
/// The three presets differ only in what the renderer does with raw HTML and
/// with URLs, so running the same workload through each isolates that cost:
///
/// | Engine id | Policy | Measures |
/// |---|---|---|
/// | `marco-core` | `commonmark()` | the permissive path — the comparability anchor, and what the other engines here do |
/// | `marco-core-safe` | `new()` — the 2.0 default | what a consumer gets without asking |
/// | `marco-core-untrusted` | `untrusted()` | the strictest path, for a server rendering one user's Markdown to another |
///
/// Parsing is policy-independent, so `Mode::Parse` is rejected rather than
/// silently reporting a number identical to `marco-core`'s — a benchmark that
/// answers a question it did not measure is worse than one that declines.
pub struct MarcoCorePolicyAdapter {
    id: &'static str,
    options: fn() -> marco_core::RenderOptions,
}

impl MarcoCorePolicyAdapter {
    /// `RenderOptions::new()` — the 2.0 default: escape raw HTML, block
    /// dangerous URL schemes.
    pub fn safe() -> Self {
        Self {
            id: "marco-core-safe",
            options: marco_core::RenderOptions::new,
        }
    }

    /// `RenderOptions::untrusted()` — escape raw HTML, allow only known-safe
    /// URL schemes.
    pub fn untrusted() -> Self {
        Self {
            id: "marco-core-untrusted",
            options: marco_core::RenderOptions::untrusted,
        }
    }
}

impl EngineAdapter for MarcoCorePolicyAdapter {
    fn id(&self) -> &'static str {
        self.id
    }

    fn run_mode(&self, mode: Mode, input: &str) -> Result<EngineRun, String> {
        let options = (self.options)();

        match mode {
            Mode::Render => {
                let doc = marco_core::parse(input).map_err(|e| e.to_string())?;
                let started = Instant::now();
                let _html = marco_core::render(&doc, &options).map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            Mode::E2e => {
                let started = Instant::now();
                let doc = marco_core::parse(input).map_err(|e| e.to_string())?;
                let _html = marco_core::render(&doc, &options).map_err(|e| e.to_string())?;
                Ok(EngineRun {
                    elapsed_ns: started.elapsed().as_nanos(),
                    diagnostics_count: 0,
                    highlights_count: 0,
                })
            }
            Mode::Parse => Err(format!(
                "{} does not support parse mode: render policy does not affect parsing, \
                 so the number would be marco-core's under another name",
                self.id
            )),
            Mode::Intelligence => Err(format!(
                "{} does not support intelligence mode: render policy does not affect analysis",
                self.id
            )),
        }
    }
}
