//! Port of tests/test_logic.py.

use calculator::logic::{Calculator, Status};

fn run(keys: &[&str]) -> (Calculator, Status) {
    let mut calc = Calculator {
        scientific: true,
        ..Calculator::default()
    };
    let mut status = Status::Ignored;
    for key in keys {
        status = calc.press(key);
    }
    (calc, status)
}

/// Keys for typing `text`: digits and "." become digit and dot keys, anything
/// else is passed through as a key name.
fn keys(parts: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for part in parts {
        if part.contains(':') {
            out.push(part.to_string());
            continue;
        }
        for c in part.chars() {
            out.push(if c == '.' {
                "dot:".into()
            } else {
                format!("digit:{c}")
            });
        }
    }
    out
}

fn run_keys(parts: &[&str]) -> (Calculator, Status) {
    let keys = keys(parts);
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    run(&refs)
}

fn display(calc: &Calculator) -> String {
    calc.display(None, true)
}

fn sci() -> Calculator {
    Calculator {
        scientific: true,
        ..Calculator::default()
    }
}

#[test]
fn basic() {
    let (calc, status) = run_keys(&["12", "op:+", "3", "eq:"]);
    assert_eq!(status, Status::Result);
    assert_eq!(display(&calc), "15");
    assert_eq!(calc.prev_line, "12+3");
}

#[test]
fn repeat_equals() {
    let (calc, _) = run_keys(&["2", "op:+", "3", "eq:", "eq:", "eq:"]);
    assert_eq!(display(&calc), "11");
}

#[test]
fn continue_from_result() {
    let (calc, _) = run_keys(&["6", "op:×", "7", "eq:", "op:÷", "2", "eq:"]);
    assert_eq!(display(&calc), "21");
}

#[test]
fn new_number_after_result() {
    let (calc, _) = run_keys(&["1", "op:+", "1", "eq:", "5"]);
    assert_eq!(calc.expr, "5");
}

#[test]
fn operator_replacement() {
    let (mut calc, _) = run_keys(&["5", "op:+", "op:×"]);
    assert_eq!(calc.expr, "5×");
    calc.press("op:−");
    assert_eq!(calc.expr, "5×−");
    calc.press("op:+");
    assert_eq!(calc.expr, "5+");
}

#[test]
fn decimal() {
    let (calc, _) = run(&["dot:", "digit:5", "dot:", "digit:2"]);
    assert_eq!(calc.expr, "0.52");
}

#[test]
fn leading_zero() {
    let (calc, _) = run(&["digit:0", "digit:0", "digit:7"]);
    assert_eq!(calc.expr, "7");
}

#[test]
fn negate() {
    let (mut calc, _) = run_keys(&["5", "neg:"]);
    assert_eq!(calc.expr, "(−5");
    calc.press("neg:");
    assert_eq!(calc.expr, "5");
    let (calc, _) = run_keys(&["3", "op:×", "4", "neg:", "eq:"]);
    assert_eq!(display(&calc), "−12");
}

#[test]
fn backspace() {
    let (calc, _) = run_keys(&["2", "func:sin(", "back:"]);
    assert_eq!(calc.expr, "2");
}

#[test]
fn preview_and_errors() {
    let (calc, _) = run_keys(&["8", "op:÷", "2"]);
    assert_eq!(calc.preview(None, true), "4");
    let (calc, status) = run_keys(&["8", "op:÷", "0", "eq:"]);
    assert_eq!(status, Status::Error);
    assert!(calc.error.is_some());
}

#[test]
fn scientific_keys() {
    let (calc, _) = run(&["func:sin(", "digit:3", "digit:0", "close:", "eq:"]);
    assert_eq!(display(&calc), "0.5");
    let (calc, _) = run_keys(&["3", "post:²", "eq:"]);
    assert_eq!(display(&calc), "9");
    let (calc, _) = run_keys(&["2", "exp:", "op:−", "3", "eq:"]);
    assert_eq!(display(&calc), "0.002");
}

#[test]
fn percent() {
    let (calc, _) = run_keys(&["200", "op:+", "10", "pct:", "eq:"]);
    assert_eq!(display(&calc), "220");
}

#[test]
fn paste_and_use_value() {
    let mut calc = sci();
    calc.paste("2*3 - 1");
    calc.press("eq:");
    assert_eq!(display(&calc), "5");
    calc.use_value(4.0);
    assert_eq!(calc.expr, "4");
}

