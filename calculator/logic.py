"""Input handling for the calculator: what each key does to the expression.

Pure Python with no GTK, so it can be unit-tested.
"""

import re

from .engine import CalcError, evaluate, format_number, localize, pretty, raw_number

BINARY_OPS = ("+", "−", "×", "÷", "^", " mod ")
# Tokens removed in one backspace.
MULTI_TOKENS = sorted(
    ["asin(", "acos(", "atan(", "sin(", "cos(", "tan(", "log(", "ln(", "abs(", "exp(",
     "√(", "∛(", "Ans", "⁻¹", " mod "],
    key=len, reverse=True)

_ENDS_OPERAND = re.compile(r"(?:[\d.)πe!%²³]|⁻¹|Ans)$")
_CURRENT_NUMBER = re.compile(r"[\d.]*(?:E−?\d*)?$")
_NUMBER = r"(?:\d+\.?\d*|\.\d+)(?:E−?\d+)?"
# The last binary operation, repeated by pressing "=" again. The operator must
# follow an operand, so the "−" in "2E−5" or a leading "−5" is not matched, and
# the operand may be negative: "5×−3" and "5×(−3)" repeat "×−3" and "×(−3)".
_TAIL = re.compile(rf"(?<=[\d.)πe!%²³¹s])(\+|−|×|÷|\^| mod )((?:\(−{_NUMBER}\)|−?{_NUMBER})%?)$")
_LAST_NUMBER = re.compile(_NUMBER + "$")
_THOUSANDS = {sep: re.compile(rf"\d{re.escape(sep)}\d{{3}}(?!\d)") for sep in ",."}


