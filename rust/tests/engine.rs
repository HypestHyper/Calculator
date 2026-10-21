//! Port of tests/test_engine.py.

use calculator::engine::{evaluate, format_number, pretty, raw_number, Format, Options};
use std::f64::consts::PI;

fn ev(text: &str) -> f64 {
    evaluate(text, &Options::default()).unwrap_or_else(|e| panic!("{text}: {e}"))
}

fn ev_with(text: &str, options: Options) -> f64 {
    evaluate(text, &options).unwrap_or_else(|e| panic!("{text}: {e}"))
}

fn radians() -> Options {
    Options {
        degrees: false,
        ..Options::default()
    }
}

/// Python's assertAlmostEqual: equal to 7 decimal places.
fn assert_close(a: f64, b: f64) {
    assert!((a - b).abs() < 5e-8, "{a} != {b}");
}

fn fmt(value: f64) -> String {
    format_number(value, &Format::default())
}

#[test]
fn precedence() {
    assert_eq!(ev("2 + 3 × 4"), 14.0);
    assert_eq!(ev("(2 + 3) × 4"), 20.0);
    assert_eq!(ev("10 − 4 − 3"), 3.0);
    assert_eq!(ev("2^3^2"), 512.0);
    assert_eq!(ev("−2^2"), -4.0);
}

#[test]
fn implicit_multiplication() {
    assert_close(ev("2π"), 2.0 * PI);
    assert_eq!(ev("3(4)"), 12.0);
    assert_eq!(ev("(1+2)(3)"), 9.0);
    assert_close(ev("2sin(30)"), 1.0);
}

#[test]
fn percent() {
    assert_eq!(ev("50%"), 0.5);
    assert_eq!(ev("200 + 10%"), 220.0);
    assert_eq!(ev("200 − 10%"), 180.0);
    assert_eq!(ev("200 × 10%"), 20.0);
}

#[test]
fn scientific() {
    assert_close(ev("sin(30)"), 0.5);
    assert_eq!(ev("sin(180)"), 0.0);
    assert_close(ev_with("sin(π/2)", radians()), 1.0);
    assert_close(ev("asin(1)"), 90.0);
    assert_eq!(ev("5!"), 120.0);
    assert_eq!(ev("√(16)"), 4.0);
    assert_eq!(ev("∛(−27)"), -3.0);
    assert_eq!(ev("4²"), 16.0);
    assert_eq!(ev("2³"), 8.0);
    assert_eq!(ev("4⁻¹"), 0.25);
    assert_close(ev("ln(e)"), 1.0);
    assert_eq!(ev("log(1000)"), 3.0);
    assert_eq!(ev("10 mod 4"), 2.0);
    assert_eq!(ev("abs(−3)"), 3.0);
    assert_eq!(ev("1.5E3"), 1500.0);
    assert_eq!(
        ev_with(
            "Ans × 2",
            Options {
                ans: 21.0,
                ..Options::default()
            }
        ),
        42.0
    );
}

#[test]
fn auto_close_parens() {
    assert_close(ev("sin(30"), 0.5);
    assert_eq!(ev("(−5"), -5.0);
}

#[test]
fn errors() {
    for bad in [
        "1/0",
        "√(−1)",
        "ln(0)",
        "2.5!",
        "tan(90)",
        "(−8)^0.5",
        "",
        "2+",
        "1)",
    ] {
        assert!(
            evaluate(bad, &Options::default()).is_err(),
            "{bad} should fail"
        );
    }
    for bad in ["tan(π/2)", "tan(−3π/2)"] {
        assert!(evaluate(bad, &radians()).is_err(), "{bad} should fail");
    }
    assert_close(ev_with("tan(π/4)", radians()), 1.0);
    assert!(evaluate("tan(270)", &Options::default()).is_err());
}

#[test]
fn standard_mode_rejects_scientific() {
    let standard = Options {
        scientific: false,
        ..Options::default()
    };
    for text in ["2^3", "sin(30)", "5!", "π"] {
        assert!(evaluate(text, &standard).is_err(), "{text} should fail");
    }
    assert_eq!(ev_with("2 + 2", standard), 4.0);
}

#[test]
fn formatting() {
    assert_eq!(fmt(0.1 + 0.2), "0.3");
    assert_eq!(fmt(1234567.5), "1\u{2009}234\u{2009}567.5");
    assert_eq!(
        format_number(
            -1234.0,
            &Format {
                grouping: false,
                ..Format::default()
            }
        ),
        "−1234"
    );
    assert_eq!(
        format_number(
            2.0,
            &Format {
                decimals: Some(3),
                ..Format::default()
            }
        ),
        "2.000"
    );
    assert_eq!(fmt(1e20), "1E20");
    assert_eq!(fmt(-0.0), "0");
    assert_eq!(
        format_number(
            -0.001,
            &Format {
                decimals: Some(2),
                ..Format::default()
            }
        ),
        "0.00"
    );
    assert_eq!(
        format_number(
            -0.5,
            &Format {
                decimals: Some(0),
                ..Format::default()
            }
        ),
        "0"
    );
    assert_eq!(fmt(1.5e-9), "1.5E−9");
    assert_eq!(ev(&raw_number(1.5e-9)), 1.5e-9);
    assert_eq!(ev(&raw_number(-2.5)), -2.5);
    assert_eq!(pretty("12345+6.78901", true), "12\u{2009}345+6.78901");
}
