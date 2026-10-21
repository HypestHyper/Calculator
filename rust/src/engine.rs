//! Safe expression engine. A small recursive-descent parser; no eval.
//!
//! Port of `calculator/engine.py`. Results and error messages match it.

use std::f64::consts::{E, PI};
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

/// Raised for any expression the engine cannot evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalcError(pub String);

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CalcError {}

type Result<T> = std::result::Result<T, CalcError>;

fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(CalcError(message.into()))
}

// Display symbols -> internal tokens. Order matters for multi-char symbols.
const DISPLAY_TO_INTERNAL: [(&str, &str); 11] = [
    ("⁻¹", "^(-1)"),
    ("²", "^2"),
    ("³", "^3"),
    ("×", "*"),
    ("÷", "/"),
    ("−", "-"),
    ("π", "pi"),
    ("√", "sqrt"),
    ("∛", "cbrt"),
    ("\u{2009}", ""), // thin space used for digit grouping
    (",", ""),
];

static TOKEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:(?P<num>(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:E[+-]?[0-9]+)?)",
        r"|(?P<name>[A-Za-z]+)",
        r"|(?P<op>[-+*/^%!(),])",
        r"|(?P<ws>\s+))",
    ))
    .unwrap()
});

const FUNCS: [&str; 12] = [
    "sin", "cos", "tan", "asin", "acos", "atan", "log", "ln", "exp", "sqrt", "cbrt", "abs",
];

fn is_scientific_only(name: &str) -> bool {
    FUNCS.contains(&name) || matches!(name, "pi" | "e" | "mod")
}

fn constant(name: &str) -> Option<f64> {
    match name {
        "pi" => Some(PI),
        "e" => Some(E),
        _ => None,
    }
}

fn cbrt(x: f64) -> f64 {
    x.abs().powf(1.0 / 3.0).copysign(x)
}

fn clean_trig(value: f64) -> f64 {
    // sin(180°) should be 0, not 1.2e-16.
    if value.abs() < 1e-12 {
        0.0
    } else {
        value
    }
}

pub fn to_internal(text: &str) -> String {
    let mut text = text.to_string();
    for (src, dst) in DISPLAY_TO_INTERNAL {
        text = text.replace(src, dst);
    }
    text
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Num,
    Name,
    Op,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    text: String,
}

fn tokenize(text: &str) -> Result<Vec<Token>> {
    let text = to_internal(text);
    let mut tokens = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        let Some(caps) = TOKEN.captures(rest) else {
            let ch = rest.chars().next().unwrap();
            return fail(format!("Unexpected “{ch}”"));
        };
        let whole = caps.get(0).unwrap();
        pos += whole.end();
        let kind = if caps.name("num").is_some() {
            Kind::Num
        } else if caps.name("name").is_some() {
            Kind::Name
        } else if caps.name("op").is_some() {
            Kind::Op
        } else {
            continue; // whitespace
        };
        tokens.push(Token {
            kind,
            text: whole.as_str().to_string(),
        });
    }
    // Auto-close open parentheses so "sin(30" and "(−5" work.
    let mut depth: i64 = 0;
    for token in &tokens {
        if token.text == "(" {
            depth += 1;
        } else if token.text == ")" {
            depth -= 1;
            if depth < 0 {
                return fail("Unbalanced parentheses");
            }
        }
    }
    for _ in 0..depth {
        tokens.push(Token {
            kind: Kind::Op,
            text: ")".into(),
        });
    }
    Ok(tokens)
}

/// Deepest nesting accepted. Python stops at its recursion limit; Rust would
/// overflow the stack instead, so the parser counts.
const MAX_DEPTH: usize = 500;

