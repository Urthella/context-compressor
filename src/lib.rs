//! Shrinks noisy terminal output (build logs, test runs, server logs) to fit an
//! LLM token budget, without losing the lines that matter.

const IMPORTANT: [&str; 10] = [
    "err",
    "fail",
    "failed",
    "failure",
    "panic",
    "panicked",
    "fatal",
    "traceback",
    "warn",
    "warning",
];

#[derive(Debug, PartialEq)]
pub struct Stats {
    pub input_lines: usize,
    pub input_tokens: usize,
    pub output_lines: usize,
    pub output_tokens: usize,
}

/// Rough token count: ~4 bytes per token for English text and code.
// ponytail: byte heuristic; swap in a real tokenizer (tiktoken-rs) if budgets must be exact.
pub fn tokens(s: &str) -> usize {
    s.len().div_ceil(4)
}

struct Group {
    text: String,
    count: usize,
    identical: bool,
    important: bool,
}

pub fn compress(input: &str, budget: usize) -> (String, Stats) {
    let groups = group_lines(input);
    let lines: Vec<String> = groups.iter().map(render).collect();
    let input_lines = groups.iter().map(|g| g.count).sum();

    let full = lines.join("\n");
    let output = if tokens(&full) <= budget {
        full
    } else {
        fit(&groups, &lines, budget)
    };

    let stats = Stats {
        input_lines,
        input_tokens: tokens(input),
        output_lines: if output.is_empty() {
            0
        } else {
            output.lines().count()
        },
        output_tokens: tokens(&output),
    };
    (output, stats)
}

/// Cleans each line, then merges runs of lines that are identical or differ only in
/// numbers, ids and timestamps (`GET /users/17 200` and `GET /users/42 200`).
fn group_lines(input: &str) -> Vec<Group> {
    let mut groups: Vec<(String, Group)> = Vec::new();
    for raw in input.lines() {
        let text = clean(raw);
        let key = template(&text);
        match groups.last_mut() {
            Some((k, g)) if *k == key => {
                g.count += 1;
                g.identical &= g.text == text;
                g.important |= is_important(&text);
            }
            _ => groups.push((
                key,
                Group {
                    important: is_important(&text),
                    text,
                    count: 1,
                    identical: true,
                },
            )),
        }
    }
    groups.into_iter().map(|(_, g)| g).collect()
}

fn render(g: &Group) -> String {
    match (g.count, g.identical) {
        (1, _) => g.text.clone(),
        _ if g.text.is_empty() => String::new(), // a run of blank lines becomes one
        (n, true) => format!("{}  [repeated ×{n}]", g.text),
        (n, false) => format!("{}  [+{} similar]", g.text, n - 1),
    }
}

/// Over budget: keep important lines with one line of context, then fill from the
/// head and tail alternately, and mark every gap with how many lines it hides.
fn fit(groups: &[Group], lines: &[String], budget: usize) -> String {
    let n = lines.len();
    let limit = budget - budget / 10; // leave room for the "omitted" markers
    let mut keep = vec![false; n];
    let mut used = 0;
    let mut take = |i: usize| -> bool {
        if !keep[i] {
            let cost = tokens(&lines[i]) + 1;
            if used + cost > limit {
                return false;
            }
            keep[i] = true;
            used += cost;
        }
        true
    };

    // ponytail: errors are kept in order until the budget runs out, so a flood of errors favors the first ones.
    for i in (0..n).filter(|&i| groups[i].important) {
        for j in i.saturating_sub(1)..=(i + 1).min(n - 1) {
            take(j);
        }
    }
    let (mut head, mut tail) = (0, n);
    while head < tail {
        let took_head = take(head);
        head += 1;
        let took_tail = head < tail && {
            tail -= 1;
            take(tail)
        };
        if !took_head && !took_tail {
            break;
        }
    }

    let mut out: Vec<String> = Vec::new();
    let mut hidden = 0;
    for (i, line) in lines.iter().enumerate() {
        if keep[i] {
            if hidden > 0 {
                out.push(format!("… {hidden} lines omitted …"));
                hidden = 0;
            }
            out.push(line.clone());
        } else {
            hidden += groups[i].count;
        }
    }
    if hidden > 0 {
        out.push(format!("… {hidden} lines omitted …"));
    }
    out.join("\n")
}

