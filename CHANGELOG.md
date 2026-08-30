# Changelog
All notable user-visible changes to **marco-core** are documented here.

This project follows **Semantic Versioning** and uses the **Keep a Changelog** format.

Version scheme note: `marco-core` and `marco-shared` follow independent semver from the application binaries (marco/polo). The library tracks API stability for crates.io consumers; breaking API changes increment the major version.

## [Unreleased]

Work towards **server-rendering**: making `marco-core` usable from a Rust web
backend that renders Markdown written by one user and displays it to another.
Until now the crate targeted a desktop editor, where the document author and the
document reader are the same person and raw HTML passing through is correct.
They are not the same person on a server.

This is a **breaking** release. Each change below carries its migration.

### Added

#### `HtmlPolicy` and `UrlPolicy` — render-time security policies

`RenderOptions` now carries two policies. `HtmlPolicy` governs author-written
raw HTML (`Allow` / `Escape` / `Strip`); `UrlPolicy` governs link and image
URLs (`Allow` / `BlockDangerous` / `SafeSchemesOnly`). Both are
`#[non_exhaustive]`, so future policies are a minor bump.

Three constructors name the useful combinations, so a consumer never has to
reason about which pairing constitutes a security boundary:

| Constructor | `html` | `urls` | For |
|---|---|---|---|
| `RenderOptions::commonmark()` | `Allow` | `Allow` | trusted single-author input; spec conformance |
| `RenderOptions::new()` / `default()` | `Escape` | `BlockDangerous` | a desktop editor |
| `RenderOptions::untrusted()` | `Escape` | `SafeSchemesOnly` | a server rendering one user's Markdown to other readers |

`RenderOptions::untrusted()` may gain further protections in future minor
versions without any consumer changing a line — which is the point of having it
rather than a documented combination of flags.

#### `logic::url_scheme` — URL scheme classification

The normalisation and classification shared by the renderer and the diagnostics
layer. One `scheme_of` normaliser — which strips ASCII whitespace and control
characters, resolves `%XX` escapes and HTML character references, lowercases,
and locates the true scheme separator — and two predicates over it.

The ordering matters more than either policy: every published bypass of a
Markdown URL filter has been a normalisation failure rather than a policy
failure. `goldmark` fell to `javascript&colon;alert()` because it read the
scheme before entities were resolved.

### Changed

#### `RenderOptions` is `#[non_exhaustive]`

Struct-literal construction from outside the crate no longer compiles, including
the `..RenderOptions::default()` form. Use `RenderOptions::new()`,
`::commonmark()` or `::untrusted()` with the `with_*` builders:

```rust
// before
let opts = RenderOptions { syntax_highlighting: true, ..RenderOptions::default() };

// after
let opts = RenderOptions::commonmark().with_syntax_highlighting(true);
```

Configuration serialized by 1.x still loads: both new fields are
`#[serde(default)]`, and absent data deserializes to the safe policies.

#### `render_mermaid_diagram` takes an `allow_interaction` argument

`render_mermaid_diagram(source, theme)` becomes
`render_mermaid_diagram(source, theme, allow_interaction)`. Pass `true` for 1.x
behaviour.

### Security

#### Mermaid `click` directives could inject a `javascript:` anchor

A `click` or `link` directive made `mermaid-rs-renderer` emit
`<a href="javascript:alert(1)" xlink:href="…">` into the SVG, which this crate
spliced into the rendered page verbatim. Any author who could write a diagram
could run script in the browser of anyone who read it. `mermaid-rs-renderer`
0.3 exposes no option to disable this; its `RenderOptions` carries only a theme
and a layout.

With `allow_interaction: false`, interaction directives are removed from the
diagram source before rendering, and the resulting SVG is checked for stray
anchors — a diagram that still carries one is rejected so the caller can fall
back to a code block.

The directives are stripped from the *source* rather than filtered out of the
rendered SVG deliberately: rewriting the output would mean re-parsing SVG whose
shape this crate does not control, and filtering the URL in place would mean
duplicating upstream's private tokenizer — a parser differential, which is the
very thing that produces bypasses. Matching the directive *line* keys on the
same two prefixes upstream keys on, so the two cannot disagree.

The cost, accepted deliberately: a legitimate `click A "https://example.com"` is
dropped too. Diagram links are rare, and the alternative is a URL filter that
can drift out of step with the parser it guards.

## [1.3.3] - 2026-08-21

### Added

#### Diagnostic code for images embedded as base64 data URIs

`MD405` (`EmbeddedBase64Image`) describes an image that carries its bytes inline
as a base64 `data:` URI rather than pointing at a file. Like `MD206` and `MD404`
it is **never raised by this crate**: an embedded image is not a defect, so the
analysis stays silent about it and no consumer is handed a finding it did not
ask for. The code and its catalog metadata — title, message template (parameter
`{mediatype}`), `Hint` severity, fix suggestion — live here so an application
that wants to describe an embedded image, in a hover popover for instance, draws
on the same text and numbering as every other diagnostic.

### Fixed

#### `MD403` reported every embedded base64 image as an unsafe protocol

`data` is on the `unsafe_protocols` list, and the image check compared the URL
scheme exactly, so any `![logo](data:image/png;base64,…)` raised
`MD403 Unsafe image protocol` at `Error` severity — advising the author to
switch to `https:`, which is not something an embedded image can do. A document
of twenty embedded images got twenty red errors.

The check now exempts a data URI that carries an inert image: one whose
mediatype is listed in the new `inert_image_data_mediatypes` catalog setting
(`image/png`, `image/jpeg`, `image/gif`, `image/webp`) **and** whose payload is
`;base64,`. Everything else still reports, including:

- `data:image/svg+xml`, base64 or not — an image mediatype, but script-capable
  in the WebViews this crate's HTML is rendered in, so deliberately not on the
  list;
- a non-base64 payload such as `data:image/png,…`;
- non-image mediatypes such as `data:text/html;base64,…`;
- `javascript:` image sources, unchanged.

The exemption applies to **images only**. The identical check on a link
(`MD203` / `UnsafeLinkProtocol`) is unchanged and stays blanket: a `data:` URL
a reader can click is a different question from one that renders as a picture.

## [1.3.2] - 2026-08-17

### Added

#### Diagnostic codes for missing link and image targets

`MD206` (`MissingLinkTarget`) and `MD404` (`MissingImageTarget`) report a link
or image whose local path has no file at it. Neither is ever raised by this
crate: whether a path exists is a filesystem question, and the analysis is never
told where the document lives. The codes and their catalog metadata — title,
message template, severity, fix suggestion — live here so an application that
*can* answer it files the result alongside every other diagnostic instead of
inventing a numbering of its own.

