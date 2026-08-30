# Development Guide

## Setup

```bash
git clone https://github.com/Ranrar/marco-core
cd marco-core
cargo build
cargo test --locked
```

**Rust 1.94.1** (stable, pinned in CI).

**Linux only:** `libfontconfig-dev` is required for the math/diagram renderers.

## Project structure

`marco-core` is a **library crate only** — no binaries, no GUI dependencies.

| Module | Responsibility |
|---|---|
| `src/grammar/` | nom combinators → spans / tokens |
| `src/parser/` | Grammar output → AST builders |
| `src/render/` | AST → HTML emitter |
| `src/intelligence/` | Highlights, diagnostics, completions, hover, TOC |
| `src/logic/` | UTF-8 sanitization, text completion, logging |
| `tools/` | Developer tools (not published): marco-ast CLI, perf-lab benchmarking |

Naming convention for grammar/parser modules:
- `cm_*` — CommonMark spec features
- `gfm_*` — GitHub Flavored Markdown extensions
- `marco_*` — Marco-specific extensions

## Development workflow

1. **Open an issue first** for non-trivial changes.
2. Fork and create a topic branch.
3. Make focused commits.
4. Add or update tests (see [TESTING.md](TESTING.md)).
5. Run checks locally:
   ```bash
   cargo fmt --all --check
   cargo clippy --all-targets --locked
   cargo test --locked
   ```
6. Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
7. Open a pull request.

## Coding rules

**No panics** in library code — use `?`, `match`, or `.ok_or(...)`.

**Borrow over clone** — public APIs prefer `&str` / `&Document`.

**No `unsafe`** unless documented and tested.

**No logging to stdout** — use the `log` crate via the `SimpleFileLogger` module.

**OS gating:** Use only `#[cfg(target_os = "linux")]` / `#[cfg(target_os = "windows")]`.
Do not use `cfg(any(...))` or `cfg(not(...))` for OS gates.

## Naming conventions

All runtime-visible identifiers must follow these rules for consistency.

### Rust symbols

- Marco extension modules and types **must** carry the `marco_` / `Marco` prefix matching their source file.
  - `marco_sliders.rs` → `MarcoSlideDeck`, `MarcoSlide`, `marco_slide_deck()`
  - `marco_tab_blocks.rs` → `MarcoTabBlock`, `MarcoTabItem`, `marco_tab_block()`
  - `marco_headerless_table.rs` → `MarcoHeaderlessTableBlock`, `marco_headerless_table()`
- File-naming prefixes: `cm_*` (CommonMark), `gfm_*` (GFM), `marco_*` (Marco extensions).
- Top-level public API from `src/lib.rs` uses neutral names (`parse`, `render`, `RenderOptions`, …).
- **The public API is `src/lib.rs` plus the `parser`, `render`, `intelligence` and `logic` module
  roots.** Their submodules are `pub(crate)`; consumers reach everything through a root re-export.
  Two rules follow:
  - **Keep the re-export set closed.** A `pub` type in a private module compiles fine while being
    unnameable — a consumer can read `node.span` but cannot write `fn f(s: Span)`. Every type
    reachable from a public type must itself be re-exported. `tests/public_api_closure_it.rs` fails
    to compile if you forget.
  - **Never expose a foreign type.** `GrammarSpan` is `nom_locate::LocatedSpan`; exposing it would
    make swapping the parser library a breaking change. `katex-rs` and `mermaid-rs-renderer` types
    would additionally make the public API feature-dependent.

### CSS classes

All CSS classes **must** follow `marco-<feature>` BEM blocks:

| Feature | Block | Example elements / modifiers |
|---|---|---|
| Sliders | `marco-sliders` | `__viewport`, `__slide`, `__controls`, `__btn`, `__btn--prev`, `__btn--next`, `__icon`, `__icon--play` |
| Tab blocks | `marco-tabs` | `__radio`, `__tablist`, `__tab`, `__panels` |
| Task list | `marco-task-list-item`, `marco-task-checkbox` | `--checked` |
| Code blocks | `marco-code-block`, `marco-copy-btn` | — |
| Headings | `marco-heading-anchor` | — |
| Platform mentions | `marco-mention`, `marco-mention-<platform>` | — |
| Tables | `marco-table-auto-align`, `marco-table-resizing` | — |

Exception: `nested-code-block` and its sub-classes are intentionally unprefixed.

### CSS custom properties

All custom properties **must** use `--mc-<feature>-<token>`:

| Property | Purpose |
|---|---|
| `--mc-task-primary` | Task checkbox primary colour |
| `--mc-task-accent` | Task checkbox accent colour |
| `--mc-sliders-border` | Slider deck border |
| `--mc-sliders-bg` | Slider deck background |

### HTML element IDs

All IDs **must** use `mc-<feature>-<part>`:

| ID | Purpose |
|---|---|
| `mc-content-container` | Main rendered-content `<div>` |
| `mc-preview-style` | External preview stylesheet |
| `mc-preview-internal-style` | Internal preview stylesheet |
| `mc-paged-page-css` | Paged-view stylesheet |

### JavaScript bridge

The single global **must** be `window.MarcoCorePreview`. Methods use camelCase:
`MarcoCorePreview.updateContent()`, `MarcoCorePreview.cleanup()`.

### Checklist for new Marco extension

When adding `marco_foo.rs`:

1. Grammar: `MarcoFoo` / `marco_foo()`
2. CSS block: `marco-foo`, `marco-foo__part`, `marco-foo__part--state`
3. CSS vars: `--mc-foo-token` (if needed)
4. HTML IDs: `mc-foo-part` (if needed)
5. Add selectors to `src/render/base_css.rs`
6. Emit classes in `src/render/markdown.rs`
7. Add integration test asserting canonical names
8. Document in `CHANGELOG.md` under `[Unreleased] → Added`

### SemVer implications

- Adding new `marco-*` classes or `--mc-*` vars → **minor** bump
- Renaming or removing public class/var/ID/JS method → **major** bump
- Internal Rust symbol renames not re-exported from `lib.rs` → **patch** bump

Use `#[deprecated(since = "X.Y.Z")]` when renaming public Rust symbols.

## Public API & semver

The contract is `src/lib.rs`. Anything re-exported from there follows semantic versioning:
- Adding a new `pub use` → minor bump
- Removing / changing one → major bump

Discuss API additions in an issue first.

## Build commands

```bash
cargo fmt --all                    # Format
cargo clippy --all-targets --locked # Lint
cargo test --locked                 # Run all tests
cargo doc --open                    # Build & view API docs
cargo build --release               # Release build
```

## Releasing (maintainers only)

1. Update `CHANGELOG.md` and `Cargo.toml` version.
2. Run `cargo publish --dry-run --locked`.
3. Commit, tag `vX.Y.Z`, and push the tag.
4. The `.github/workflows/publish-crate.yml` workflow handles crates.io publication.

See [CHANGELOG.md](../CHANGELOG.md) for the format and versioning policy.