/// Keeps only the last frame of `\r` progress bars, strips ANSI colors and trailing spaces.
fn clean(raw: &str) -> String {
    let last_frame = raw.trim_end_matches('\r').rsplit('\r').next().unwrap_or("");
    let mut out = String::with_capacity(last_frame.len());
    let mut chars = last_frame.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ponytail: handles CSI sequences (colors, cursor moves), the only kind common in logs.
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out.truncate(out.trim_end().len());
    out
}

/// Replaces every alphanumeric run that contains a digit with `#`, so numbers,
/// hashes, UUIDs, timestamps and ports all compare equal.
fn template(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut word = String::new();
    for c in line.chars().chain(std::iter::once(' ')) {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        if word.chars().any(|w| w.is_ascii_digit()) {
            out.push('#');
        } else {
            out.push_str(&word);
        }
        word.clear();
        out.push(c);
    }
    out.pop();
    out
}

/// Whole-word match, so `TypeError:` and `npm ERR!` count but a package named `http-errors` doesn't.
fn is_important(line: &str) -> bool {
    line.split(|c: char| !c.is_alphanumeric()).any(|w| {
        let w = w.to_lowercase();
        IMPORTANT.contains(&w.as_str()) || w.ends_with("error") || w.ends_with("exception")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_and_keeps_last_progress_frame() {
        let (out, _) = compress(
            "\x1b[32mok\x1b[0m done  \nDownloading 10%\rDownloading 55%\rDownloading 100%\r\n",
            1000,
        );
        assert_eq!(out, "ok done\nDownloading 100%");
    }

    #[test]
    fn collapses_identical_and_similar_runs() {
        let input = "start\nretrying\nretrying\nretrying\nGET /users/17 200 3ms\nGET /users/42 200 5ms\nGET /users/a1b2c3 200 1ms\n\n\n\nend";
        let (out, stats) = compress(input, 1000);
        assert_eq!(
            out,
            "start\nretrying  [repeated ×3]\nGET /users/17 200 3ms  [+2 similar]\n\nend"
        );
        assert_eq!((stats.input_lines, stats.output_lines), (11, 5));
    }

    #[test]
    fn different_words_are_not_merged() {
        let (out, _) = compress(
            "Compiling serde v1.0.1\nCompiling tokio v1.4.0\nFinished in 3s",
            1000,
        );
        assert_eq!(out.lines().count(), 3);
    }

    #[test]
    fn over_budget_keeps_errors_head_and_tail() {
        let mut lines: Vec<String> = (0..2000)
            .map(|i| format!("step {i}: {} work", "x".repeat(i % 7)))
            .collect();
        // Break up the similar-line runs so plain collapsing can't save the budget.
        for (i, line) in lines.iter_mut().enumerate() {
            line.push_str(["", " alpha", " beta"][i % 3]);
        }
        lines[1000] = "thread 'main' panicked at src/db.rs:42: connection refused".into();
        let input = lines.join("\n");

        let (out, stats) = compress(&input, 300);
        assert!(stats.output_tokens <= 300, "{} tokens", stats.output_tokens);
        assert!(out.contains("panicked at src/db.rs:42"));
        assert!(out.contains("step 999"), "context before the error");
        assert!(out.starts_with("step 0"));
        assert!(out.contains("step 1999"));
        assert!(out.contains("lines omitted"));
    }

    #[test]
    fn important_lines_are_whole_words() {
        for line in [
            "TypeError: x is undefined",
            "npm ERR! code 1",
            "test foo ... FAILED",
            "warning: unused import",
            "java.lang.NullPointerException",
        ] {
            assert!(is_important(line), "{line}");
        }
        for line in [
            "'http-errors': [ '2.0.1' ]",
            "'es-errors': [ '1.3.0' ]",
            "terrible weather",
            "Interrupt handler",
        ] {
            assert!(!is_important(line), "{line}");
        }
    }

    #[test]
    fn omitted_counts_add_up() {
        let input: String = (0..500)
            .map(|i| format!("line {i} {}\n", ["a", "b"][i % 2]))
            .collect();
        let (out, _) = compress(&input, 100);
        let shown = out.lines().filter(|l| !l.contains("omitted")).count();
        let hidden: usize = out
            .lines()
            .filter_map(|l| {
                l.strip_prefix("… ")?
                    .split(' ')
                    .next()?
                    .parse::<usize>()
                    .ok()
            })
            .sum();
        assert_eq!(shown + hidden, 500);
    }

    #[test]
    fn empty_input() {
        assert_eq!(
            compress("", 100),
            (
                String::new(),
                Stats {
                    input_lines: 0,
                    input_tokens: 0,
                    output_lines: 0,
                    output_tokens: 0
                }
            )
        );
    }
}
