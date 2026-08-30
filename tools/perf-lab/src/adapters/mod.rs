pub mod comrak;
pub mod marco_core;
pub mod markdown_it;
pub mod markdown_rs;
pub mod pulldown_cmark;

use crate::cli::Mode;

#[derive(Debug, Clone)]
pub struct EngineDescriptor {
    pub id: &'static str,
    pub implemented: bool,
    pub notes: &'static str,
}

#[derive(Debug, Clone)]
pub struct EngineRun {
    pub elapsed_ns: u128,
    pub diagnostics_count: usize,
    pub highlights_count: usize,
}

pub trait EngineAdapter {
    fn id(&self) -> &'static str;
    fn run_mode(&self, mode: Mode, input: &str) -> Result<EngineRun, String>;
}

pub fn descriptors() -> Vec<EngineDescriptor> {
    vec![
        EngineDescriptor {
            id: "marco-core",
            implemented: true,
            notes: "native in-process adapter (full features)",
        },
        EngineDescriptor {
            id: "marco-core-raw",
            implemented: true,
            notes: "marco-core with track_positions=false, parse_math=false, parse_diagrams=false",
        },
        EngineDescriptor {
            id: "marco-core-safe",
            implemented: true,
            notes: "marco-core with the 2.0 default policy (escape raw HTML, block dangerous URL \
                    schemes); render/e2e only",
        },
        EngineDescriptor {
            id: "marco-core-untrusted",
            implemented: true,
            notes: "marco-core with RenderOptions::untrusted() (escape raw HTML, safe URL schemes \
                    only); render/e2e only",
        },
        EngineDescriptor {
            id: "pulldown-cmark",
            implemented: true,
            notes: "parse/render/e2e modes; intelligence unsupported",
        },
        EngineDescriptor {
            id: "comrak",
            implemented: true,
            notes: "parse/render/e2e modes; intelligence unsupported",
        },
        EngineDescriptor {
            id: "markdown-rs",
            implemented: false,
            notes: "phase 5 adapter placeholder",
        },
        EngineDescriptor {
            id: "markdown-it-rs",
            implemented: false,
            notes: "phase 5 adapter placeholder",
        },
    ]
}

pub fn get_adapter(engine: &str) -> Result<Box<dyn EngineAdapter>, String> {
    match engine {
        "marco-core" => Ok(Box::new(marco_core::MarcoCoreAdapter::default())),
        "marco-core-raw" => Ok(Box::new(marco_core::MarcoCoreRawAdapter::default())),
        "marco-core-safe" => Ok(Box::new(marco_core::MarcoCorePolicyAdapter::safe())),
        "marco-core-untrusted" => Ok(Box::new(marco_core::MarcoCorePolicyAdapter::untrusted())),
        "pulldown-cmark" => Ok(Box::new(pulldown_cmark::PulldownCmarkAdapter)),
        "comrak" => Ok(Box::new(comrak::ComrakAdapter)),
        "markdown-rs" => Ok(Box::new(markdown_rs::MarkdownRsAdapter)),
        "markdown-it-rs" => Ok(Box::new(markdown_it::MarkdownItRsAdapter)),
        _ => Err(format!("unknown engine: {engine}")),
    }
}
