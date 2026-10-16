import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from calculator.logic import Calculator  # noqa: E402


def run(keys, scientific=True):
    calc = Calculator()
    calc.scientific = scientific
    status = None
    for key in keys:
        status = calc.press(key)
    return calc, status


def digits(text):
    return [f"digit:{c}" if c.isdigit() else ("dot:" if c == "." else c) for c in text]


class LogicTests(unittest.TestCase):
    def test_basic(self):
        calc, status = run(digits("12") + ["op:+"] + digits("3") + ["eq:"])
        self.assertEqual(status, "result")
        self.assertEqual(calc.display(), "15")
        self.assertEqual(calc.prev_line, "12+3")

    def test_repeat_equals(self):
        calc, _ = run(digits("2") + ["op:+"] + digits("3") + ["eq:", "eq:", "eq:"])
        self.assertEqual(calc.display(), "11")

    def test_continue_from_result(self):
        calc, _ = run(digits("6") + ["op:×"] + digits("7") + ["eq:", "op:÷"] + digits("2") + ["eq:"])
        self.assertEqual(calc.display(), "21")

    def test_new_number_after_result(self):
        calc, _ = run(digits("1") + ["op:+", "digit:1", "eq:", "digit:5"])
        self.assertEqual(calc.expr, "5")

    def test_operator_replacement(self):
        calc, _ = run(digits("5") + ["op:+", "op:×"])
        self.assertEqual(calc.expr, "5×")
        calc.press("op:−")
        self.assertEqual(calc.expr, "5×−")
        calc.press("op:+")
        self.assertEqual(calc.expr, "5+")

    def test_decimal(self):
        calc, _ = run(["dot:", "digit:5", "dot:", "digit:2"])
        self.assertEqual(calc.expr, "0.52")

    def test_leading_zero(self):
        calc, _ = run(["digit:0", "digit:0", "digit:7"])
        self.assertEqual(calc.expr, "7")

    def test_negate(self):
        calc, _ = run(digits("5") + ["neg:"])
        self.assertEqual(calc.expr, "(−5")
        calc.press("neg:")
        self.assertEqual(calc.expr, "5")
        calc, _ = run(digits("3") + ["op:×"] + digits("4") + ["neg:", "eq:"])
        self.assertEqual(calc.display(), "−12")

    def test_backspace(self):
        calc, _ = run(digits("2") + ["func:sin(", "back:"])
        self.assertEqual(calc.expr, "2")

    def test_preview_and_errors(self):
        calc, _ = run(digits("8") + ["op:÷"] + digits("2"))
        self.assertEqual(calc.preview(), "4")
        calc, status = run(digits("8") + ["op:÷"] + digits("0") + ["eq:"])
        self.assertEqual(status, "error")
        self.assertTrue(calc.error)

    def test_scientific_keys(self):
        calc, _ = run(["func:sin(", "digit:3", "digit:0", "close:", "eq:"])
        self.assertEqual(calc.display(), "0.5")
        calc, _ = run(digits("3") + ["post:²", "eq:"])
        self.assertEqual(calc.display(), "9")
        calc, _ = run(digits("2") + ["exp:", "op:−", "digit:3", "eq:"])
        self.assertEqual(calc.display(), "0.002")

    def test_percent(self):
        calc, _ = run(digits("200") + ["op:+"] + digits("10") + ["pct:", "eq:"])
        self.assertEqual(calc.display(), "220")

    def test_paste_and_use_value(self):
        calc = Calculator()
        calc.scientific = True
        calc.paste("2*3 - 1")
        calc.press("eq:")
        self.assertEqual(calc.display(), "5")
        calc.use_value(4)
        self.assertEqual(calc.expr, "4")

    def test_repeat_equals_tail(self):
        # The "−" in E notation is not a subtraction to repeat.
        calc, _ = run(digits("2") + ["exp:", "op:−", "digit:5", "eq:", "eq:"])
        self.assertEqual(calc.ans, 2e-5)
        calc, _ = run(digits("3") + ["op:+", "digit:1", "exp:", "op:−", "digit:5", "eq:", "eq:"])
        self.assertAlmostEqual(calc.ans, 3.00002)
        # Multiplying by a negative number keeps the "×".
        calc, _ = run(digits("5") + ["op:×", "op:−", "digit:3", "eq:", "eq:"])
        self.assertEqual(calc.ans, 45)
        calc, _ = run(digits("5") + ["op:×", "digit:3", "neg:", "eq:", "eq:"])
        self.assertEqual(calc.ans, 45)
        # A leading minus is not an operation.
        calc, _ = run(["op:−", "digit:5", "eq:", "eq:"])
        self.assertEqual(calc.ans, -5)

    def test_result_keeps_full_precision(self):
        calc, _ = run(digits("1") + ["op:÷", "digit:3", "eq:", "op:×", "digit:3", "eq:"])
        self.assertEqual(calc.ans, 1)
        calc, _ = run(digits("1") + ["op:÷", "digit:3", "eq:", "eq:"])
        self.assertEqual(calc.last_tail, "÷3")
        calc, _ = run(digits("1") + ["op:÷", "digit:3", "eq:", "op:×", "digit:3"])
        self.assertEqual(calc.preview(), "1")
        # Typing onto the rounded text uses the text as written.
        calc, _ = run(digits("1") + ["op:÷", "digit:3", "eq:", "back:", "digit:7", "eq:"])
        self.assertEqual(calc.ans, 0.3333333333333337)

    def test_paste_names_and_commas(self):
        cases = {
            "exp(1)": "exp(1)",
            "2x3": "2×3",
            "2xsin(30)": "2×sin(30)",
            "1,000+1": "1000+1",
            "1,234.5": "1234.5",
            "1,5": "1.5",
            "Ans+1": "Ans+1",
            "7 mod 4": "7 mod 4",
        }
        for text, expr in cases.items():
            calc = Calculator()
            calc.scientific = True
            self.assertEqual(calc.paste(text), "ok", text)
            self.assertEqual(calc.expr, expr)
        calc = Calculator()
        calc.scientific = True
        calc.paste("exp(")
        calc.press("back:")
        self.assertEqual(calc.expr, "")
        for text, value in (("exp(0)", "1"), ("7 mod 4", "3"), ("1,000+1", "1\u2009001")):
            calc = Calculator()
            calc.scientific = True
            calc.paste(text)
            calc.press("eq:")
            self.assertEqual(calc.display(), value, text)


if __name__ == "__main__":
    unittest.main()