/// Grammar, lowest to highest precedence:
///
/// ```text
/// expr    := term (('+' | '-') term)*
/// term    := unary (('*' | '/' | 'mod' | <implicit>) unary)*
/// unary   := ('+' | '-') unary | power
/// power   := postfix ('^' unary)?            right-associative
/// postfix := primary ('!' | '%')*
/// primary := number | constant | 'Ans' | func '(' expr ')' | '(' expr ')'
/// ```
///
/// Implicit multiplication: 2π, 3(4), (1+2)(3), 2sin(30).
/// "a + b%" means a + a·b/100, like a desk calculator.
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    scientific: bool,
    degrees: bool,
    ans: f64,
    depth: usize,
}

impl Parser {
    // Angle helpers -------------------------------------------------------
    fn radians(&self, x: f64) -> f64 {
        if self.degrees {
            x.to_radians()
        } else {
            x
        }
    }

    fn angle_from_radians(&self, x: f64) -> f64 {
        if self.degrees {
            x.to_degrees()
        } else {
            x
        }
    }

    fn tan(&self, x: f64) -> Result<f64> {
        let rad = self.radians(x);
        // Odd multiples of 90° (π/2) give a huge number instead of an error.
        if rad.cos().abs() < 1e-12 {
            return fail("tan is undefined here");
        }
        Ok(clean_trig(rad.tan()))
    }

    /// One function call, raising errors the way Python's math module does:
    /// a NaN from a real argument is a domain error, an infinity from a
    /// finite one is an overflow.
    fn call(&self, name: &str, x: f64) -> Result<f64> {
        let y = match name {
            "sin" => clean_trig(self.radians(x).sin()),
            "cos" => clean_trig(self.radians(x).cos()),
            "tan" => self.tan(x)?,
            "asin" => self.angle_from_radians(x.asin()),
            "acos" => self.angle_from_radians(x.acos()),
            "atan" => self.angle_from_radians(x.atan()),
            "log" | "ln" if x <= 0.0 => f64::NAN,
            "log" => x.log10(),
            "ln" => x.ln(),
            "exp" => x.exp(),
            "sqrt" => x.sqrt(),
            "cbrt" => cbrt(x),
            "abs" => x.abs(),
            _ => unreachable!("not a function: {name}"),
        };
        if y.is_nan() && !x.is_nan() {
            return fail(format!("Outside the domain of {name}"));
        }
        if y.is_infinite() && x.is_finite() {
            return fail("Number too large");
        }
        Ok(y)
    }

