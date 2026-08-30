# Testing

All tests use the public API re-exported from `src/lib.rs`.

## Running tests

```bash
# All tests (unit + integration + doc)
cargo test --locked

# A single integration suite
cargo test --test commonmark_spec_it --locked

# CommonMark strict mode (must pass 100%)
MARCO_SPEC_STRICT=1 cargo test --test commonmark_spec_it --locked

# Print failing spec examples
MARCO_SPEC_VERBOSE=1 cargo test --test commonmark_spec_it --locked
```

## Test layout

| Layer | Location | What it tests |
|---|---|---|
| Unit / smoke | `src/**/*.rs` (`#[cfg(test)]`) | Module internals (578 tests) |
| Integration | `tests/*.rs` | Public API surface (124 tests across 26 files, default features) |
| Spec conformance | `tests/commonmark_spec_it.rs` | CommonMark + extensions (652 examples + 30+ extension cases) |
| Extension specs | `tools/tests/extension_spec_it.rs` | GFM / Marco / Math / Diagram cases (5 suites) |
| Doc tests | `///` comments | Rustdoc examples (19 run, 12 `ignore`) |

**Total: 726 `cargo test` tests** (578 unit + 124 integration + 5 extension-spec
+ 19 doc), all passing — plus the 652 CommonMark spec examples and extension
fixture cases checked *within* those test functions (not separate `cargo test`
tests, see "CommonMark conformance" below).

The `parallel-parse` tests are 9 unit tests in
`src/parser/inlines/parallel_tests.rs` plus 4 in
`src/parser/blocks/parallel_inline.rs`, gated on the feature, which is on by
default. They are unit rather than integration tests deliberately: they call
`parse_inlines_from_span`, whose `GrammarSpan` argument is a `nom_locate` type
that must not appear in the public API.

The 12 `ignore` doc tests are examples on now-private block and inline parsers.
They demonstrate calls that were only ever possible while `parser::shared` was
public, since those functions take a `GrammarSpan`. They remain as illustrations
for crate developers; the modules' unit tests cover the behaviour.

### Three tests that guard a class of mistake

Most tests here check a behaviour. These three check that a *kind* of regression
cannot happen, and are worth understanding before changing:

| Test | Guards |
|---|---|
| `render_byte_identity_it.rs` | `RenderOptions::commonmark()` output, against a committed baseline of 848 fixtures. Regenerate with `MARCO_UPDATE_BASELINE=1` **only** for an intended render change, and read the diff — every changed line is a change in what consumers receive |
| `public_api_closure_it.rs` | a public type that is reachable but unnameable. Its `NodeKind` match is exhaustive with no `_` arm on purpose: adding a variant breaks the build until someone writes an arm, which is when they notice its payload types |
| `render_security_it.rs` | the six known XSS vectors, and exact output for each `HtmlPolicy`/`UrlPolicy` |

**Conformance and fixture suites must render with `RenderOptions::commonmark()`,
never `::default()`.** The default escapes raw HTML, and the CommonMark spec is
full of raw-HTML examples — measuring conformance under the safe policy measures
the policy, not the parser. It dropped the suite from 357 to 314 when the
default flipped.

**Assert on the absence of a live element, not of a payload substring.** Correct
escaped output legitimately contains the text `onerror`; showing the author what
they wrote is the point of `HtmlPolicy::Escape`. So `!html.contains("onerror")`
fails on *correct* output. Check for `<img`, `<a `, `href=` instead.

## Spec fixtures

Fixtures live in `tools/spec/`:

| File | Purpose |
|---|---|
| `commonmark.json` | 652 examples from the CommonMark spec (CC-BY-SA 4.0) |
| `gfm.json` | GitHub Flavored Markdown extension cases (MIT) |
| `marco.json` | Marco-specific extension cases (MIT) |
| `diagram.json` | Mermaid / diagram cases (MIT) |
| `math.json` | Math delimiter cases (MIT) |
| `combos.json` | Cross-feature combination + escape cases (MIT) |

See `tools/spec/README.md` for source attribution and fixture descriptions.

## CommonMark conformance

The `commonmark_spec_it` test runs two variants:

### Strict byte-for-byte equality (real conformance)

Each of 652 spec examples: `parse(md) → render() → assert_eq!(html)`.

