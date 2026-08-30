//! `MarcoError` behaviour that consumers depend on.

use marco_core::{parse, MarcoError};

/// Exactly `anyhow::Error`'s blanket `From` bound.
///
/// Written as a bound rather than by depending on `anyhow`, because the bound
/// *is* the contract — if `MarcoError` satisfies it, `?` works in an
/// `anyhow::Result` function, and it can cross a `tokio::spawn` or `.await`
/// boundary. `Box<dyn Error>`, which this replaced, satisfied none of it.
fn accepted_where_anyhow_would<E: std::error::Error + Send + Sync + 'static>(_error: E) {}

/// Note the error comes from `parse` rather than being constructed: every
/// variant is `#[non_exhaustive]`, so a consumer cannot fabricate one. That is
/// deliberate — it keeps payload shapes evolvable — and it means consumers
/// match on errors rather than building them.
#[test]
fn error_meets_the_bound_that_makes_question_mark_work() {
    let deep = format!("{} text\n", ">".repeat(150));
    let error = parse(&deep).expect_err("should exceed the depth limit");
    accepted_where_anyhow_would(error);
}

/// B4: past the depth limit the parser used to return `Ok(Document::new())` —
/// an empty AST, indistinguishable from empty input. A server would render a
/// blank page and return 200.
#[test]
fn deeply_nested_input_errors_instead_of_returning_an_empty_document() {
    let deep = format!("{} text\n", ">".repeat(150));

    match parse(&deep) {
        Err(MarcoError::MaxDepthExceeded { depth, .. }) => {
            assert!(depth > 0, "the limit should be reported");
        }
        Err(other) => panic!("expected MaxDepthExceeded, got {other:?}"),
        Ok(document) => panic!(
            "expected an error; got Ok with {} children — this is the silent \
             data loss B4 describes",
            document.children.len()
        ),
    }
}

/// The failure must be distinguishable from a genuinely empty document, which
/// is the whole point of the change.
#[test]
fn empty_input_is_still_ok_and_not_confused_with_the_depth_limit() {
    let document = parse("").expect("empty input is valid Markdown");
    assert!(document.children.is_empty());
}

/// Nesting below the limit keeps working.
#[test]
fn ordinary_nesting_is_unaffected() {
    let nested = format!("{} text\n", ">".repeat(10));
    let document = parse(&nested).expect("ten levels is ordinary");
    assert!(!document.children.is_empty());
}

/// A consumer can map failures onto HTTP status codes, which `Box<dyn Error>`
/// made impossible — every failure was opaque and had to become a 500.
#[test]
fn failures_are_distinguishable_for_status_mapping() {
    fn status_for(error: &MarcoError) -> u16 {
        match error {
            // The document is at fault.
            MarcoError::MaxDepthExceeded { .. } => 400,
            // The server is at fault.
            MarcoError::Diagram { .. }
            | MarcoError::Math { .. }
            | MarcoError::Io { .. }
            | MarcoError::LoggerInit { .. } => 500,
            _ => 500,
        }
    }

    let deep = format!("{} text\n", ">".repeat(150));
    let error = parse(&deep).expect_err("should exceed the depth limit");
    assert_eq!(status_for(&error), 400);
}
