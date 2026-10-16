"""Safe expression engine. A small recursive-descent parser; no eval()."""

import math
import re

__all__ = ["CalcError", "evaluate", "format_number", "to_internal", "pretty", "localize"]


class CalcError(Exception):
    """Raised for any expression the engine cannot evaluate."""


# Display symbols -> internal tokens. Order matters for multi-char symbols.
_DISPLAY_TO_INTERNAL = [
    ("⁻¹", "^(-1)"),
    ("²", "^2"),
    ("³", "^3"),
    ("×", "*"),
    ("÷", "/"),
    ("−", "-"),
    ("π", "pi"),
    ("√", "sqrt"),
    ("∛", "cbrt"),
    ("\u2009", ""),  # thin space used for digit grouping
    (",", ""),
]

_TOKEN_RE = re.compile(
    r"(?P<num>(?:\d+\.?\d*|\.\d+)(?:E[+-]?\d+)?)"
    r"|(?P<name>[A-Za-z]+)"
    r"|(?P<op>[-+*/^%!(),])"
    r"|(?P<ws>\s+)"
)

_CONSTANTS = {"pi": math.pi, "e": math.e}


def _cbrt(x):
    return math.copysign(abs(x) ** (1.0 / 3.0), x)


def _clean_trig(value):
    # sin(180°) should be 0, not 1.2e-16.
    return 0.0 if abs(value) < 1e-12 else value


# Each function takes (parser, argument) so trig can honour the angle unit.
_FUNCS = {
    "sin": lambda p, x: _clean_trig(math.sin(p.to_rad(x))),
    "cos": lambda p, x: _clean_trig(math.cos(p.to_rad(x))),
    "tan": lambda p, x: p.tan(x),
    "asin": lambda p, x: p.from_rad(math.asin(x)),
    "acos": lambda p, x: p.from_rad(math.acos(x)),
    "atan": lambda p, x: p.from_rad(math.atan(x)),
    "log": lambda p, x: math.log10(x),
    "ln": lambda p, x: math.log(x),
    "exp": lambda p, x: math.exp(x),
    "sqrt": lambda p, x: math.sqrt(x),
    "cbrt": lambda p, x: _cbrt(x),
    "abs": lambda p, x: abs(x),
}

_SCIENTIFIC_ONLY = set(_FUNCS) | set(_CONSTANTS) | {"mod"}


def to_internal(text):
    for src, dst in _DISPLAY_TO_INTERNAL:
        text = text.replace(src, dst)
    return text


def _tokenize(text):
    text = to_internal(text)
    tokens, pos = [], 0
    while pos < len(text):
        m = _TOKEN_RE.match(text, pos)
        if m is None:
            raise CalcError(f"Unexpected “{text[pos]}”")
        pos = m.end()
        if m.lastgroup != "ws":
            tokens.append((m.lastgroup, m.group()))
    # Auto-close open parentheses so "sin(30" and "(−5" work.
    depth = 0
    for kind, tok in tokens:
        if tok == "(":
            depth += 1
        elif tok == ")":
            depth -= 1
            if depth < 0:
                raise CalcError("Unbalanced parentheses")
    tokens.extend([("op", ")")] * depth)
    return tokens