**Current baseline: 357 / 652 ≈ 54.8%** (regression-guarded).

This is the real conformance number. The rest (295) are near-misses:
the parser produces the right structure but with formatting differences
(whitespace, entity escaping, attribute order, etc.).

### Loose structural match (for parity with legacy tooling)

Checks only if the same set of block-level tags (`<h1>`...`<h6>`, `<p>`,
`<pre>`, `<ul>`, `<ol>`, `<blockquote>`, `<hr>`, `<table>`) appear in both.

**Current baseline: 644 / 652 ≈ 98.8%** (confirms parser shape is correct).

## Regression guards

Tests assert **floors**, not exact values:

```rust
const MIN_COMMONMARK_PASS: usize = 350;  // current: 357
let min_structural_pct = 97.0;           // current: 98.8%
```

Why? Small intentional formatting tweaks (357 → 360) don't break the test.
Actual regressions (357 → 300) do fail. When conformance improves, raise
the constant — never lower it without a documented reason.

## Adding tests

### New grammar rule

- Add a unit test in the grammar module: `#[cfg(test)] mod tests { ... }`
- Add an integration test under `tests/` exercising parse → render

### Bug fix

- Add a regression test that fails before the fix and passes after

### New intelligence feature

- Add an integration test under `tests/`

### New render output

- Add an integration test asserting the exact HTML

## Test inventory

**Integration tests:** 124 total across 26 files under `tests/`:

| File | Count | Coverage |
|---|---:|---|
| `autolink_highlighting_it.rs` | 1 | Highlight offsets for multi-byte URLs |
| `commonmark_features_it.rs` | 9 | Entity refs, link references, Unicode casefold |
| `commonmark_spec_it.rs` | 3 | Strict spec run + structural run + extension suite |
| `definition_lists_it.rs` | 4 | Headerless definition lists |
| `gfm_admonitions_it.rs` | 5 | Alert blocks (`> [!NOTE]`) |
| `gfm_autolinks_it.rs` | 8 | Bare URLs and email autolinks |
| `gfm_footnotes_it.rs` | 5 | Footnote references and definitions |
| `gfm_tables_it.rs` | 4 | Pipe tables with alignment |
| `gfm_tasklist_it.rs` | 6 | Task list checkboxes |
| `heading_anchor_links_it.rs` | 2 | Custom heading IDs |
| `heading_ids_it.rs` | 2 | Auto-slug ID generation |
| `highlighting_it.rs` | 4 | Syntax highlight spans |
| `html_autolink_it.rs` | 8 | HTML block regression tests |
| `html_block_single_line_it.rs` | 1 | Single-line HTML blocks |
| `intelligence_provider_it.rs` | 6 | MarkdownIntelligenceProvider features |
| `marco_emoji_shortcode_it.rs` | 5 | Emoji shortcodes (`:smile:`) |
| `marco_headerless_table_it.rs` | 2 | Headerless tables |
| `marco_inline_footnotes_it.rs` | 6 | Inline footnotes |
| `marco_sliders_it.rs` | 3 | Slider blocks (`@slidestart` / `@slideend`) |
| `marco_tab_blocks_it.rs` | 3 | Tab blocks (`:::tab` / `@tab`) |
| `platform_mentions_it.rs` | 3 | Platform mentions (`@user[github]`) |
| `public_api_closure_it.rs` | 4 | Every public type is nameable from the crate root |
| `render_byte_identity_it.rs` | 3 | `commonmark()` output vs a committed 848-fixture baseline |
| `render_security_it.rs` | 18 | XSS vectors and exact output per `HtmlPolicy` / `UrlPolicy` |
| `error_it.rs` | 5 | `MarcoError` is `Send + Sync`; the depth limit errors rather than truncating |
| `sanitize_input_it.rs` | 4 | UTF-8 sanitization |

**Extension spec tests:** 5 suites in `tools/tests/extension_spec_it.rs`:
- `test_gfm_fixtures_match_expected_html`
- `test_marco_fixtures_match_expected_html`
- `test_diagram_fixtures_match_expected_html` (requires `render-diagrams` feature)
- `test_math_fixtures_match_expected_html` (requires `render-math` feature)
- `test_combos_fixtures_match_expected_html`

See [TOOLS.md](TOOLS.md) for how to run them.