## [1.3.1] - 2026-08-16

### Fixed

#### Blockquotes and lists corrupted block structure on non-ASCII input

Both parsers computed line lengths as **byte** offsets but consumed them with
nom's `bytes::complete::take()` combinator, which counts **characters** when the
input is a `&str`. For ASCII the two units coincide; for anything else the
parser over-consumed by one character per extra UTF-8 byte, and the damage
compounded across the document.

In blockquotes this meant a quote swallowed whatever followed it — a thematic
break after `> Nóte.` was partially eaten, and with nothing to absorb the drift
the blockquote failed to match at all and its `>` marker leaked into the output
as literal text. In lists a multi-byte continuation line consumed past its own
newline, turning sibling list items into nested ones.

Every language outside ASCII was affected, including Czech, Japanese, German,
French, Spanish, Greek, Cyrillic and emoji. Both parsers now slice by byte
length via `Input::take`/`take_from`. The CommonMark spec corpus never caught
this because every example in it is ASCII, so regression tests covering
non-ASCII input were added alongside the fix.

Reported, diagnosed and patched by @vobornik (#4); the list occurrence was found
while auditing the other `take()` call sites they flagged.

#### Headings did not parse their content as inline elements

ATX and setext headings carried their content as a plain string and never ran
the inline grammar over it, so emphasis, strong, code spans, strikethrough,
highlights, links and images all rendered as literal markup — `# foo *bar*`
produced `<h1>foo *bar*</h1>`. Heading content is now parsed as inline content
per CommonMark §4.2 / §4.3.

Auto-generated heading ids are unchanged: they are still derived from the raw
heading text, so existing `#fragment` links keep working.

One rendering change is visible. Marco wraps heading text in a
`marco-heading-anchor` self-link so the whole heading is clickable, and HTML
forbids nesting one `<a>` inside another. A heading whose content renders its
own anchor — a link, a resolved footnote reference, or raw inline HTML
containing one — therefore omits the wrapper, leaving the inner link clickable
instead. Headings containing an image keep the wrapper, since `<img>` inside
`<a>` is valid.

Reported by @vobornik (#5).

### Changed

#### Inline elements inside headings are now styled correctly

Headings paint their text with a gradient clipped to the glyph shape, which
makes the text transparent and is inherited by every descendant. The rules that
compensated for this were written when headings could not contain inline
elements, and became wrong as soon as they could:

- `code` and `mark` were reset with the `background` shorthand, erasing the
  code box and the highlight that make them recognisable. They now keep their
  own backgrounds, and `mark` keeps `--mark-color` rather than being forced to
  `--text-color`, which is chosen to contrast with `--mark-bg`.
- Links in heading text inherited the transparent fill and were painted with
  the heading gradient instead of `--link-color`, so they did not read as
  links.
- `strong` was indistinguishable from surrounding heading text, because
  headings are already `font-weight: 600` and the sans-serif families that
  Marco's themes resolve to on most Linux systems ship only Regular and Bold —
  so 600 and 700 render as the same face. Heading `strong` now also thickens
  the glyph outline, which is independent of the weights a font provides.
  Themes can set `--heading-strong-stroke: 0` to opt out, or tune
  `--heading-strong-weight`.

## [1.3.0] - 2026-07-17

### Fixed

#### Catastrophic slowdown on pathological/adversarial Markdown

Inline parsing previously fell off an algorithmic cliff on certain input
shapes — most severely on documents with many unmatched `[`/`]` brackets
(e.g. `[[[[[unbalanced]`), which triggered exponential-in-nesting-depth
speculative re-parsing and could take **hundreds of milliseconds** for a
13 KB file. Deeply nested emphasis delimiter runs (`*a**b***c...`) hit a
related, less severe quadratic blowup. On the full CommonMark spec suite
this showed up as marco-core running **~211x slower than pulldown-cmark**
and **~209x slower than comrak**; on the isolated bracket-heavy case, up to
**~1061x/574x** slower.

Two root causes, both fixed:
- Emphasis/strong resolution now implements the CommonMark delimiter-stack
  algorithm (spec Appendix A) instead of a naive per-position rescan. As a
  side effect, strict CommonMark spec conformance rose from 285/652 (43.7%)
  to 357/652 (54.8%), with zero regressions elsewhere (structural
  conformance unchanged at 644/652; both extension spec suites still 100%).
- Link/image bracket matching is now precomputed in a single linear pass
  instead of rescanned per attempt, and reference-link parsing no longer
  eagerly re-parses speculative link text before validating the surrounding
  construct is even a valid link.

Net effect: the full spec suite now runs within **~2.4x/2.3x** of
pulldown-cmark/comrak (down from ~211x/209x), and the bracket-heavy
pathological case is within **~4.3x/2.4x** (down from ~1061x/574x). Normal,
non-adversarial documents are unaffected — this was never a normal-case
regression, only a worst-case one.

Also fixed as part of this work: `NodeKind::StrongEmphasis` (`***text***`)
rendered with the wrong tag order (`<strong><em>` instead of the
spec-correct `<em><strong>`); and an unbounded per-plain-text-chunk
lookahead scan (for autolinks, emoji shortcodes, and platform mentions)
that re-scanned the entire remaining document on every chunk instead of
being bounded to the current line, an `O(n²)` cost on any multi-line
paragraph with no matches.

### Added

#### Multi-core parallelism (`parallel-render`, `parallel-parse`)

Two new Cargo features, both **on by default** (they pull in `rayon`, a
real OS thread pool; targets that don't want threads — e.g. plain
`wasm32-unknown-unknown` — should build with `default-features = false`
and re-enable every other default feature explicitly, see README "Feature
flags"):

- `parallel-render` fans out per-code-block syntax highlighting across
  cores at render time. On a 16-core machine, a code-block-heavy document
  (80 fenced blocks) rendered **~6.9x faster** with the feature enabled.
- `parallel-parse` fans out inline parsing of independent top-level blocks
  (paragraphs, table cells, definition terms, footnote-definition bodies)
  across cores at parse time. On a flat, paragraph-heavy document (400
  top-level paragraphs), parsing was **~1.95x faster**. Content nested
  inside a list item, blockquote, or slide/tab panel is deliberately left
  on the sequential path — depth-gated after benchmarking showed
  parallelizing it naively regressed list-heavy documents by ~28%.

Both produce byte-for-byte HTML / AST-identical output to the sequential
path — a pure performance toggle, not a behavior change. Also
added: `warm_render_thread_pool(languages: &[&str])`, re-exported at the
crate root, lets an embedder pre-pay `parallel-render`'s one-time
thread-pool and per-language syntax-highlighter warm-up cost at a time of
its choosing (e.g. application startup) instead of on the first render. A
no-op when `parallel-render` isn't compiled in, so call sites don't need
their own `#[cfg]`.

#### Root-level `cargo bench`

`benches/core_benchmarks.rs` adds a small Criterion suite so `cargo bench`
works directly on the crate without a separate tool invocation — parse/
render/e2e timings against representative small/medium/large fixtures plus
the two pathological cases above. `tools/perf-lab` remains the source of
truth for cross-engine comparison, the full spec-suite corpus, stress
testing, and CI regression gating; this is just quick local feedback.

### Changed

#### CI performance regression gate now actually gates

`.github/workflows/ci-perf.yml`'s regression gate had two bugs that meant
it could never fail a PR, regardless of actual performance: its two
benchmark stages (`e2e` and `parse` mode) each overwrote the same "latest
artifact" file, so the parse-mode run's output silently discarded the
e2e run's, and the comparison against baseline then matched zero records
(mode mismatch) — "regression gate passed" was printed unconditionally.
Both fixed (both modes are now merged into one artifact before comparing).
The gate also gained a dedicated critical-workload check —
`spec:commonmark` and `fixture:pathological:*` now fail a PR on their own
past a 20% parse-time regression, rather than needing a second, unrelated
workload to also regress before the broad gate trips.

#### Dependency update and cleanup

Routine patch/minor bumps: `ron`, `log`, `chrono`, `serial_test` (dev).
Breaking bumps, internal-only (no marco-core public API exposes these
crates' types, so consumers are unaffected regardless): `emojis` 0.8→0.9,
`mermaid-rs-renderer` 0.2→0.3, `criterion` 0.5→0.8 (dev-only; 0.6 dropped
`criterion::black_box` for `std::hint::black_box`, updated accordingly).

Removed two dependencies confirmed unused anywhere in the repo: `fontconfig`
(Linux-only; was never called from `src/`, and CI no longer installs
`libfontconfig-dev` as a result) and `tempfile`. Moved `serde_json` from
`[dependencies]` to `[dev-dependencies]` — only the test suite used it,
not the library.

## [1.2.0] - 2026-07-08

### Added

#### Theme metadata parsing

Theme token CSS files can now declare descriptive `--theme-*` custom
properties (`--theme-name`, `--theme-author`, `--theme-license`,
`--theme-version`, `--theme-description`) alongside their colour/font
tokens. The new `render::theme_meta::parse_theme_metadata` function reads
these back out of a theme's raw CSS into a `ThemeMetadata` struct, so
consumers (e.g. a theme picker UI) can display attribution and version
info without deriving it from the filename.

## [1.1.1] - 2026-06-05

### Fixed

#### Task list items with nested blocks render correctly

Task list items that contain nested block elements (e.g. a sub-list of bullet
points) were rendered incorrectly: the nested `<ul>` appeared as a second
flex row-sibling of the checkbox icon instead of flowing below the label text.

The renderer now wraps the content of such items in
`<div class="marco-task-content">`, and the base stylesheet gives that element
`flex: 1; min-width: 0` so all nested blocks flow in a single column beside
the checkbox. Simple items (text only) are unaffected — no wrapper is emitted
when there are no nested block children.

## [1.1.0] - 2026-05-08

### Removed

#### `cache` feature and `ParserCache` API

The `cache` feature (backed by `moka`) and its public types — `ParserCache`,
`parse_to_html`, `parse_to_html_cached` — have been removed from `marco-core`.

Caching is application-level concern. Downstream consumers that need parse or
render caching should implement it directly (e.g. wrapping `parse` + `render`
in a `HashMap` or `moka::sync::Cache` keyed by content hash).

The `moka` dependency is no longer pulled in by default.

### Added

#### Public crate version constant

`marco-core` now exports `VERSION` from the crate root (`marco_core::VERSION`),
backed by `env!("CARGO_PKG_VERSION")`, so downstream apps can display the
library version in About dialogs and diagnostics.

#### `ParseOptions` and `parse_with_options` — runtime parser control

New public API that lets callers tune parse behaviour per-call without changing
compile-time features:

| Option | Default | Effect when `false` |
|---|---|---|
| `track_positions` | `true` | Skips O(n) span construction; nodes carry `span: None` |
| `parse_math` | `true` | Skips display-math and inline-math grammar branches |
| `parse_diagrams` | `true` | Skips mermaid fenced-code-block detection |

`parse_with_options(input, opts)` installs a `ParseOptionsGuard` that restores
the previous per-thread values on return or panic (RAII, thread-local).
`parse(input)` is unchanged and continues to enable all options.

#### Compile-time feature flags — eight optional feature gates

`Cargo.toml` now declares a `[features]` table with eight flags, all on by default:

| Feature | Gates |
|---|---|
| `render-math` | KaTeX math rendering (`dep:katex-rs`) |
| `render-diagrams` | Mermaid diagram rendering (`dep:mermaid-rs-renderer`) |
| `render-syntax-highlighting` | syntect code highlighting (`dep:syntect`) |
| `file-logger` | `SimpleFileLogger`, log rotation, log-path helpers |
| `intelligence-highlights` | `MarkdownIntelligenceProvider::highlights` (implies `render-syntax-highlighting`) |
| `intelligence-diagnostics` | `::diagnostics` and the analysis sub-module |
| `intelligence-completions` | `::completions` |
| `intelligence-hover` | `::hover` and `::get_position_span` |

Building with `--no-default-features` produces a minimal parser + renderer with
no optional dependencies. All 25 test suites pass in both configurations.
Test files that exercise optional APIs are gated with `#![cfg(feature = "...")]`.

#### `marco-core-raw` perf-lab engine adapter

A new `"marco-core-raw"` engine for `perf-lab compare` calls `parse_with_options`
with all three cost-saving options disabled, making it easy to measure the
runtime overhead of each opt-in feature in isolation.

#### `tools/perf-lab` — performance and regression benchmarking crate
A standalone `publish = false` Rust crate under `tools/perf-lab/` that provides
direct-timing benchmarks, stress testing, cross-engine comparison, artifact
management, and a CI-ready regression gate for `marco-core`.

Five subcommands:

| Subcommand | Purpose |
|---|---|
| `perf-lab bench` | Direct-timing benchmark — measures parse/render/e2e latency, writes `BenchRecord` JSON/CSV/Markdown artifacts |
| `perf-lab stress` | Stress harness — drives the parser at concurrency, captures per-sample stats and throughput |
| `perf-lab compare` | In-process speedup table against pulldown-cmark and comrak |
| `perf-lab report` | Artifact re-render; ingests hyperfine JSON for unified reporting |
| `perf-lab regression` | Regression gate — loads two `BenchRecord` JSON files, exits 1 when hard-failure count meets `--min-failures` threshold |

Shell wrappers under `tools/perf-lab/scripts/`:
`run-bench.sh`, `run-stress.sh`, `run-compare.sh`, `run-hyperfine.sh`, `run-regression.sh`.

Engine adapters: `pulldown-cmark 0.13`, `comrak 0.52` (no-default-features).

#### `.github/workflows/ci-perf.yml` — two-stage performance CI
- **Stage A** (push to `main`): builds release, runs `bench --mode e2e --iterations 30` and `bench --mode parse --iterations 30`, uploads a `BenchRecord` JSON artifact (30-day retention, keyed by `${{ github.sha }}`).
- **Stage B** (pull request): runs `bench --iterations 20` on the PR branch, downloads the base-SHA artifact, calls `perf-lab regression --warn-threshold 10 --fail-threshold 20 --min-failures 2`. Skips gracefully when no baseline artifact exists.

#### `.github/workflows/update-crate.yml` — crates.io update preflight
Manual `workflow_dispatch` workflow that runs `cargo publish --dry-run` and all
quality gates (`fmt`, `clippy -D warnings`, `test`) to validate the crate before
a real `cargo publish`.

#### `tools/marco-ast` — AST introspection CLI tool
Standalone `publish = false` binary crate (`tools/marco-ast/`) for inspecting the
`marco-core` parse tree. Supports text, JSON, and interactive tree-walk output
modes. Useful for debugging grammar and parser changes.

#### `tools/spec/` — CommonMark and extension specification fixtures
Spec fixture JSON files relocated from `tests/spec/` to `tools/spec/`:
`commonmark.json`, `gfm.json`, `marco.json`, `diagram.json`, and `README.md`.
The `Cargo.toml` `include` list updated accordingly so the published `.crate`
tarball continues to ship the fixtures.

#### `tools/tests/extension_spec_it.rs` — extension spec conformance test
Integration test for Marco-specific extension spec fixtures (loaded from
`tools/spec/marco.json` and `tools/spec/diagram.json`). Registered in
`Cargo.toml` as a named `[[test]]` target (`extension_spec_it`).

#### `Documentation/` — developer reference documents
| File | Purpose |
|---|---|
| `Documentation/testing.md` | Integration test inventory (676 total, 93 integration across 23 files) |
| `Documentation/tools.md` | Full developer reference for the `perf-lab` tooling |
| `Documentation/API Naming Standard.md` | CSS class and custom-property naming conventions |
| `Documentation/COMPARISON_RAW_vs_FULL_FEATURED.md` | Benchmark comparison: marco-core vs pulldown-cmark vs comrak |

### Changed

#### Parser and renderer — targeted performance improvements

Five changes that reduce parse+render latency by **10–37%** on typical workloads
with no API or feature changes:

1. **`to_parser_span` — single-pass newline scan** (`src/parser/shared.rs`)
   Replaced two separate O(n) passes (`matches('\n').count()` + `rfind('\n')`)
   with a single byte-scan loop that computes `newline_count` and `last_nl` together.

2. **Inline loop — first-byte fast-path** (`src/parser/inlines/mod.rs`)
   Normal prose bytes (`a-z`, `A-Z`, `0-9`, `.`, `,`, space, …) were triggering ~25
   sequential parser attempts before reaching `parse_text`. A first-byte dispatch
   now jumps straight to `parse_text` for bytes that cannot start any special inline
   sequence. Guarded edge cases: hard line break (`≥2 spaces + \n`), GFM autolink
   literals, emoji shortcodes, and platform mentions all still fall through to full
   dispatch when needed.

3. **`normalize_label` — no intermediate Vec** (`src/parser/ast.rs`)
   `split_whitespace().collect::<Vec<_>>().join(" ")` on every link reference lookup
   replaced with a direct `String` write loop.

4. **HTML renderer — allocation reductions** (`src/render/markdown.rs`)
   - Output `String` pre-allocated with `with_capacity(nodes × 64)` to reduce
     early reallocations.
   - Heading level rendered as a `char` via `char::from_digit` (no `to_string()` heap alloc).
   - `escape_html(&effective_id)` computed once and reused for both the `id` attribute
     and the anchor `href`.
   - `format!("<li id=\"fn{}\">" , n)` and `format!(" class=\"language-{}\"", …)`
     hot paths replaced with `push_str` / `push` chains.

5. **`parse_inlines_from_span` — Vec capacity hint** (`src/parser/inlines/mod.rs`)
   Inline node accumulator seeded with `Vec::with_capacity(8)` to avoid
   reallocation on the first few nodes.

Before / after (parse mode, 50-iteration mean, release build):

| Workload | Before (ns) | After (ns) | Δ |
|---|---|---|---|
| small (~1 KB) | 6 472 | 5 003 | −23% |
| medium (~10 KB) | 146 263 | 91 524 | −37% |
| large (~100 KB) | 2 727 867 | 2 103 670 | −23% |
| pathological | 11 516 501 | 9 127 307 | −21% |
| spec:gfm | 433 231 | 361 778 | −16% |

#### Integration tests — file names standardised to `*_it.rs`
All 23 integration test files under `tests/` were renamed to the `*_it.rs`
suffix pattern, replacing the previous mixed `*_integration.rs` naming.

| Previous name | New name |
|---|---|
| `autolink_highlighting.rs` | `autolink_highlighting_it.rs` |
| `commonmark_features_integration.rs` | `commonmark_features_it.rs` |
| `commonmark_spec_conformance.rs` | `commonmark_spec_it.rs` |
| `extended_definition_lists_integration.rs` | `definition_lists_it.rs` |
| `extended_heading_ids_integration.rs` | `heading_ids_it.rs` |
| `gfm_admonitions_integration.rs` | `gfm_admonitions_it.rs` |
| `gfm_autolink_literals_integration.rs` | `gfm_autolinks_it.rs` |
| `gfm_footnotes_integration.rs` | `gfm_footnotes_it.rs` |
| `gfm_table_integration.rs` | `gfm_tables_it.rs` |
| `gfm_tasklist_integration.rs` | `gfm_tasklist_it.rs` |
| `heading_anchor_links_integration.rs` | `heading_anchor_links_it.rs` |
| `highlighting_integration.rs` | `highlighting_it.rs` |
| `html_autolink_integration.rs` | `html_autolink_it.rs` |
| `html_block_single_line_integration.rs` | `html_block_single_line_it.rs` |
| `intelligence_provider_integration.rs` | `intelligence_provider_it.rs` |
| `marco_emoji_shortcode_integration.rs` | `marco_emoji_shortcode_it.rs` |
| `marco_headerless_table_integration.rs` | `marco_headerless_table_it.rs` |
| `marco_inline_footnotes_integration.rs` | `marco_inline_footnotes_it.rs` |
| `marco_sliders_integration.rs` | `marco_sliders_it.rs` |
| `marco_tab_blocks_integration.rs` | `marco_tab_blocks_it.rs` |
| `platform_mentions_integration.rs` | `platform_mentions_it.rs` |
| `sanitize_input_integration.rs` | `sanitize_input_it.rs` |

#### Rust Grammar API — doc-comments added to all public grammar types
All public grammar structs, enums, and functions in `src/grammar/` now carry
Rust doc-comments (`///` / `//!`), improving `cargo doc` output for crates.io
consumers.

#### Module-level doc-comments added throughout the codebase
`src/lib.rs`, `src/parser/mod.rs`, `src/render/options.rs`, `src/logic/mod.rs`,
`src/logic/logger.rs`, and all intelligence sub-modules now use `//!` inner
doc-comments, with the crate root exposing a runnable example in the top-level
rustdoc.

#### `.gitignore` — trimmed and updated for standalone crate layout
Removed coverage artifacts, legacy IDE config entries, and Windows thumbnails;
added `marco-core.code-workspace` and per-tool `target/` ignores
(`/tools/marco-ast/target/`, `/tools/perf-lab/target/`).

#### `devskim.yml` — ignore-glob path updated
Spec fixture ignore path updated from `**/tests/spec/**` to `**/tools/spec/**`
to match the relocated fixture directory.

#### `CONTRIBUTING.md` — spec test path references updated
`MARCO_SPEC_VERBOSE=1` example command and file path reference updated to
`tests/commonmark_spec_it.rs` (from the old `tests/commonmark_spec_conformance.rs`).

#### `src/logic/logger.rs` — smoke tests and documentation
Added `#[cfg(test)]` smoke tests for file logger initialisation; editorial
doc-comments clarified platform-specific paths to avoid editor-specific
references.

#### CI workflows — reshaped for standalone library
`ci-linux.yml` installs only `pkg-config` + `libfontconfig-dev`, runs
`cargo fmt` / `clippy -D warnings` / `test` / doc-tests.
`ci-windows.yml` switched to native MSVC (no MSYS2/GTK setup).
`publish-crate.yml` triggers on `v*` tags, runs `cargo publish --dry-run` before publishing.
`README.md` rewritten with a top-down pipeline ASCII diagram, a complete module/file map,
supported-Markdown matrix, and runnable usage snippets.
`.github/instructions/rust.instructions.md` retargeted at the
pure-Rust library context (removed GTK/GUI guidance).

#### Rust Grammar API — namespaced symbol renames
The public grammar types and parser functions for Marco-specific block extensions
now carry the `Marco` prefix (structs) or `marco_` prefix (functions), aligning
them with the `marco_*` file-naming convention and making their origin unambiguous
to crates.io consumers.

| Previous name | New name | File |
|---|---|---|
| `Slide` | `MarcoSlide` | `src/grammar/blocks/marco_sliders.rs` |
| `SlideDeck` | `MarcoSlideDeck` | `src/grammar/blocks/marco_sliders.rs` |
| `slide_deck()` | `marco_slide_deck()` | `src/grammar/blocks/marco_sliders.rs` |
| `TabItem` | `MarcoTabItem` | `src/grammar/blocks/marco_tab_blocks.rs` |
| `TabBlock` | `MarcoTabBlock` | `src/grammar/blocks/marco_tab_blocks.rs` |
| `tab_block()` | `marco_tab_block()` | `src/grammar/blocks/marco_tab_blocks.rs` |
| `HeaderlessTableBlock` | `MarcoHeaderlessTableBlock` | `src/grammar/blocks/marco_headerless_table.rs` |

#### HTML render — namespaced CSS class and custom-property renames
All runtime class names emitted by the renderer and defined in the embedded base
stylesheet now use the `marco-` namespace (BEM blocks) or `mc-` namespace (CSS
custom properties), preventing collisions with host-page styles.

**Sliders** (`marco-sliders` block):

| Previous class | New class |
|---|---|
| `slideshow` | `marco-sliders` |
| `slideshow__viewport` | `marco-sliders__viewport` |
| `slideshow__slide` | `marco-sliders__slide` |
| `slideshow__controls` | `marco-sliders__controls` |
| `slideshow__btn` | `marco-sliders__btn` |
| `slideshow__btn--prev` | `marco-sliders__btn--prev` |
| `slideshow__btn--next` | `marco-sliders__btn--next` |
| `slideshow__btn--toggle` | `marco-sliders__btn--toggle` |
| `slideshow__icon` | `marco-sliders__icon` |
| `slideshow__icon--play` | `marco-sliders__icon--play` |
| `slideshow__icon--pause` | `marco-sliders__icon--pause` |
| `slideshow__dots` | `marco-sliders__dots` |
| `slideshow__dot` | `marco-sliders__dot` |
| `slideshow__dot-icon` | `marco-sliders__dot-icon` |
| `slideshow__dot-icon--active` | `marco-sliders__dot-icon--active` |
| `slideshow__dot-icon--inactive` | `marco-sliders__dot-icon--inactive` |

**Tab blocks** (`marco-tabs` block):

| Previous class | New class |
|---|---|
| `tabset` | `marco-tabs` |
| `tabset__radio` | `marco-tabs__radio` |
| `tabset__tablist` | `marco-tabs__tablist` |
| `tabset__tab` | `marco-tabs__tab` |
| `tabset__panels` | `marco-tabs__panels` |
| `tabset__panel` | `marco-tabs__panel` |

**Task list**:

| Previous class | New class |
|---|---|
| `task-list-item` | `marco-task-list-item` |
| `task-list-item-checkbox` | `marco-task-checkbox` |
| `task-checkbox` | `marco-task-checkbox` |
| `task-icon` | `marco-task-icon` |
| `task-check` | `marco-task-check` |
| `task-box` | `marco-task-box` |

**Code blocks, headings, and inline elements**:

| Previous class | New class |
|---|---|
| `code-block` | `marco-code-block` |
| `copy-button` | `marco-copy-btn` |
| `heading-anchor` | `marco-heading-anchor` |
| `mention` / `mention-*` | `marco-mention` / `marco-mention-*` |
| `autolink` | `marco-autolink` |
| `inline-footnote` | `marco-inline-footnote` |
| `emoji` | `marco-emoji` |
| `diagram` | `marco-diagram` |
| `table-auto-align` | `marco-table-auto-align` |
| `table-resizing` | `marco-table-resizing` |
| `resize-active` | `marco-resize-active` |

**CSS custom properties**:

| Previous property | New property |
|---|---|
| `--task-primary` | `--mc-task-primary` |
| `--task-accent` | `--mc-task-accent` |
| `--slideshow-border` | `--mc-sliders-border` |
| `--slideshow-bg` | `--mc-sliders-bg` |

#### Preview document — namespaced HTML IDs and JS bridge rename
The preview document wrapper (`src/render/preview_document.rs`) now uses the
`mc-` prefix for HTML element IDs and exposes the JavaScript bridge under a
stable namespaced global.

| Previous value | New value |
|---|---|
| `id="content-container"` | `id="mc-content-container"` |
| `id="preview-style"` | `id="mc-preview-style"` |
| `id="preview-internal-style"` | `id="mc-preview-internal-style"` |
| `id="paged-page-css"` | `id="mc-paged-page-css"` |
| `id="print-export-css"` | `id="mc-print-export-css"` |
| `window.PreviewBridge` | `window.MarcoCorePreview` |
| `document.title = 'paged_ready'` | `document.title = 'mc_paged_ready'` |

### Fixed
- Broken import in `src/parser/blocks/marco_headerless_table_parser.rs` that
  referenced a stale module path (`headerless_table`) instead of the correct
  `marco_headerless_table` module, which caused a compile error in test builds.

- CommonMark spec conformance test (`tests/commonmark_spec_it.rs`)
  loading the official 652 examples (and a small set of Marco extras) from
  `tools/spec/*.json`, with a regression-guard baseline. Set
  `MARCO_SPEC_STRICT=1` to require 100% conformance,
  `MARCO_SPEC_VERBOSE=1` to print failing examples.
- `documentation` key and explicit `include` list in `Cargo.toml`. The
  published `.crate` tarball now ships `src/`, `README.md`, `LICENSE`,
  `Cargo.toml`/`Cargo.lock`, and the spec fixtures
  (`tools/spec/*.json` + `tools/spec/README.md`) for downstream
  conformance verification; everything else (`.github/`, integration
  test sources, `target*/`, `CHANGELOG.md`) stays out.
- `tools/spec/README.md` documenting the upstream source and
  CC-BY-SA 4.0 attribution for `commonmark.json`.

### Removed
- Stale `target-linux/` build artifact directory left over from the former
  workspace layout.
- `marco-rust` chat mode (belonged to the consuming editor, not this crate).

## [1.0.2] - 2026-04-28

### Added
- New CI workflow for publishing `marco-core` to crates.io.

### Changed
- Debian package runtime dependency compatibility was broadened for newer Ubuntu-family distributions by allowing `libxml2-16` as an alternative to `libxml2`.
- Workspace refactor renamed the legacy `core` crate to `marco-core` and moved app-oriented shared logic/assets into `marco-shared`, clarifying the reusable library boundary.

### Fixed
- Linux package build script path handling was hardened to correctly locate built binaries when Cargo target output is configured outside the default `target/` directory.

### Removed
- Removed the legacy `core` crate path/name from the workspace in favor of `marco-core`.

### Security
- Verified mitigation status for GHSA-82j2-j2ch-gfr8: dependency graph resolves to patched `rustls-webpki` 0.103.13.
- Updated transitive `rand` to 0.8.6 across the workspace lockfile to keep the dependency graph current with upstream advisories.

## [1.0.0] - 2026-04-15

### Added
- First stable release published to crates.io as `marco-core`.
- Public API: `parse`, `Document`, `Node`, `NodeKind`, `render`, `RenderOptions`, `MarkdownIntelligenceProvider`, `ParserCache`, `parse_to_html`, `parse_to_html_cached`, `sanitize_input`, `sanitize_input_with_stats`.
- `mermaid-rs-renderer` dependency switched from git source to crates.io (`0.2.1`).

### Changed
- Library and shared crates (`marco-core`, `marco-shared`) now version independently from the application binaries (`marco`, `polo`).
- Removed `global_parser_cache`, `shutdown_global_parser_cache`, and `safe_preview` from the crate-root public API — these remain accessible at `marco_core::logic::cache` / `marco_core::logic::logger` for internal use.

### Security
- Nothing yet.

## [0.23.1] - 2026-04-14

### Changed
- Rust toolchain updated to 1.94.1 (MSRV bumped from 1.93.0).
- GTK ecosystem upgraded to gtk4 0.11.2 / glib 0.22.5 / cairo-rs 0.22.0 / gdk-pixbuf 0.22.0 / gio 0.22.5 series.
- `reqwest` updated to 0.13 and `toml` updated to 1.x.

### Security
- Updated `rand` dependency to address unsound behavior (GHSA-cq8v-f236-94qc).

## [0.23.0] - 2026-04-12

### Added
- Paged preview document API for CSS Paged Media workflows: `wrap_preview_html_document_paged` with `PageViewOptions` (paper size, orientation, margins, page numbers, columns per row, export mode, and standalone-export mode).
- Base preview stylesheet module (`render::base_css`) and compositing pipeline that layers structural base CSS first, then theme tokens.
- New layout settings fields for page-view and zoom state persistence: page-view enabled flag, paper, orientation, margin, page-number toggle, update delay, columns-per-row, and preview zoom.

### Changed
- Preview theming model was refactored from monolithic per-theme stylesheets to base structural CSS plus theme-token overrides, reducing duplication and aligning rendering across themes.
- Heading anchors now wrap the full heading text, making the heading itself the clickable permalink target.

### Fixed
- Hard line break parsing now correctly accepts two **or more** trailing spaces before newline (CommonMark behavior), and consumes all trailing spaces to avoid stray-space artifacts before `<br />`.
- Paragraph blank-line detection now follows CommonMark blank-line rules (ASCII space/tab only), so non-breaking-space spacer paragraphs are preserved.
- Inline link parsing now handles nested bracket content correctly (for example image-in-link syntax like `[![alt](img)](url)`).

### Removed
- Removed decorative anchor/link icon adorners from generated preview heading/link markup.

## [0.22.0] - 2026-04-08

### Added
- `intelligence::toc` module — extracts a table of contents from a parsed document (`extract_toc`), generates URL-safe heading slugs, produces Markdown TOC output (`generate_toc_markdown`), and replaces or inserts TOC marker blocks in source text (`replace_toc_in_text`).
- `table_auto_align` field added to `EditorSettings` and `toc_depth` field added to `LayoutSettings` in the settings schema.

### Changed
- HTML renderer now generates an `id` attribute and a hover-anchor link for every heading, including headings without an explicit `{#id}` marker; duplicate slugs receive a `-1`, `-2`, … suffix to guarantee uniqueness.

## [0.21.0] - 2026-03-13

### Added
- In-process intelligence engine (`core/src/intelligence/`) — replaces the previous `lsp/` module; provides syntax highlighting, hover information, Markdown completions, and diagnostic analysis behind a clean public API boundary.
- Two diagnostics catalogs in RON format (`diagnostics_catalog_marco.ron`, `diagnostics_catalog_markdownlint.ron`) — map diagnostic codes to human-readable titles, descriptions, severity, and fix suggestions.
- `get_position_span` — returns the tightest AST node span enclosing a cursor position; supports hover suppression logic in the editor.
- `intelligence/markdown/` boundary module — structured reinterpretation of the AST for intelligence consumers (blocks and inlines classification).
- LSP protocol adapter stub (`intelligence/lsp_protocol.rs`) for future language server integration.
- `DiagnosticsFilterSettings` struct in `EditorSettings` — per-severity toggles (errors, warnings, hints, infos) for the diagnostics display; replaces the previous `linting` boolean.
- `diagnostics_underlines_enabled`, `diagnostics_hover_enabled`, and `markdown_hover_enabled` fields in `EditorSettings` for fine-grained intelligence feature control.
- `SettingsManager::reload_settings` is now `pub` to allow external callers to trigger a settings reload.

### Changed
- Replaced `core/src/lsp/` module with `core/src/intelligence/` — Markdown intelligence capabilities (highlights, diagnostics, completions, hover) are now organized under feature-area sub-modules (`editor/`, `markdown/`, `analysis/`, `catalog/`).
- `EditorSettings`: removed `auto_pairing` and `linting` fields; added `diagnostics_underlines_enabled`, `diagnostics_hover_enabled`, `markdown_hover_enabled`, and `diagnostics_filter`.

### Fixed
- Image nodes now carry the span of the full `![alt](url)` syntax rather than just the alt-text fragment; this prevents zero-length spans when alt text is empty.
- Footnote definition nodes had their span end set to end-of-document due to use of the inclusive span range helper; corrected to use the exclusive variant so subsequent content is not erroneously enclosed within a footnote definition span.

### Removed
- Old `core/src/lsp/` module (completion, diagnostics, highlights, hover) superseded by `core/src/intelligence/`.

## [0.20.0] - 2026-03-04

### Added
- Centralized settings manager (`SettingsManager` / `Settings` in `logic/swanson.rs`) — thread-safe RON-based settings with typed sub-structs for editor, appearance, layout, language, telemetry, window, file, and polo configuration; supports change listeners, file I/O, validation, and auto-repair on load.
- Bookmark and emoji history persistence in `Settings`: `get_bookmarks`, `set_bookmarks`, `record_emoji_usage`, `get_top_emoji_usage`, and `clean_recent_files`.
- `PoloSettings` and `PoloWindowSettings` structs for Polo-specific persistent configuration.
- Emoji shortcode completion API (`logic/text_completion.rs`) — OnceLock-cached static list of `EmojiCompletionItem` values with `normalize_completion_query` and `emoji_shortcode_matches_query` prefix-match helpers.
- Pure-Rust Mermaid diagram rendering (`render/diagram.rs`) — native diagram rendering via `mermaid_rs_renderer` with a `MERMAID_MAX_CHARS` safety limit; full GitHub-style dark and light themes via `create_mermaid_theme`.
- KaTeX math rendering (`render/math.rs`) — `render_inline_math` and `render_display_math` with MathML output and a global `OnceLock<KatexContext>` for performance.
- Unified HTML preview document builder (`render/preview_document.rs`) — `wrap_preview_html_document()` shared by Marco and Polo; embeds interactive table-resize CSS and JS, heading anchor link CSS and SVG, background-flash prevention, and the `window.MarcoPreview` JS API.
- Layout state enum (`logic/layoutstate.rs`) — `LayoutState { DualView, EditorOnly, ViewOnly, EditorAndViewSeparate }` with a `layout_state_label` string helper.
- Cross-platform detection helpers (`logic/crossplatforms.rs`) — `Platform { Linux, Windows, Unknown }` enum, `detect_platform()`, and `is_dark_mode_supported()`.

### Changed
- Settings system expanded from minimal telemetry and window structs (0.18.0) to a full application settings hub with typed per-component structs, audit-logged file I/O, and per-component typed change listeners.

## [0.18.0] - 2026-02-09

### Added
- System locale detection helper (`detect_system_locale_iso639_1`) with ISO 639-1 normalization.
- Telemetry settings support (persisted settings fields for enabling telemetry + tracking whether the first-run dialog has been shown).
- Inline SVG icons for About dialog link buttons (GitHub, Link, Bug, Help).

### Changed
- Portable mode detection on Linux and Windows now prefers a writable `config/` directory next to the executable (more reliable portable installs).
- Asset root validation now requires `language/` alongside `icons/` and `themes/`.
- Settings change listeners are now stored as `Arc` and notified outside the listener lock (reduces lock contention and avoids re-entrancy hazards).

### Fixed
- Reduced false-positive portable-mode detection in development environments by tightening Linux portable heuristics.

## [0.17.1] - 2026-02-04

### Changed
- Improved cross-platform compilation with refined conditional compilation attributes for Linux and Windows builds.
- Enhanced platform-specific code organization using explicit `cfg(target_os)` gates.

### Fixed
- Resolved Linux build compilation error in font loader module (missing `HashMap` import under platform-specific code path).

## [0.17.0] - 2026-02-03

### Added
- **Platform abstraction module** (`core::paths::platform`) for OS-specific path implementations.
- **Windows portable mode detection** - automatically uses local config/data when exe directory is writable.
- **Linux platform paths** - XDG-compliant user directories (`~/.local/share/marco`, `~/.config/marco`).
- **Windows platform paths** - Standard Windows locations (`%LOCALAPPDATA%\Marco`) with portable fallback.
- **Asset root validation** - verifies asset bundles contain required directories before accepting paths.

### Changed
- **Refactored path system** to use platform-specific modules with explicit `cfg` attributes.
- **Install location detection** now properly distinguishes between system/user/portable installs.

## [0.16.0] - 2026-02-02

### Added
- **Full cross-platform support** for Windows and Linux.
- Platform-agnostic core library works identically on both platforms.

### Changed
- Migrated logger from `static mut` to `OnceLock<T>` for Rust 2024 compatibility.
- Removed all unsafe blocks for logger access in favor of safe initialization pattern.

### Fixed
- Fixed `static_mut_refs` warnings to comply with Rust 2024 edition.

## [0.15.2] - 2026-01-30

### Added
- Added `DualView` layout **inline SVG** to the Core icon loader.
- Icon loader documentation updated to describe inline SVG usage and HiDPI rasterization expectations.

### Changed
- Icon loader and related docs updated to formally deprecate icon-font usage across the workspace and to prefer inline SVG assets.

### Fixed
- N/A

### Removed
- Removed the legacy `icon_font()` helper from `core::paths::SharedPaths` (icon-font helper was no longer used).

### Security
- Nothing yet.

## [0.15.1] - 2026-01-26

### Changed
- Icon loader now supports inline SVG rendering for window controls
  - Added window icon SVG generation (minimize, maximize, restore, close)
  - Integrated with rsvg/librsvg for high-quality SVG rasterization

## [0.15.0] - 2026-01-25

### Added
- Cross-platform file path support for Windows and Linux
  - Asset root discovery now supports Windows paths (`%LOCALAPPDATA%`, `%PROGRAMFILES%`, `%PROGRAMDATA%`)
  - Install location detection works for both Linux (`/usr/share`, `~/.local/share`) and Windows (Program Files, AppData)
  - Platform-appropriate log directories (Linux: `~/.cache/marco/logs`, Windows: `%LOCALAPPDATA%\Marco\logs`)
  - Config and data directories use platform-specific locations via `dirs` crate

### Changed
- Logger now uses platform-appropriate cache directory instead of hardcoded `cwd/log`
- Path detection uses conditional compilation for Linux and Windows specific logic

### Fixed
- Removed `anyhow` dependency, replaced with standard `Result<T, Box<dyn std::error::Error>>`
- Fixed all Result type annotations throughout parser and logic modules
- Fixed thread safety issues for error types used with GTK's `gio::spawn_blocking`

### Removed
- `anyhow` dependency removed from core library

## [0.14.0] - 2026-01-18

### Added
- Extended GitHub-style admonitions with custom headers, e.g. `> [:joy: Happy Header]` (quote-styled, with optional emoji/icon + custom title).
- Marco_Extended tab blocks (`:::tab` + `@tab ...` + closing `:::`), parsed into `TabGroup`/`TabItem` and rendered as a no-JS tab UI in HTML preview.
- Marco_sliders slideshow decks (`@slidestart[:tN]` … `@slideend` with `---` / `--` separators), parsed into a dedicated AST and rendered as an interactive slideshow in the preview.
- Source-aware LSP highlighting helper (`compute_highlights_with_source`) to color structural marker lines like tab block markers.

## [0.13.3] - 2026-01-17

### Added
- Per-application icon/logo helpers in `SharedPaths` (Marco vs Polo), with backwards-compatible fallbacks for older shared icon filenames.
- New Marco and Polo logo/icon assets are now supported by the shared asset resolution layer.

### Changed
- Shared asset icon resolution now prefers the Marco/Polo-specific icon/favicons when present.

## [0.13.2] - 2026-01-15

### Added
- Extended definition lists (Markdown Guide / Markdown Extra-style).
- Headerless pipe tables (delimiter-first, no header row).
- Inline footnotes (`^[...]`) (inline definitions collected into the document footnotes section).
- Platform mentions (`@username[platform]` with optional display name) rendered as profile links for known platforms.
- Inline task checkbox markers (`[ ]` / `[x]` / `[X]`) inside paragraphs (including mid-paragraph markers and after hard line breaks).

### Changed
- Emoji shortcode handling now uses a full shortcode dataset for broader support.
- LSP highlight output is now sorted and de-duplicated for deterministic application.

### Fixed
- Prevented a UTF-8 boundary slicing panic when scanning for emoji shortcode candidates.

## [0.13.1] - 2026-01-14

### Changed
- Reduced build footprint by removing unused direct dependencies.
- Made external-link CSS selectors in the preview document more tolerant (match `http:`/`https:` prefixes).

### Security
- Reduced DevSkim/code-scanning noise by avoiding insecure URL literals in non-user-facing examples/tests.

## [0.13.0] - 2026-01-14

### Added
- Syntax-highlighted code rendering.
- Emoji shortcodes in rendered output.
- Footnotes.
- Extended heading identifiers.

## [0.12.0] - 2026-01-13

### Added
- Editor/LSP support for task list checkboxes and tables.

## [0.11.0] - 2026-01-12

### Changed
- Packaging/build workflow for Linux installs was updated and simplified.

## [0.10.0] - 2026-01-11

### Added
- GitHub Flavored Markdown tables.
- Additional inline formatting extensions.

## [0.9.0] - 2025-10-28

### Fixed
- More robust handling of autolinks vs inline HTML (reduces false-positive autolinks around common tags).

## [0.8.0] - 2025-10-27

### Fixed
- More consistent parsing of thematic breaks.

## [0.7.0] - 2025-10-25

### Added
- Syntax highlighting support in editor integrations.

## [0.6.0] - 2025-10-24

### Changed
- Theme color definitions were standardized for consistent appearance.

## [0.5.0] - 2025-10-23

### Added
- Editor/LSP completions and diagnostics for common Markdown structures (including lists, blockquotes, thematic breaks, and HTML blocks).

### Changed
- Linux install flow moved toward packaged installation.

## [0.4.0] - 2025-10-21

### Changed
- Parsing pipeline was modularized and legacy components were removed to improve maintainability and stability.

## [0.3.0] - 2025-10-20

### Added
- Unicode normalization for more consistent parsing of equivalent text.
- Link reference definitions.
- HTML blocks.

### Changed
- Block parsing improved for nested structures and blank-line edge cases.

## [0.2.0] - 2025-10-19

### Fixed
- Improved handling of invalid or tricky UTF-8 input.

## [0.1.0] - 2025-10-18

### Added
- Initial Core crate integration as a shared library used by Marco and Polo.

### Changed
- Parsing engine was rebuilt around a nom-based pipeline.
