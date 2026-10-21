#!/usr/bin/env python3
"""Compare the Rust port with the Python app on random inputs.

Generates expressions, numbers to format and key sequences, answers them with
the Python code in calculator/, asks the Rust port (examples/parity.rs) the
same, and prints every case where the two differ.

    python3 rust/scripts/parity.py [count] [seed]
"""

import random
import struct
import subprocess
import sys
from pathlib import Path

RUST = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RUST.parent))

from calculator.engine import CalcError, evaluate, format_number, pretty, raw_number  # noqa: E402
from calculator.logic import Calculator  # noqa: E402


def bits(value):
    return str(struct.unpack("<Q", struct.pack("<d", value))[0])


def from_bits(text):
    return struct.unpack("<d", struct.pack("<Q", int(text)))[0]


EXPR_PARTS = [
    "0", "1", "2", "3", "7", "9", "10", "12", "45", "90", "180", "1000", "123456", "0.5", ".5", "5.",
    "1.5", "0.1", "1E3", "2E−5", "1E400", "1E+2", "+", "−", "-", "×", "*", "÷", "/", "^", "%", "!",
    "(", ")", "π", "e", "pi", "sin(", "cos(", "tan(", "asin(", "acos(", "atan(", "log(", "ln(",
    "√(", "sqrt(", "∛(", "abs(", "exp(", "Ans", " mod ", "²", "³", "⁻¹", ",", " ", " ", "x",
    "foo", "E", "@", "1,000",
]

KEYS = (
    [f"digit:{d}" for d in "0123456789"] * 3
    + ["dot:"] * 3
    + ["op:+", "op:−", "op:×", "op:÷", "op:^", "op: mod "] * 2
    + ["pct:", "neg:", "clear:", "back:", "back:", "open:", "close:", "close:", "exp:"]
    + ["eq:"] * 4
    + ["func:sin(", "func:cos(", "func:tan(", "func:asin(", "func:ln(", "func:log(",
       "func:√(", "func:∛(", "func:abs(", "func:e^(", "func:10^("]
    + ["post:²", "post:³", "post:!", "post:⁻¹", "const:π", "const:e", "const:Ans", "bogus:1"]
)

PASTES = ["2*3 - 1", "exp(1)", "1,000+1", "1,5", "1.234,5", "3.14", "2x3", "Ans+1", "7 mod 4",
          "hello!", "", "  ", "−5", "1e5", "sin(30)", "12,345.678"]


def random_value(rng):
    roll = rng.random()
    if roll < 0.15:
        return rng.choice([0.0, -0.0, 0.5, -0.5, 1.5, 2.5, 0.125, 1.005, 1e-7, 9.9999999999995e11,
                           999999999999.95, 1e12, 1e15, 1 / 3, 2 / 3, 0.1 + 0.2, 1e300, 5e-324])
    if roll < 0.25:
        return float(rng.randint(-10**6, 10**6))
    return rng.choice([1, -1]) * rng.random() * 10 ** rng.randint(-12, 20)


def make_cases(count, rng):
    cases = []
    for _ in range(count):
        kind = rng.random()
        if kind < 0.4:
            expr = "".join(rng.choice(EXPR_PARTS) for _ in range(rng.randint(1, 10)))
            ans = rng.choice([0.0, 21.0, -2.5, 1 / 3, 1e20])
            cases.append(("eval", str(rng.randint(0, 1)), str(rng.randint(0, 1)), bits(ans), expr))
        elif kind < 0.6:
            decimals = rng.choice(["-", "0", "2", "5", "10"])
            cases.append(("fmt", bits(random_value(rng)), decimals, str(rng.randint(0, 1)),
                          str(rng.choice([12, 15])), str(rng.randint(0, 1))))
        elif kind < 0.65:
            text = "".join(rng.choice(["1", "23", "4567", "89012", ".", "E", "+", "−", "(", "5."])
                           for _ in range(rng.randint(1, 8)))
            cases.append(("pretty", text))
        else:
            keys = []
            for _ in range(rng.randint(1, 18)):
                roll = rng.random()
                if roll < 0.05:
                    keys.append("paste=" + rng.choice(PASTES))
                elif roll < 0.08:
                    keys.append("use=" + bits(random_value(rng)))
                else:
                    keys.append(rng.choice(KEYS))
            cases.append(("keys", str(rng.randint(0, 1)), rng.choice(".,"), *keys))
    return cases


def python_answer(case):
    kind = case[0]
    if kind == "eval":
        _, scientific, degrees, ans, expr = case
        try:
            value = evaluate(expr, scientific=scientific == "1", degrees=degrees == "1",
                             ans=from_bits(ans))
            return f"ok {bits(value)}"
        except CalcError as err:
            return f"err {err}"
    if kind == "fmt":
        _, value, decimals, grouping, max_digits, pretty_minus = case
        v = from_bits(value)
        text = format_number(v, decimals=None if decimals == "-" else int(decimals),
                             grouping=grouping == "1", max_digits=int(max_digits),
                             pretty_minus=pretty_minus == "1")
        return f"{text}|{raw_number(v)}"
    if kind == "pretty":
        return f"{pretty(case[1], True)}|{pretty(case[1], False)}"
    calc = Calculator()
    calc.scientific = case[1] == "1"
    calc.decimal = case[2]
    statuses = []
    for key in case[3:]:
        if key.startswith("paste="):
            statuses.append(calc.paste(key[6:]))
        elif key.startswith("use="):
            statuses.append(calc.use_value(from_bits(key[4:])))
        else:
            statuses.append(calc.press(key))
    return "|".join([
        ",".join(statuses), calc.expr, bits(calc.ans), str(calc.evaluated).lower(),
        calc.prev_line, calc.error or "-", calc.last_tail or "-", calc.display(),
        calc.preview(3, False), calc.copy_text(),
    ])


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 20000
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    cases = make_cases(count, random.Random(seed))
    subprocess.run(["cargo", "build", "-q", "--release", "--example", "parity"], cwd=RUST, check=True)
    binary = RUST / "target" / "release" / "examples" / "parity"
    stdin = "".join("\t".join(case) + "\n" for case in cases)
    rust = subprocess.run([binary], input=stdin, capture_output=True, text=True, check=True)
    rust_lines = rust.stdout.splitlines()
    assert len(rust_lines) == len(cases), (len(rust_lines), len(cases))

    differences = 0
    by_kind = {}
    for case, rust_answer in zip(cases, rust_lines):
        expected = python_answer(case)
        by_kind[case[0]] = by_kind.get(case[0], 0) + 1
        if expected != rust_answer:
            differences += 1
            if differences <= 25:
                print("case:  ", "\t".join(case).replace(" ", "<thin>"))
                print("python:", expected.replace(" ", "<thin>"))
                print("rust:  ", rust_answer.replace(" ", "<thin>"))
                print()
    summary = ", ".join(f"{n} {kind}" for kind, n in sorted(by_kind.items()))
    print(f"{len(cases)} cases ({summary}), seed {seed}: {differences} differ")
    sys.exit(1 if differences else 0)


if __name__ == "__main__":
    main()