class Calculator:
    def __init__(self):
        self.expr = ""
        self.ans = 0.0
        self.evaluated = False
        self.prev_line = ""
        self.error = None
        self.last_tail = None
        # Rounded text of `ans` at the start of `expr`; evaluated as the exact `ans`.
        self.ans_text = ""
        self.scientific = False
        self.degrees = True
        # Decimal symbol shown, copied and pasted: "." or ",". `expr` always uses ".".
        self.decimal = "."

    # Helpers -------------------------------------------------------------
    def _ends_operand(self):
        return bool(_ENDS_OPERAND.search(self.expr))

    def _ends_with_op(self):
        return self.expr.endswith(BINARY_OPS)

    def _start_fresh(self):
        self.expr = ""
        self.evaluated = False
        self.prev_line = ""
        self.ans_text = ""

    def _show_ans(self):
        self.expr = self.ans_text = raw_number(self.ans)

    def _continue_from_result(self):
        self._show_ans()
        self.evaluated = False

    def _exact(self, text):
        """Swap the rounded result at the start of `text` for the exact `Ans`,
        so 1÷3=, ×3= gives 1 rather than 0.999999999999999."""
        seed = self.ans_text
        if seed and text.startswith(seed) and not re.match(r"[\d.E]", text[len(seed):]):
            return "Ans" + text[len(seed):]
        return text

    def evaluate(self, text):
        return evaluate(self._exact(text), scientific=self.scientific, degrees=self.degrees,
                        ans=self.ans)

    # Keys ----------------------------------------------------------------
    def press(self, action):
        """Apply one key. Returns 'ok', 'result', 'error' or 'ignored'."""
        self.error = None
        kind, _, arg = action.partition(":")
        handler = getattr(self, f"_key_{kind}", None)
        if handler is None:
            return "ignored"
        return handler(arg) or "ok"

    def _key_digit(self, digit):
        if self.evaluated:
            self._start_fresh()
        number = _CURRENT_NUMBER.search(self.expr).group()
        if number == "0" and digit != ".":
            self.expr = self.expr[:-1]
        self.expr += digit
        return "ok"

    def _key_dot(self, _):
        if self.evaluated:
            self._start_fresh()
        number = _CURRENT_NUMBER.search(self.expr).group()
        if "." in number or "E" in number:
            return "ignored"
        self.expr += "." if number else "0."
        return "ok"

    def _key_op(self, op):
        if self.evaluated:
            self._continue_from_result()
        if self.expr.endswith("E"):
            if op == "−":
                self.expr += op
                return "ok"
            return "ignored"
        if not self.expr or self.expr.endswith("("):
            if op == "−":
                self.expr += op
                return "ok"
            return "ignored"
        if self._ends_with_op():
            if op == "−" and self.expr.endswith(("×", "÷", "^")):
                self.expr += op
                return "ok"
            # Replace the trailing operator(s), e.g. "5×−" then "+" gives "5+".
            while self._ends_with_op():
                for o in BINARY_OPS:
                    if self.expr.endswith(o):
                        self.expr = self.expr[: -len(o)]
                        break
            if not self.expr:
                return "ignored"
        self.expr += op
        return "ok"

    def _key_pct(self, _):
        if self.evaluated:
            self._continue_from_result()
        if self._ends_operand() and not self.expr.endswith("%"):
            self.expr += "%"
            return "ok"
        return "ignored"

    def _key_neg(self, _):
        if self.evaluated:
            self.ans = -self.ans
            self._show_ans()
            return "ok"
        m = _LAST_NUMBER.search(self.expr)
        if m is None:
            if not self.expr or self._ends_with_op() or self.expr.endswith("("):
                self.expr += "(−"
                return "ok"
            return "ignored"
        prefix, number = self.expr[: m.start()], m.group()
        if prefix.endswith("(−"):
            self.expr = prefix[:-2] + number
        else:
            self.expr = prefix + "(−" + number
        return "ok"

    def _key_clear(self, _):
        self._start_fresh()
        self.last_tail = None
        return "ok"

    def _key_back(self, _):
        if self.evaluated:
            self.evaluated = False
            self.prev_line = ""
            return "ok"
        if not self.expr:
            return "ignored"
        for tok in MULTI_TOKENS:
            if self.expr.endswith(tok):
                self.expr = self.expr[: -len(tok)]
                return "ok"
        self.expr = self.expr[:-1]
        return "ok"

    def _key_func(self, func):
        if self.evaluated:
            self._start_fresh()
        self.expr += func
        return "ok"

    def _key_post(self, post):
        if self.evaluated:
            self._continue_from_result()
        if self._ends_operand():
            self.expr += post
            return "ok"
        return "ignored"

    def _key_const(self, const):
        if self.evaluated:
            self._start_fresh()
        self.expr += const
        return "ok"

    def _key_open(self, _):
        if self.evaluated:
            self._start_fresh()
        self.expr += "("
        return "ok"

    def _key_close(self, _):
        if self.expr.count("(") > self.expr.count(")") and self._ends_operand():
            self.evaluated = False
            self.expr += ")"
            return "ok"
        return "ignored"

    def _key_exp(self, _):
        if self.evaluated:
            self._continue_from_result()
        number = _CURRENT_NUMBER.search(self.expr).group()
        if number and number[-1].isdigit() and "E" not in number:
            self.expr += "E"
            return "ok"
        return "ignored"

    def _key_eq(self, _):
        if self.evaluated:
            if not self.last_tail:
                return "ignored"
            expr = raw_number(self.ans) + self.last_tail
        else:
            expr = self.expr
        if not expr:
            return "ignored"
        expr += ")" * max(0, expr.count("(") - expr.count(")"))
        try:
            value = self.evaluate(expr)
        except CalcError as err:
            self.error = str(err)
            return "error"
        m = _TAIL.search(expr)
        self.last_tail = m.group(0) if m else None
        self.prev_line = expr
        self.ans = value
        self._show_ans()
        self.evaluated = True
        return "result"

    # Paste ---------------------------------------------------------------
    def paste(self, text):
        text = re.sub(r"\s+", "", text)
        # The other symbol is a thousands separator when the decimal symbol is
        # also present ("1,234.5") or it splits off groups of three ("1,000");
        # otherwise it is read as a decimal point too ("1,5" with ".").
        other = "," if self.decimal == "." else "."
        if self.decimal in text or _THOUSANDS[other].search(text):
            text = text.replace(other, "")
        table = {"*": "×", "/": "÷", "-": "−", ",": ".", "–": "−"}
        cleaned = "".join(table.get(c, c) for c in text)
        # "x" is multiplication, except in "exp".
        cleaned = re.sub(r"(?<!e)x|x(?!p)", "×", cleaned)
        cleaned = cleaned.replace("mod", " mod ")
        allowed = re.fullmatch(r"[\d.+−×÷^()%!πeE²³⁻¹√∛A-Za-z ]*", cleaned)
        if not cleaned or not allowed:
            return "ignored"
        if self.evaluated:
            self._start_fresh()
        self.expr += cleaned
        return "ok"

    # Views ---------------------------------------------------------------
    def display(self, decimals=None, grouping=True):
        if self.evaluated:
            text = format_number(self.ans, decimals=decimals, grouping=grouping)
        else:
            text = pretty(self.expr, grouping) if self.expr else "0"
        return localize(text, self.decimal)

    def preview(self, decimals=None, grouping=True):
        """Live result while typing, or '' when there is nothing useful to show."""
        if self.evaluated or not self.expr:
            return ""
        if re.fullmatch(r"−?(?:\d+\.?\d*|\.\d+)", self.expr):
            return ""
        try:
            value = self.evaluate(self.expr)
        except CalcError:
            return ""
        return localize(format_number(value, decimals=decimals, grouping=grouping), self.decimal)

    def copy_text(self, decimals=None):
        if self.evaluated:
            text = format_number(self.ans, decimals=decimals, grouping=False, pretty_minus=False)
        else:
            try:
                value = self.evaluate(self.expr)
                text = format_number(value, decimals=decimals, grouping=False, pretty_minus=False)
            except CalcError:
                text = self.expr
        return localize(text, self.decimal)

    def use_value(self, value):
        """Insert a value (e.g. from history) into the expression."""
        text = raw_number(value)
        if self.evaluated or not self.expr:
            self.expr = text
            self.evaluated = False
            self.prev_line = ""
            self.ans_text = ""
        elif self._ends_with_op() or self.expr.endswith("("):
            self.expr += text
        else:
            self.expr += "×" + text
        return "ok"
