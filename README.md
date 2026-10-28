# context-compressor

[![ci](https://github.com/Urthella/context-compressor/actions/workflows/ci.yml/badge.svg)](https://github.com/Urthella/context-compressor/actions/workflows/ci.yml)

Shrinks noisy terminal output (test runs, builds, server logs) to fit an LLM token budget, without losing the lines that matter. Deterministic, no model calls, zero dependencies.

```bash
cargo test 2>&1 | ctxc -b 1500
```

## Example

A `cargo test` run with 400 tests and one failure, **423 lines / ~3,261 tokens**, becomes this:

```text
   Compiling billing v0.1.0 (E:\gh-work\.demo)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 4.42s
     Running unittests src\lib.rs (E:/gh-work/.target\debug\deps\billing-4f4f2d9c172714e4.exe)

running 400 tests
test tests::invoice_0 ... ok  [+127 similar]
test tests::invoice_217 ... FAILED
test tests::invoice_218 ... ok  [+270 similar]

failures:

---- tests::invoice_217 stdout ----

thread 'tests::invoice_217' (24484) panicked at src\lib.rs:220:28:
assertion `left == right` failed: rounding drift in invoice 217
  left: 200
 right: 201
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

failures:
    tests::invoice_217

test result: FAILED. 399 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s

error: test failed, to rerun pass `--lib`
```

```text
ctxc: 423 lines / ~3261 tokens -> 25 lines / ~205 tokens (94% smaller)
```

A verbose `npm ci --loglevel silly` log goes from 349 lines / ~5,357 tokens to 55 lines / ~660 tokens at `-b 800`, with the dropped stretches marked `… N lines omitted …`.

## Install

```bash
cargo install --git https://github.com/Urthella/context-compressor
```

## Use

```bash
cargo test 2>&1 | ctxc -b 1500           # pipe anything in
ctxc server.log --budget 4000 > ctx.txt  # or read a file
ctxc -q build.log                        # no summary line on stderr
```

The summary goes to stderr, so stdout stays clean for pasting or piping into an agent.

**With coding agents**: add a line like *"Pipe long command output through `ctxc` before reading it"* to your `CLAUDE.md` / `AGENTS.md`, and the agent spends its context on the failure instead of 400 passing tests.

## How it works

1. **Clean**: keep only the last frame of `\r` progress bars, strip ANSI colors and trailing spaces.
2. **Collapse**: identical runs become `line  [repeated ×N]`. Runs that differ only in words containing digits (counters, ids, hashes, timestamps, ports) become `first line  [+N similar]`. Blank runs become one blank line.
3. **Fit**, only if still over budget: keep every important line with one line of context, then fill from the head and tail alternately. Each gap is marked `… N lines omitted …`.

Important means a whole word like `error`, `ERR`, `FAILED`, `panicked`, `fatal`, `warning`, `Traceback`, or any word ending in `Error` / `Exception` (`TypeError`, `NullPointerException`). A package named `http-errors` doesn't count.

Tokens are estimated at ~4 bytes each, which is close enough for budgeting English text and code.

## Library

```rust
let (output, stats) = ctxc::compress(&log, 2000);
eprintln!("{} -> {} tokens", stats.input_tokens, stats.output_tokens);
```

## Develop

```bash
cargo test
```

`nix develop` (or direnv) gives you the same Rust toolchain. CI runs fmt, clippy and tests on Linux, macOS and Windows.

## License

[MIT](LICENSE)