    // Token helpers -------------------------------------------------------
    fn peek_kind(&self) -> Option<Kind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }

    /// Text of the next token, or "" at the end (no token is empty).
    fn peek(&self) -> &str {
        self.tokens.get(self.pos).map_or("", |t| t.text.as_str())
    }

    fn expect(&mut self, sym: &str) -> Result<()> {
        if self.peek() != sym {
            return fail(format!("Missing “{sym}”"));
        }
        self.pos += 1;
        Ok(())
    }

    fn starts_operand(&self) -> bool {
        match self.peek_kind() {
            Some(Kind::Num) => true,
            Some(Kind::Name) => self.peek() != "mod",
            _ => self.peek() == "(",
        }
    }

    fn require_scientific(&self, what: &str) -> Result<()> {
        if self.scientific {
            Ok(())
        } else {
            fail(format!("{what} needs scientific mode"))
        }
    }

    // Grammar -------------------------------------------------------------
    fn parse(&mut self) -> Result<f64> {
        if self.tokens.is_empty() {
            return fail("Nothing to calculate");
        }
        let value = self.expr()?;
        if self.pos != self.tokens.len() {
            return fail(format!("Unexpected “{}”", self.peek()));
        }
        Ok(value)
    }

    fn expr(&mut self) -> Result<f64> {
        let mut value = self.term()?;
        while matches!(self.peek(), "+" | "-") {
            let op = self.peek().to_string();
            self.pos += 1;
            let start = self.pos;
            let mut rhs = self.term()?;
            // Desk-calculator percent: 200 + 10% = 220.
            if self.tokens[self.pos - 1].text == "%" && self.is_plain_percent(start) {
                rhs *= value;
            }
            value = if op == "+" { value + rhs } else { value - rhs };
        }
        Ok(value)
    }

    fn is_plain_percent(&self, start: usize) -> bool {
        self.tokens[start..self.pos]
            .iter()
            .all(|t| t.kind == Kind::Num || t.text == "%")
    }

    fn term(&mut self) -> Result<f64> {
        let mut value = self.unary()?;
        loop {
            match self.peek() {
                sym @ ("*" | "/") => {
                    let multiply = sym == "*";
                    self.pos += 1;
                    let rhs = self.unary()?;
                    if multiply {
                        value *= rhs;
                    } else {
                        if rhs == 0.0 {
                            return fail("Can’t divide by zero");
                        }
                        value /= rhs;
                    }
                }
                "mod" if self.peek_kind() == Some(Kind::Name) => {
                    self.require_scientific("mod")?;
                    self.pos += 1;
                    let rhs = self.unary()?;
                    if rhs == 0.0 {
                        return fail("Can’t divide by zero");
                    }
                    if value.is_infinite() && !rhs.is_nan() {
                        return fail("Error"); // math.fmod refuses an infinite dividend
                    }
                    value %= rhs; // fmod, like math.fmod
                }
                _ if self.starts_operand() => value *= self.unary()?,
                _ => return Ok(value),
            }
        }
    }

    fn unary(&mut self) -> Result<f64> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return fail("Expression too deep");
        }
        let value = match self.peek() {
            sym @ ("+" | "-") => {
                let negate = sym == "-";
                self.pos += 1;
                let value = self.unary()?;
                if negate {
                    -value
                } else {
                    value
                }
            }
            _ => self.power()?,
        };
        self.depth -= 1;
        Ok(value)
    }

    fn power(&mut self) -> Result<f64> {
        let base = self.postfix()?;
        if self.peek() != "^" {
            return Ok(base);
        }
        self.require_scientific("Powers")?;
        self.pos += 1;
        let exponent = self.unary()?;
        if base == 0.0 && exponent < 0.0 {
            return fail("Can’t divide by zero");
        }
        if base < 0.0 {
            if !exponent.is_finite() {
                return fail("Error");
            }
            if exponent.fract() != 0.0 {
                return fail("Not a real number");
            }
        }
        let value = base.powf(exponent);
        if value.is_infinite() && base.is_finite() && exponent.is_finite() {
            return fail("Number too large");
        }
        Ok(value)
    }

    fn postfix(&mut self) -> Result<f64> {
        let mut value = self.primary()?;
        loop {
            match self.peek() {
                "%" => {
                    self.pos += 1;
                    value /= 100.0;
                }
                "!" => {
                    self.require_scientific("Factorial")?;
                    self.pos += 1;
                    value = factorial(value)?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn primary(&mut self) -> Result<f64> {
        let Some(token) = self.tokens.get(self.pos).cloned() else {
            return fail("Incomplete expression");
        };
        match token.kind {
            Kind::Num => {
                self.pos += 1;
                token.text.parse::<f64>().or_else(|_| fail("Error"))
            }
            Kind::Name => {
                self.pos += 1;
                let name = token.text.as_str();
                if name == "Ans" {
                    return Ok(self.ans);
                }
                if is_scientific_only(name) {
                    self.require_scientific(name)?;
                }
                if let Some(value) = constant(name) {
                    return Ok(value);
                }
                if FUNCS.contains(&name) {
                    self.expect("(")?;
                    let arg = self.expr()?;
                    self.expect(")")?;
                    return self.call(name, arg);
                }
                fail(format!("Unknown “{name}”"))
            }
            Kind::Op if token.text == "(" => {
                self.pos += 1;
                let value = self.expr()?;
                self.expect(")")?;
                Ok(value)
            }
            Kind::Op => fail(format!("Unexpected “{}”", token.text)),
        }
    }
}

fn factorial(x: f64) -> Result<f64> {
    if x < 0.0 {
        return fail("Factorial needs a whole number");
    }
    if !x.is_finite() {
        return fail("Error");
    }
    if x.fract() != 0.0 {
        return fail("Factorial needs a whole number");
    }
    if x > 170.0 {
        return fail("Number too large");
    }
    // Exact, then rounded once, like float(math.factorial(n)) in Python.
    let mut limbs: Vec<u32> = vec![1]; // little-endian, base 2^32
    for i in 2..=(x as u64) {
        let mut carry = 0u64;
        for limb in &mut limbs {
            let product = u64::from(*limb) * i + carry;
            *limb = product as u32;
            carry = product >> 32;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
    }
    Ok(big_to_f64(&limbs))
}

/// A non-negative big integer (little-endian base-2^32 limbs) to the nearest
/// f64, ties to even, as Python's int-to-float conversion does.
fn big_to_f64(limbs: &[u32]) -> f64 {
    let bit = |n: usize| (limbs[n / 32] >> (n % 32)) & 1 == 1;
    let top = limbs.len() - 1;
    let bit_len = top * 32 + (32 - limbs[top].leading_zeros() as usize);
    if bit_len <= 53 {
        return limbs
            .iter()
            .rev()
            .fold(0.0, |acc, &l| acc * 4294967296.0 + f64::from(l));
    }
    // Keep 53 bits, then round on the next bit and anything below it.
    let shift = bit_len - 53;
    let mut mantissa = (shift..bit_len)
        .rev()
        .fold(0u64, |acc, n| acc << 1 | u64::from(bit(n)));
    let round = bit(shift - 1);
    let sticky = (0..shift - 1).any(bit);
    if round && (sticky || mantissa & 1 == 1) {
        mantissa += 1;
    }
    mantissa as f64 * 2f64.powi(shift as i32)
}

/// Evaluation settings: scientific mode, degrees or radians, and the last answer.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub scientific: bool,
    pub degrees: bool,
    pub ans: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            scientific: true,
            degrees: true,
            ans: 0.0,
        }
    }
}

/// Evaluate an expression (display or ASCII form).
pub fn evaluate(text: &str, options: &Options) -> Result<f64> {
    let tokens = tokenize(text)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        scientific: options.scientific,
        degrees: options.degrees,
        ans: options.ans,
        depth: 0,
    };
    let value = parser.parse()?;
    if value.is_nan() {
        return fail("Not a real number");
    }
    if value.is_infinite() {
        return fail("Number too large");
    }
    Ok(value)
}