class _Parser:
    """Grammar, lowest to highest precedence:

        expr    := term (('+' | '-') term)*
        term    := unary (('*' | '/' | 'mod' | <implicit>) unary)*
        unary   := ('+' | '-') unary | power
        power   := postfix ('^' unary)?            right-associative
        postfix := primary ('!' | '%')*
        primary := number | constant | 'Ans' | func '(' expr ')' | '(' expr ')'

    Implicit multiplication: 2π, 3(4), (1+2)(3), 2sin(30).
    "a + b%" means a + a·b/100, like a desk calculator.
    """

    def __init__(self, tokens, scientific, degrees, ans):
        self.tokens = tokens
        self.pos = 0
        self.scientific = scientific
        self.degrees = degrees
        self.ans = ans

    # Angle helpers -------------------------------------------------------
    def to_rad(self, x):
        return math.radians(x) if self.degrees else x

    def from_rad(self, x):
        return math.degrees(x) if self.degrees else x

    def tan(self, x):
        rad = self.to_rad(x)
        # Odd multiples of 90° (π/2) give a huge number instead of an error.
        if abs(math.cos(rad)) < 1e-12:
            raise CalcError("tan is undefined here")
        return _clean_trig(math.tan(rad))

    # Token helpers -------------------------------------------------------
    def _peek(self):
        return self.tokens[self.pos] if self.pos < len(self.tokens) else (None, None)

    def _next(self):
        tok = self._peek()
        self.pos += 1
        return tok

    def _expect(self, sym):
        if self._peek()[1] != sym:
            raise CalcError(f"Missing “{sym}”")
        self.pos += 1

    def _starts_operand(self):
        kind, text = self._peek()
        if kind == "num" or text == "(":
            return True
        return kind == "name" and text != "mod"

    # Grammar -------------------------------------------------------------
    def parse(self):
        if not self.tokens:
            raise CalcError("Nothing to calculate")
        value = self._expr()
        if self.pos != len(self.tokens):
            raise CalcError(f"Unexpected “{self._peek()[1]}”")
        return value

    def _expr(self):
        value = self._term()
        while self._peek()[1] in ("+", "-"):
            op = self._next()[1]
            start = self.pos
            rhs = self._term()
            # Desk-calculator percent: 200 + 10% = 220.
            if self.tokens[self.pos - 1][1] == "%" and self._is_plain_percent(start):
                rhs = value * rhs
            value = value + rhs if op == "+" else value - rhs
        return value

    def _is_plain_percent(self, start):
        span = self.tokens[start:self.pos]
        return all(tok[0] == "num" or tok[1] == "%" for tok in span)

    def _term(self):
        value = self._unary()
        while True:
            kind, sym = self._peek()
            if sym in ("*", "/"):
                self._next()
                rhs = self._unary()
                if sym == "*":
                    value *= rhs
                else:
                    if rhs == 0:
                        raise CalcError("Can’t divide by zero")
                    value /= rhs
            elif kind == "name" and sym == "mod":
                self._require_scientific("mod")
                self._next()
                rhs = self._unary()
                if rhs == 0:
                    raise CalcError("Can’t divide by zero")
                value = math.fmod(value, rhs)
            elif self._starts_operand():
                value *= self._unary()
            else:
                return value

    def _unary(self):
        sym = self._peek()[1]
        if sym in ("+", "-"):
            self._next()
            value = self._unary()
            return -value if sym == "-" else value
        return self._power()

    def _power(self):
        base = self._postfix()
        if self._peek()[1] == "^":
            self._require_scientific("Powers")
            self._next()
            exponent = self._unary()
            if base == 0 and exponent < 0:
                raise CalcError("Can’t divide by zero")
            if base < 0 and exponent != int(exponent):
                raise CalcError("Not a real number")
            try:
                return float(base) ** exponent
            except OverflowError:
                raise CalcError("Number too large")
        return base

    def _postfix(self):
        value = self._primary()
        while True:
            sym = self._peek()[1]
            if sym == "%":
                self._next()
                value = value / 100.0
            elif sym == "!":
                self._require_scientific("Factorial")
                self._next()
                value = _factorial(value)
            else:
                return value

    def _primary(self):
        kind, text = self._peek()
        if kind == "num":
            self._next()
            return float(text)
        if kind == "name":
            self._next()
            if text == "Ans":
                return self.ans
            if text in _SCIENTIFIC_ONLY:
                self._require_scientific(text)
            if text in _CONSTANTS:
                return _CONSTANTS[text]
            if text in _FUNCS:
                self._expect("(")
                arg = self._expr()
                self._expect(")")
                try:
                    return float(_FUNCS[text](self, arg))
                except ValueError:
                    raise CalcError(f"Outside the domain of {text}")
                except OverflowError:
                    raise CalcError("Number too large")
            raise CalcError(f"Unknown “{text}”")
        if text == "(":
            self._next()
            value = self._expr()
            self._expect(")")
            return value
        if kind is None:
            raise CalcError("Incomplete expression")
        raise CalcError(f"Unexpected “{text}”")

    def _require_scientific(self, what):
        if not self.scientific:
            raise CalcError(f"{what} needs scientific mode")


