# context-compressor

[![ci](https://github.com/Urthella/context-compressor/actions/workflows/ci.yml/badge.svg)](https://github.com/Urthella/context-compressor/actions/workflows/ci.yml)

Shrinks noisy terminal output (test runs, builds, server logs) to fit an LLM token budget, without losing the lines that matter. Deterministic, no model calls, zero dependencies.

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