#[test]
fn repeat_equals_tail() {
    // The "−" in E notation is not a subtraction to repeat.
    let (calc, _) = run_keys(&["2", "exp:", "op:−", "5", "eq:", "eq:"]);
    assert_eq!(calc.ans, 2e-5);
    let (calc, _) = run_keys(&["3", "op:+", "1", "exp:", "op:−", "5", "eq:", "eq:"]);
    assert!((calc.ans - 3.00002).abs() < 5e-8);
    // Multiplying by a negative number keeps the "×".
    let (calc, _) = run_keys(&["5", "op:×", "op:−", "3", "eq:", "eq:"]);
    assert_eq!(calc.ans, 45.0);
    let (calc, _) = run_keys(&["5", "op:×", "3", "neg:", "eq:", "eq:"]);
    assert_eq!(calc.ans, 45.0);
    // A leading minus is not an operation.
    let (calc, _) = run_keys(&["op:−", "5", "eq:", "eq:"]);
    assert_eq!(calc.ans, -5.0);
}

#[test]
fn result_keeps_full_precision() {
    let (calc, _) = run_keys(&["1", "op:÷", "3", "eq:", "op:×", "3", "eq:"]);
    assert_eq!(calc.ans, 1.0);
    let (calc, _) = run_keys(&["1", "op:÷", "3", "eq:", "eq:"]);
    assert_eq!(calc.last_tail.as_deref(), Some("÷3"));
    let (calc, _) = run_keys(&["1", "op:÷", "3", "eq:", "op:×", "3"]);
    assert_eq!(calc.preview(None, true), "1");
    // Typing onto the rounded text uses the text as written.
    let (calc, _) = run_keys(&["1", "op:÷", "3", "eq:", "back:", "7", "eq:"]);
    assert_eq!(calc.ans, 0.3333333333333337);
}

#[test]
fn paste_names_and_commas() {
    let cases = [
        ("exp(1)", "exp(1)"),
        ("2x3", "2×3"),
        ("2xsin(30)", "2×sin(30)"),
        ("1,000+1", "1000+1"),
        ("1,234.5", "1234.5"),
        ("1,5", "1.5"),
        ("Ans+1", "Ans+1"),
        ("7 mod 4", "7 mod 4"),
    ];
    for (text, expr) in cases {
        let mut calc = sci();
        assert_eq!(calc.paste(text), Status::Ok, "{text}");
        assert_eq!(calc.expr, expr);
    }
    let mut calc = sci();
    calc.paste("exp(");
    calc.press("back:");
    assert_eq!(calc.expr, "");
    for (text, value) in [
        ("exp(0)", "1"),
        ("7 mod 4", "3"),
        ("1,000+1", "1\u{2009}001"),
    ] {
        let mut calc = sci();
        calc.paste(text);
        calc.press("eq:");
        assert_eq!(display(&calc), value, "{text}");
    }
}

#[test]
fn decimal_comma() {
    let (mut calc, _) = run_keys(&["1.5", "op:+", "2"]);
    calc.decimal = ',';
    assert_eq!(display(&calc), "1,5+2");
    assert_eq!(calc.preview(None, true), "3,5");
    calc.press("eq:");
    assert_eq!(display(&calc), "3,5");
    assert_eq!(calc.copy_text(None), "3,5");
    assert_eq!(calc.expr, "3.5"); // internal form is unchanged
}

#[test]
fn paste_follows_decimal_symbol() {
    let cases: [(char, &[(&str, &str)]); 2] = [
        (
            '.',
            &[
                ("1,5", "1.5"),
                ("1,000", "1000"),
                ("1,234.5", "1234.5"),
                ("3.14", "3.14"),
            ],
        ),
        (
            ',',
            &[
                ("1,5", "1.5"),
                ("1.000", "1000"),
                ("1.234,5", "1234.5"),
                ("3.14", "3.14"),
                ("1,000", "1.000"),
            ],
        ),
    ];
    for (decimal, pastes) in cases {
        for (text, expr) in pastes {
            let mut calc = Calculator {
                decimal,
                ..Calculator::default()
            };
            calc.paste(text);
            assert_eq!(calc.expr, *expr, "{decimal} {text}");
        }
    }
}
