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


if __name__ == "__main__":
    unittest.main()
