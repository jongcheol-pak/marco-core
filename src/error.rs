//! The crate's error type.
//!
//! # Why an enum, and why these variants
//!
//! CommonMark has no syntax errors: every string is a valid document. Unclosed
//! emphasis becomes literal text, a broken link reference degrades to brackets,
//! malformed HTML is just text. So there is no input for which the right answer
//! is "parse failed at line 5, column 12", and none of these variants carries a
//! source position — they are all *resource* failures, and they carry
//! primitives.
//!
//! That is also why the set is small and why matching on it is useful: a web
//! backend can map a malformed document to 400 and a broken subsystem to 500,
//! which `Box<dyn Error>` made impossible.
//!
//! # Evolution
//!
//! [`MarcoError`] and every variant that carries fields are `#[non_exhaustive]`.
//! The enum attribute keeps *adding a variant* a minor change; the variant
//! attribute keeps *adding a field* a minor change. Both are needed — the first
//! alone would freeze every payload shape on the day it shipped.
//!
//! No variant carries a type from `nom`, `katex-rs` or `mermaid-rs-renderer`.
//! The parser types would make swapping the parser library a breaking change,
//! and the other two live behind feature flags, which would make the public API
//! itself feature-dependent.

use std::fmt;

/// An error produced by parsing or rendering.
///
/// Implements [`std::error::Error`] and is `Send + Sync`, so it converts into
/// `anyhow::Error` with `?` and can cross a `tokio::spawn` or `.await`
/// boundary — neither of which was true of the `Box<dyn Error>` this replaced.
#[non_exhaustive]
#[derive(Debug)]
pub enum MarcoError {
    /// Block nesting exceeded the parser's depth limit.
    ///
    /// The document is rejected rather than truncated. Before this existed the
    /// parser returned an empty document and `Ok`, so a caller could not tell
    /// "the input was empty" from "we gave up" — a server would serve a blank
    /// page with a 200.
    ///
    /// Deep nesting at this scale is either generated or hostile; a document
    /// written by a person does not reach it.
    #[non_exhaustive]
    MaxDepthExceeded {
        /// The limit that was exceeded.
        depth: usize,
    },

    /// A diagram could not be rendered.
    ///
    /// Carries the renderer's message as an owned `String` rather than the
    /// upstream error type, which lives behind the `render-diagrams` feature.
    #[non_exhaustive]
    Diagram {
        /// What went wrong, for logging and display.
        message: String,
    },

    /// A math expression could not be rendered.
    ///
    /// Carries an owned message for the same reason as [`Self::Diagram`].
    #[non_exhaustive]
    Math {
        /// What went wrong, for logging and display.
        message: String,
    },

    /// An I/O operation failed.
    ///
    /// Raised by the file logger; parsing and rendering do no I/O.
    #[non_exhaustive]
    Io {
        /// The underlying failure, available through
        /// [`std::error::Error::source`].
        source: std::io::Error,
    },

    /// The global logger could not be installed.
    ///
    /// Distinct from [`Self::Io`]: the log directory was writable, but
    /// something else had already claimed the process-wide logger. A consumer
    /// that does not care can ignore this without ignoring real I/O failures.
    #[non_exhaustive]
    LoggerInit {
        /// What went wrong, for logging and display.
        message: String,
    },
}

impl MarcoError {
    /// A diagram failure carrying `message`.
    pub(crate) fn diagram(message: impl Into<String>) -> Self {
        Self::Diagram {
            message: message.into(),
        }
    }

    /// A math failure carrying `message`.
    #[cfg(feature = "render-math")]
    pub(crate) fn math(message: impl Into<String>) -> Self {
        Self::Math {
            message: message.into(),
        }
    }

    /// A logger-installation failure carrying `message`.
    #[cfg(feature = "file-logger")]
    pub(crate) fn logger_init(message: impl Into<String>) -> Self {
        Self::LoggerInit {
            message: message.into(),
        }
    }
}

impl fmt::Display for MarcoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxDepthExceeded { depth } => {
                write!(f, "block nesting exceeded the limit of {depth} levels")
            }
            Self::Diagram { message } => write!(f, "diagram render failed: {message}"),
            Self::Math { message } => write!(f, "math render failed: {message}"),
            Self::Io { source } => write!(f, "I/O error: {source}"),
            Self::LoggerInit { message } => write!(f, "logger initialisation failed: {message}"),
        }
    }
}

impl std::error::Error for MarcoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for MarcoError {
    fn from(source: std::io::Error) -> Self {
        Self::Io { source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property that motivated the whole change: `Box<dyn Error>` is
    /// neither, so it could not cross an `.await` or reach `anyhow`.
    #[test]
    fn is_send_and_sync() {
        fn require<T: Send + Sync + 'static>() {}
        require::<MarcoError>();
    }

    #[test]
    fn is_a_std_error_with_a_source_chain() {
        fn require<T: std::error::Error>() {}
        require::<MarcoError>();

        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = MarcoError::from(io);
        assert!(std::error::Error::source(&err).is_some());

        assert!(std::error::Error::source(&MarcoError::MaxDepthExceeded { depth: 100 }).is_none());
    }

    #[test]
    fn display_says_what_happened() {
        assert_eq!(
            MarcoError::MaxDepthExceeded { depth: 100 }.to_string(),
            "block nesting exceeded the limit of 100 levels"
        );
        assert_eq!(
            MarcoError::diagram("empty").to_string(),
            "diagram render failed: empty"
        );
    }

    /// A consumer can branch on the failure, which is the point of an enum over
    /// `Box<dyn Error>` — a malformed document is a 400, a broken subsystem a 500.
    #[test]
    fn variants_are_distinguishable_by_a_consumer() {
        let status = |e: &MarcoError| match e {
            MarcoError::MaxDepthExceeded { .. } => 400,
            MarcoError::Diagram { .. } | MarcoError::Math { .. } => 500,
            MarcoError::Io { .. } => 500,
            _ => 500,
        };

        assert_eq!(status(&MarcoError::MaxDepthExceeded { depth: 100 }), 400);
        assert_eq!(status(&MarcoError::diagram("x")), 500);
    }
}
