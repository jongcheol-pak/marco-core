# tests

Integration tests for `marco-core`. Each `.rs` file is a separate `cargo test`
binary, which is the point: they compile as their own crates, so they see
exactly the public API a crates.io consumer sees — the crate root plus the
`parser`, `render`, `intelligence` and `logic` module roots.

**A test that needs crate internals does not belong here.** Put it in the module
it tests, as a unit test. Widening the API to let an integration test reach
something is the wrong trade: `src/parser/inlines/parallel_tests.rs` lives inside
the crate because it needs `GrammarSpan`, and re-exporting a `nom` type would
have made swapping the parser library a breaking change.

See [`Documentation/TESTING.md`](../Documentation/TESTING.md) for:

- The full test inventory (124 integration tests across 26 files)
- The three tests that guard a class of mistake rather than a behaviour
- CommonMark conformance numbers and how to interpret them
- Regression-guard thresholds and strict/verbose modes
- Conventions for adding new tests

## Running

```bash
# All tests (unit + integration + doc tests)
cargo test --locked

# A single integration binary
cargo test --test commonmark_spec_it --locked

# Strict CommonMark conformance mode
MARCO_SPEC_STRICT=1 cargo test --test commonmark_spec_it --locked

# Print failing spec examples
MARCO_SPEC_VERBOSE=1 cargo test --test commonmark_spec_it --locked
```

## Fixtures

Spec fixture JSON files live in [`tools/spec/`](../tools/spec/README.md) and
are loaded by `commonmark_spec_it.rs` via `include_str!`.
