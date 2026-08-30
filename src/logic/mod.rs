/// File-backed logging utilities.
pub mod logger;
pub mod text_completion;
/// URL scheme classification shared by the renderer and diagnostics.
pub mod url_scheme;
pub mod utf8;

// Re-export commonly used types
pub use utf8::{sanitize_input, sanitize_input_with_stats, InputSource, SanitizeStats};
