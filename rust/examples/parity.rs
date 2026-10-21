//! Reads test cases on stdin and prints what the Rust port does with them, one
//! line each, in the same format as scripts/parity.py prints for the Python
//! version. Used by scripts/parity.py to compare the two.
//!
//! Cases, tab-separated:
//!   eval  <scientific 0|1> <degrees 0|1> <ans bits> <expression>
//!   fmt   <value bits> <decimals or -> <grouping 0|1> <max digits> <pretty minus 0|1>
//!   pretty <text>
//!   keys  <scientific 0|1> <decimal> <key> <key> ...   (key "paste=<text>" or "use=<bits>")

use std::io::{self, BufRead, Write};

use calculator::engine::{evaluate, format_number, pretty, raw_number, Format, Options};
use calculator::logic::{Calculator, Status};

fn bits(text: &str) -> f64 {
    f64::from_bits(text.parse().unwrap())
}

fn flag(text: &str) -> bool {
    text == "1"
}

fn result(value: Result<f64, calculator::engine::CalcError>) -> String {
    match value {
        Ok(v) => format!("ok {}", v.to_bits()),
        Err(e) => format!("err {e}"),
    }
}

fn status(s: Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::Result => "result",
        Status::Error => "error",
        Status::Ignored => "ignored",
    }
}

fn main() {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let f: Vec<&str> = line.split('\t').collect();
        let answer = match f[0] {
            "eval" => {
                let options = Options {
                    scientific: flag(f[1]),
                    degrees: flag(f[2]),
                    ans: bits(f[3]),
                };
                result(evaluate(f[4], &options))
            }
            "fmt" => {
                let format = Format {
                    decimals: f[2].parse().ok(),
                    grouping: flag(f[3]),
                    max_digits: f[4].parse().unwrap(),
                    pretty_minus: flag(f[5]),
                };
                let v = bits(f[1]);
                format!("{}|{}", format_number(v, &format), raw_number(v))
            }
            "pretty" => format!("{}|{}", pretty(f[1], true), pretty(f[1], false)),
            "keys" => {
                let mut calc = Calculator {
                    scientific: flag(f[1]),
                    decimal: f[2].chars().next().unwrap(),
                    ..Calculator::default()
                };
                let mut statuses = Vec::new();
                for key in &f[3..] {
                    let s = if let Some(text) = key.strip_prefix("paste=") {
                        calc.paste(text)
                    } else if let Some(v) = key.strip_prefix("use=") {
                        calc.use_value(bits(v))
                    } else {
                        calc.press(key)
                    };
                    statuses.push(status(s));
                }
                format!(
                    "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                    statuses.join(","),
                    calc.expr,
                    calc.ans.to_bits(),
                    calc.evaluated,
                    calc.prev_line,
                    calc.error.as_deref().unwrap_or("-"),
                    calc.last_tail.as_deref().unwrap_or("-"),
                    calc.display(None, true),
                    calc.preview(Some(3), false),
                    calc.copy_text(None),
                )
            }
            other => panic!("unknown case {other}"),
        };
        writeln!(out, "{answer}").unwrap();
    }
}
