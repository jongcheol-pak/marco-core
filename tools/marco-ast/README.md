# marco-ast

`marco-ast` is a standalone developer CLI for inspecting `marco-core` parsing,
rendering, and intelligence behavior.

It supports:

- AST tree output
- HTML render output
- Intelligence diagnostics/highlight summaries
- Per-node span inspection
- UTF-8 byte/char inspection
- Timing breakdowns (sanitize / parse / render / intel)
- Structured JSON reports for automation

## Build

```bash
cargo run --manifest-path tools/marco-ast/Cargo.toml -- --help
```

## Common Usage

From text:

```bash
cargo run --manifest-path tools/marco-ast/Cargo.toml -- \
  --text "# Title\n\nHello  \nworld" --mode both
```

From file:

```bash
cargo run --manifest-path tools/marco-ast/Cargo.toml -- \
  path/to/input.md --mode ast
```

## Inspection Flags

- `--spans`: print line/column/byte ranges for AST nodes
- `--excerpts`: print source excerpt slices next to AST nodes
- `--utf8`: print UTF-8 scalar and byte-vs-char diagnostics
- `--time`: print timing summary (us)
- `--json`: emit a machine-readable JSON report

Example full inspection run:

```bash
cargo run --manifest-path tools/marco-ast/Cargo.toml -- \
  /tmp/sample.md --mode both --spans --excerpts --utf8 --time
```

Example JSON report:

```bash
cargo run --manifest-path tools/marco-ast/Cargo.toml -- \
  --text "# Tëst\n\nHello" --mode both --json --spans --utf8 --time
```

## Modes

- `--mode ast`
- `--mode html`
- `--mode both`
- `--mode intel`

## Rendering policy

`--mode html` and `--mode both` render with `RenderOptions::commonmark()`, so
raw HTML passes through and URLs are emitted unfiltered — output matches
marco-core 1.x.

That is deliberate: a tool whose purpose is showing what the renderer does must
not silently apply a policy you did not ask for. It also means **marco-ast is
not a safety check.** Seeing `<script>` in its output tells you the parser
produced a raw-HTML node, not that a consumer would emit one — under the
library's default (`RenderOptions::default()`) that node is escaped.

To see what a policy does, use the library directly or the `marco-core-safe` /
`marco-core-untrusted` adapters in [`tools/perf-lab`](../perf-lab/README.md).

## Interactive REPL

```bash
cargo run --manifest-path tools/marco-ast/Cargo.toml -- --interactive --mode both
```

Commands:

- `:mode <ast|html|both|intel>`
- `:clear`
- `:help`
- `:quit`
