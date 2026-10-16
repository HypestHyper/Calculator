import math
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from calculator.engine import CalcError, evaluate, format_number, pretty, raw_number  # noqa: E402


def ev(text, **kw):
    return evaluate(text, **kw)


class EngineTests(unittest.TestCase):
    def test_precedence(self):
        self.assertEqual(ev("2 + 3 × 4"), 14)
        self.assertEqual(ev("(2 + 3) × 4"), 20)
        self.assertEqual(ev("10 − 4 − 3"), 3)
        self.assertEqual(ev("2^3^2"), 512)
        self.assertEqual(ev("−2^2"), -4)

    def test_implicit_multiplication(self):
        self.assertAlmostEqual(ev("2π"), 2 * math.pi)
        self.assertEqual(ev("3(4)"), 12)
        self.assertEqual(ev("(1+2)(3)"), 9)
        self.assertAlmostEqual(ev("2sin(30)"), 1.0)

    def test_percent(self):
        self.assertEqual(ev("50%"), 0.5)
        self.assertEqual(ev("200 + 10%"), 220)
        self.assertEqual(ev("200 − 10%"), 180)
        self.assertEqual(ev("200 × 10%"), 20)

    def test_scientific(self):
        self.assertAlmostEqual(ev("sin(30)"), 0.5)
        self.assertEqual(ev("sin(180)"), 0.0)
        self.assertAlmostEqual(ev("sin(π/2)", degrees=False), 1.0)
        self.assertAlmostEqual(ev("asin(1)"), 90.0)
        self.assertEqual(ev("5!"), 120)
        self.assertEqual(ev("√(16)"), 4)
        self.assertEqual(ev("∛(−27)"), -3)
        self.assertEqual(ev("4²"), 16)
        self.assertEqual(ev("2³"), 8)
        self.assertEqual(ev("4⁻¹"), 0.25)
        self.assertAlmostEqual(ev("ln(e)"), 1.0)
        self.assertEqual(ev("log(1000)"), 3)
        self.assertEqual(ev("10 mod 4"), 2)
        self.assertEqual(ev("abs(−3)"), 3)
        self.assertEqual(ev("1.5E3"), 1500)
        self.assertEqual(ev("Ans × 2", ans=21), 42)

    def test_auto_close_parens(self):
        self.assertAlmostEqual(ev("sin(30"), 0.5)
        self.assertEqual(ev("(−5"), -5)

    def test_errors(self):
        for bad in ("1/0", "√(−1)", "ln(0)", "2.5!", "tan(90)", "(−8)^0.5", "", "2+", "1)"):
            with self.assertRaises(CalcError, msg=bad):
                ev(bad)
        for bad in ("tan(π/2)", "tan(−3π/2)"):
            with self.assertRaises(CalcError, msg=bad):
                ev(bad, degrees=False)
        self.assertAlmostEqual(ev("tan(π/4)", degrees=False), 1)
        with self.assertRaises(CalcError):
            ev("tan(270)")

    def test_standard_mode_rejects_scientific(self):
        for text in ("2^3", "sin(30)", "5!", "π"):
            with self.assertRaises(CalcError):
                ev(text, scientific=False)
        self.assertEqual(ev("2 + 2", scientific=False), 4)

    def test_formatting(self):
        self.assertEqual(format_number(0.1 + 0.2), "0.3")
        self.assertEqual(format_number(1234567.5), "1 234 567.5")
        self.assertEqual(format_number(-1234, grouping=False), "−1234")
        self.assertEqual(format_number(2.0, decimals=3), "2.000")
        self.assertEqual(format_number(1e20), "1E20")
        self.assertEqual(format_number(-0.0), "0")
        self.assertEqual(format_number(-0.001, decimals=2), "0.00")
        self.assertEqual(format_number(-0.5, decimals=0), "0")
        self.assertEqual(format_number(1.5e-9), "1.5E−9")
        self.assertEqual(ev(raw_number(1.5e-9)), 1.5e-9)
        self.assertEqual(ev(raw_number(-2.5)), -2.5)
        self.assertEqual(pretty("12345+6.78901"), "12 345+6.78901")


if __name__ == "__main__":
    unittest.main()
