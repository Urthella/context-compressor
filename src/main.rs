use std::io::{self, Read, Write};
use std::process::ExitCode;

const USAGE: &str = "\
Usage: ctxc [FILE] [--budget TOKENS] [--quiet]

Shrinks terminal output to fit an LLM token budget. Reads FILE or stdin,
writes to stdout, and prints a one-line summary to stderr.

  -b, --budget TOKENS   target size in tokens (default 2000)
  -q, --quiet           no summary on stderr
  -h, --help            show this help

Example: cargo test 2>&1 | ctxc -b 1500";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ctxc: {e}\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut budget = 2000;
    let mut quiet = false;
    let mut file = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(());
            }
            "-q" | "--quiet" => quiet = true,
            "-b" | "--budget" => {
                budget = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|&b| b > 0)
                    .ok_or("--budget needs a positive number")?;
            }
            a if a.starts_with('-') && a != "-" => return Err(format!("unknown option {a}")),
            a if file.is_none() => file = Some(a.to_string()),
            a => return Err(format!("unexpected argument {a}")),
        }
    }

    // Logs are not always valid UTF-8; replace bad bytes instead of failing.
    let bytes = match file.as_deref() {
        None | Some("-") => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| format!("reading stdin: {e}"))?;
            buf
        }
        Some(path) => std::fs::read(path).map_err(|e| format!("{path}: {e}"))?,
    };
    let (output, stats) = ctxc::compress(&String::from_utf8_lossy(&bytes), budget);

    let mut stdout = io::stdout().lock();
    // A closed pipe (`ctxc log | head`) is not an error worth reporting.
    if writeln!(stdout, "{output}").is_err() {
        return Ok(());
    }
    if !quiet {
        // Tiny inputs can grow by a "[repeated ×2]" label, so clamp at 0% instead of underflowing.
        let saved = 100usize.saturating_sub(
            (stats.output_tokens * 100)
                .checked_div(stats.input_tokens)
                .unwrap_or(100),
        );
        eprintln!(
            "ctxc: {} lines / ~{} tokens -> {} lines / ~{} tokens ({saved}% smaller)",
            stats.input_lines, stats.input_tokens, stats.output_lines, stats.output_tokens
        );
    }
    Ok(())
}
