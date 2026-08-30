# Contributing to marco-core

Thanks for your interest in contributing. This document covers the basics; detailed
guidance is in the [Documentation](Documentation/) folder.

## Before you start

`marco-core` is a **standalone, pure-Rust library** published to crates.io.

- **Library only** — no binaries, no GUI dependencies (GTK, WebKit, etc.)
- **Self-contained** — no `marco`, `marco-shared`, or `polo` crates
- **API-stable** — anything in `src/lib.rs` follows semver

For editor or UI changes, open an issue against [Marco](https://github.com/Ranrar/Marco) instead.

## Quick workflow

1. **Open an issue first** for non-trivial changes (design discussion).
2. Fork and create a topic branch.
3. Make focused commits.
4. Add or update tests.
5. Run checks locally:
   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets --locked
   cargo test --locked
   ```
6. Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
7. Open a pull request.

## Setup & build

```bash
git clone https://github.com/Ranrar/marco-core
cd marco-core
cargo build
cargo test --locked
```

**Rust 1.94.1** (stable, pinned in CI).

## Detailed guides

- **[Documentation/DEVELOPMENT.md](Documentation/DEVELOPMENT.md)** — Modules, coding rules, public API contracts
- **[Documentation/TESTING.md](Documentation/TESTING.md)** — Test inventory, adding tests, spec fixtures
- **[Documentation/TOOLS.md](Documentation/TOOLS.md)** — Developer tools (marco-ast, perf-lab)

## Coding rules

- **No panics** — use `?`, `match`, or `.ok_or(...)` instead of `unwrap()`.
- **Errors are `MarcoError`**, never `Box<dyn std::error::Error>` — the latter is not `Send + Sync`,
  so it cannot cross an `.await` and `?` will not compile against `anyhow::Result`. A new failure
  mode is a new `#[non_exhaustive]` variant in `src/error.rs`, carrying owned data only: no `nom`,
  `katex-rs` or `mermaid-rs-renderer` types.
- **Borrow over clone** — public APIs use `&str` / `&Document`, not owned values.
- **No `unsafe`** unless documented and tested.
- **No logging to stdout** — use the `log` crate or `SimpleFileLogger`.
- **OS gating:** Use only `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "windows")]`.

## Testing

- Unit tests live next to the code: `#[cfg(test)] mod tests { ... }`
- Integration tests in `tests/` exercise the public API only — and a test that needs crate
  internals belongs *inside* the crate, not behind a widened API
- Conformance and fixture suites render with `RenderOptions::commonmark()`, never `::default()` —
  the default escapes raw HTML, so measuring conformance under it measures the policy, not the parser
- New grammar rule → needs both unit test **and** integration test
- Bug fix → needs a regression test that fails before, passes after

Run all tests:
```bash
cargo test --locked

# CommonMark strict mode
MARCO_SPEC_STRICT=1 cargo test --test commonmark_spec_it

# Print failing spec examples
MARCO_SPEC_VERBOSE=1 cargo test --test commonmark_spec_it
```

If your change improves spec conformance, bump `MIN_COMMONMARK_PASS` in
`tests/commonmark_spec_it.rs` to the new measured baseline.

## Public API & semver

The contract is `src/lib.rs`. Adding a new `pub use` is a minor bump;
removing or changing one is a major bump.

Discuss API additions in an issue first — the public surface is intentionally small.

## License

By contributing, you agree your contributions will be licensed under the [MIT License](LICENSE).

---

**Questions?** Open an issue or check the [Documentation](Documentation/) folder.