// Formatting --------------------------------------------------------------

pub const THIN_SPACE: char = '\u{2009}';

fn group(int_part: &str) -> String {
    let (sign, digits) = match int_part.strip_prefix('-') {
        Some(digits) => ("−", digits),
        None => ("", int_part),
    };
    let mut groups = Vec::new();
    let mut rest = digits;
    while rest.len() > 3 {
        let (head, tail) = rest.split_at(rest.len() - 3);
        groups.push(tail);
        rest = head;
    }
    groups.push(rest);
    groups.reverse();
    let separator = THIN_SPACE.to_string();
    format!("{sign}{}", groups.join(&separator))
}

/// Drop trailing zeros after the decimal point, then the point itself.
fn strip_zeros(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}

/// Python's `format(value, f".{precision}g")`.
fn format_general(value: f64, precision: usize) -> String {
    let precision = precision.max(1);
    let sci = format!("{:.*e}", precision - 1, value);
    let (mantissa, exp) = sci.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if -4 <= exp && exp < precision as i32 {
        let decimals = (precision as i32 - 1 - exp) as usize;
        strip_zeros(&format!("{value:.decimals$}")).to_string()
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip_zeros(mantissa), exp.abs())
    }
}

/// How `format_number` writes a value.
#[derive(Debug, Clone, Copy)]
pub struct Format {
    /// None is Auto: up to `max_digits` significant digits with trailing
    /// zeros dropped, switching to E notation for very large or small values.
    /// A number fixes the number of decimal places.
    pub decimals: Option<usize>,
    pub grouping: bool,
    pub max_digits: usize,
    pub pretty_minus: bool,
}

