# tools/tests

Extension spec conformance tests that run as `[[test]]` targets inside the
`tools/perf-lab` crate. These tests load fixture files from `tools/spec/` and
assert strict `parse → render → assert_eq!` equality.

## `extension_spec_it.rs`

| Test function | Fixture | Feature gate |
|---|---|---|
| `test_diagram_fixtures_match_expected_html` | `tools/spec/diagram.json` | `render-diagrams` |
| `test_gfm_fixtures_match_expected_html` | `tools/spec/gfm.json` | _(none)_ |
| `test_marco_fixtures_match_expected_html` | `tools/spec/marco.json` | _(none)_ |
| `test_math_fixtures_match_expected_html` | `tools/spec/math.json` | `render-math` |
| `test_combos_fixtures_match_expected_html` | `tools/spec/combos.json` | _(none)_ |

Run all five suites:

```bash
cargo test --manifest-path tools/perf-lab/Cargo.toml --test extension_spec_it
```

Run a single suite:

```bash
cargo test --manifest-path tools/perf-lab/Cargo.toml --test extension_spec_it \
  test_marco_fixtures_match_expected_html
```

Each failing case prints: fixture name, example number, section, line range,
input markdown, expected HTML, and actual HTML.

## Rendering policy

These suites render with `RenderOptions::commonmark()`, **not** `::default()`.
The default escapes raw HTML and filters URL schemes, so a fixture asserting
exact HTML would be measuring the security policy rather than the extension
under test. No extension fixture currently contains raw HTML, so `default()`
would pass today — by luck, not design, and the next fixture that did would fail
for a reason unrelated to what it tests.

See [`tools/spec/README.md`](../spec/README.md) for fixture file descriptions
and [`Documentation/TESTING.md`](../../Documentation/TESTING.md) for the
broader test inventory.
