# Spec fixtures

Test fixtures used by:

- `tests/commonmark_spec_it.rs` — loads `commonmark.json` + `diagram.json` + `gfm.json` + `marco.json` + `math.json`
- `tools/tests/extension_spec_it.rs` — loads `diagram.json` + `gfm.json` + `marco.json` + `math.json` + `combos.json`
- `tests/render_byte_identity_it.rs` — renders every example and compares against a committed baseline
- `tools/perf-lab` — as **benchmark workloads**, see below

All three test suites render with `RenderOptions::commonmark()`, never
`::default()`. The default escapes raw HTML, and these fixtures are full of it,
so measuring under the safe policy would measure the policy rather than the
parser.

## A note on how `perf-lab` uses these files

`perf-lab` feeds each `.json` file to the parser **whole, as Markdown text** —
not the `markdown` field of each example. So the `spec:gfm` workload is the
6 KB JSON file parsed as a document, which produces one long paragraph of text
and soft breaks rather than the constructs the fixture describes.

That is fine for a stable, reproducible workload, but it means `spec:*`
benchmark numbers say nothing about the performance of the feature the fixture
is named after. Use `fixtures/` for representative documents.

## `commonmark.json`

Sourced from the **CommonMark spec** test suite
(<https://spec.commonmark.org/>, <https://github.com/commonmark/commonmark-spec>).

The CommonMark spec and its accompanying test suite are © 2014–present John
MacFarlane and contributors, distributed under the
[Creative Commons CC-BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)
license. This file is included unmodified for regression-testing purposes.

If you redistribute this file, you must preserve this attribution and license
notice.

## Extension fixtures

The following fixture files are authored for this repository and licensed under
the same MIT license as the rest of `marco-core`:

| File | Contents |
|---|---|
| `diagram.json` | Mermaid / diagram-focused cases and edge cases |
| `gfm.json` | GitHub Flavored Markdown extension cases |
| `marco.json` | Marco-specific extension cases |
| `math.json` | Math delimiter and rendering cases |
| `combos.json` | Cross-feature combination and backslash-escape cases |