impl Default for Format {
    fn default() -> Self {
        Format {
            decimals: None,
            grouping: true,
            max_digits: 12,
            pretty_minus: true,
        }
    }
}

/// Format a result.
pub fn format_number(value: f64, format: &Format) -> String {
    let value = if value == 0.0 { 0.0 } else { value }; // never print "-0"
    let max_digits = format.max_digits;
    let magnitude = value.abs();
    let use_sci =
        magnitude >= 10f64.powi(max_digits as i32) || (0.0 < magnitude && magnitude < 1e-7);

    if use_sci {
        let sci = format!("{:.*e}", max_digits - 1, value);
        let (mantissa, exp) = sci.split_once('e').unwrap();
        let mantissa = match format.decimals {
            None => strip_zeros(mantissa).to_string(),
            Some(decimals) => format!("{:.decimals$}", mantissa.parse::<f64>().unwrap()),
        };
        let exp: i32 = exp.parse().unwrap();
        let text = format!("{mantissa}E{exp}");
        return if format.pretty_minus {
            text.replace('-', "−")
        } else {
            text
        };
    }

    let mut text = match format.decimals {
        None => {
            let text = format_general(value, max_digits);
            if text.contains('e') {
                // .g fell back to exponent form
                strip_zeros(&format!("{value:.max_digits$}")).to_string()
            } else {
                text
            }
        }
        Some(decimals) => {
            let text = format!("{value:.decimals$}");
            // -0.001 at 2 decimals is "0.00", not "-0.00"
            if text.parse::<f64>() == Ok(0.0) {
                text.trim_start_matches('-').to_string()
            } else {
                text
            }
        }
    };

    if format.grouping {
        text = match text.split_once('.') {
            Some((int_part, frac)) => format!("{}.{frac}", group(int_part)),
            None => group(&text),
        };
    }
    if format.pretty_minus {
        text = text.replace('-', "−");
    }
    text
}

/// Full-precision, parseable form used when a result feeds a new expression.
pub fn raw_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    format_number(
        value,
        &Format {
            grouping: false,
            max_digits: 15,
            ..Format::default()
        },
    )
}

/// Show `text` (which uses "." internally) with the chosen decimal symbol.
pub fn localize(text: &str, decimal: char) -> String {
    if decimal == '.' {
        text.to_string()
    } else {
        text.replace('.', &decimal.to_string())
    }
}

/// Display form of an expression: groups long integer runs (4 or more
/// digits) that do not follow a decimal point or an E.
pub fn pretty(expr: &str, grouping: bool) -> String {
    if !grouping {
        return expr.to_string();
    }
    let mut out = String::with_capacity(expr.len() + 8);
    let mut prev: Option<char> = None;
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        if !c.is_ascii_digit() || prev.is_some_and(|p| p == '.' || p == 'E') {
            // Not the start of a run that should be grouped: copy it, along
            // with the rest of its digit run.
            out.push(c);
            prev = Some(c);
            if c.is_ascii_digit() {
                while let Some(&d) = chars.peek().filter(|d| d.is_ascii_digit()) {
                    out.push(d);
                    prev = Some(d);
                    chars.next();
                }
            }
            continue;
        }
        let mut run = String::from(c);
        while let Some(&d) = chars.peek().filter(|d| d.is_ascii_digit()) {
            run.push(d);
            chars.next();
        }
        prev = run.chars().last();
        if run.len() >= 4 {
            out.push_str(&group(&run));
        } else {
            out.push_str(&run);
        }
    }
    out
}