def _factorial(x):
    if x < 0 or x != int(x):
        raise CalcError("Factorial needs a whole number")
    if x > 170:
        raise CalcError("Number too large")
    return float(math.factorial(int(x)))


def evaluate(text, *, scientific=True, degrees=True, ans=0.0):
    """Evaluate an expression (display or ASCII form). Raises CalcError."""
    try:
        value = _Parser(_tokenize(text), scientific, degrees, ans).parse()
    except CalcError:
        raise
    except RecursionError:
        raise CalcError("Expression too deep")
    except (ValueError, OverflowError, TypeError, ZeroDivisionError):
        raise CalcError("Error")
    if math.isnan(value):
        raise CalcError("Not a real number")
    if math.isinf(value):
        raise CalcError("Number too large")
    return value


# Formatting --------------------------------------------------------------

THIN_SPACE = "\u2009"


def _group(int_part):
    sign = "−" if int_part.startswith("-") else ""
    digits = int_part.lstrip("-")
    groups = []
    while len(digits) > 3:
        groups.insert(0, digits[-3:])
        digits = digits[:-3]
    groups.insert(0, digits)
    return sign + THIN_SPACE.join(groups)


def format_number(value, *, decimals=None, grouping=True, max_digits=12, pretty_minus=True):
    """Format a result.

    decimals=None is Auto: up to `max_digits` significant digits with trailing
    zeros dropped, switching to E notation for very large or small values.
    An integer fixes the number of decimal places.
    """
    if value == 0:
        value = 0.0  # never print "-0"
    magnitude = abs(value)
    use_sci = magnitude >= 10 ** max_digits or (0 < magnitude < 1e-7)

    if use_sci:
        mant, exp = f"{value:.{max_digits - 1}e}".split("e")
        if decimals is None:
            mant = mant.rstrip("0").rstrip(".") if "." in mant else mant
        else:
            mant = f"{float(mant):.{decimals}f}"
        text = f"{mant}E{int(exp)}"
        return text.replace("-", "−") if pretty_minus else text

    if decimals is None:
        text = f"{value:.{max_digits}g}"
        if "e" in text:  # .g fell back to exponent form
            text = f"{value:.{max_digits}f}".rstrip("0").rstrip(".")
    else:
        text = f"{value:.{decimals}f}"
        if float(text) == 0:
            text = text.lstrip("-")  # -0.001 at 2 decimals is "0.00", not "-0.00"

    if grouping:
        int_part, _, frac = text.partition(".")
        text = _group(int_part) + ("." + frac if frac else "")
    if pretty_minus:
        text = text.replace("-", "−")
    return text


def raw_number(value):
    """Full-precision, parseable form used when a result feeds a new expression."""
    if value == 0:
        return "0"
    text = format_number(value, grouping=False, max_digits=15, pretty_minus=True)
    return text


def localize(text, decimal="."):
    """Show `text` (which uses "." internally) with the chosen decimal symbol."""
    return text.replace(".", decimal) if decimal != "." else text


_DIGIT_RUN = re.compile(r"(?<![\dE.])(\d{4,})")


def pretty(expr, grouping=True):
    """Display form of an expression: groups long integer runs."""
    if not grouping:
        return expr
    return _DIGIT_RUN.sub(lambda m: _group(m.group(1)), expr)
