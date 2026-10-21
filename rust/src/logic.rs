//! Input handling for the calculator: what each key does to the expression.
//!
//! Port of `calculator/logic.py`. No GTK, so it can be unit-tested.

use std::sync::LazyLock;

use regex::Regex;

use crate::engine::{
    self, format_number, localize, pretty, raw_number, CalcError, Format, Options,
};

const BINARY_OPS: [&str; 6] = ["+", "−", "×", "÷", "^", " mod "];

/// Tokens removed in one backspace, longest first.
const MULTI_TOKENS: [&str; 15] = [
    "asin(", "acos(", "atan(", " mod ", "sin(", "cos(", "tan(", "log(", "abs(", "exp(", "ln(",
    "Ans", "√(", "∛(", "⁻¹",
];

const NUMBER: &str = r"(?:[0-9]+\.?[0-9]*|\.[0-9]+)(?:E−?[0-9]+)?";

static ENDS_OPERAND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:[0-9.)πe!%²³]|⁻¹|Ans)$").unwrap());
static CURRENT_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[0-9.]*(?:E−?[0-9]*)?$").unwrap());
static LAST_NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!("{NUMBER}$")).unwrap());
/// The operation that pressing "=" again repeats, from its operator to the end.
static TAIL_BODY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:\+|−|×|÷|\^| mod )(?:\(−{NUMBER}\)|−?{NUMBER})%?$"
    ))
    .unwrap()
});
static PLAIN_NUMBER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^−?(?:[0-9]+\.?[0-9]*|\.[0-9]+)$").unwrap());
static PASTE_ALLOWED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[0-9.+−×÷^()%!πeE²³⁻¹√∛A-Za-z ]*$").unwrap());

/// The last binary operation in `expr`, repeated by pressing "=" again. The
/// operator must follow an operand, so the "−" in "2E−5" or a leading "−5" is
/// not matched, and the operand may be negative: "5×−3" and "5×(−3)" repeat
/// "×−3" and "×(−3)".
fn tail(expr: &str) -> Option<String> {
    let mut prev: Option<char> = None;
    for (i, c) in expr.char_indices() {
        let after_operand = prev.is_some_and(|p| "0123456789.)πe!%²³¹s".contains(p));
        if after_operand && TAIL_BODY.is_match(&expr[i..]) {
            return Some(expr[i..].to_string());
        }
        prev = Some(c);
    }
    None
}

/// True when `sep` splits off a group of three digits, as in "1,000".
fn has_thousands(text: &str, sep: char) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let digit = |i: usize| chars.get(i).is_some_and(|c| c.is_ascii_digit());
    (1..chars.len()).any(|i| {
        chars[i] == sep
            && digit(i - 1)
            && digit(i + 1)
            && digit(i + 2)
            && digit(i + 3)
            && !digit(i + 4)
    })
}

/// What a key press did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// "=" produced a result.
    Result,
    /// "=" failed; the message is in `Calculator::error`.
    Error,
    Ignored,
}

#[derive(Debug, Clone)]
pub struct Calculator {
    pub expr: String,
    pub ans: f64,
    pub evaluated: bool,
    pub prev_line: String,
    pub error: Option<String>,
    pub last_tail: Option<String>,
    /// Rounded text of `ans` at the start of `expr`; evaluated as the exact `ans`.
    pub ans_text: String,
    pub scientific: bool,
    pub degrees: bool,
    /// Decimal symbol shown, copied and pasted: '.' or ','. `expr` always uses '.'.
    pub decimal: char,
}

impl Default for Calculator {
    fn default() -> Self {
        Calculator {
            expr: String::new(),
            ans: 0.0,
            evaluated: false,
            prev_line: String::new(),
            error: None,
            last_tail: None,
            ans_text: String::new(),
            scientific: false,
            degrees: true,
            decimal: '.',
        }
    }
}

impl Calculator {
    pub fn new() -> Self {
        Self::default()
    }

    // Helpers -------------------------------------------------------------
    fn ends_operand(&self) -> bool {
        ENDS_OPERAND.is_match(&self.expr)
    }

    fn ends_with_op(&self) -> bool {
        BINARY_OPS.iter().any(|op| self.expr.ends_with(op))
    }

    fn current_number(&self) -> &str {
        CURRENT_NUMBER.find(&self.expr).map_or("", |m| m.as_str())
    }

    fn start_fresh(&mut self) {
        self.expr.clear();
        self.evaluated = false;
        self.prev_line.clear();
        self.ans_text.clear();
    }

    fn show_ans(&mut self) {
        self.ans_text = raw_number(self.ans);
        self.expr = self.ans_text.clone();
    }

    fn continue_from_result(&mut self) {
        self.show_ans();
        self.evaluated = false;
    }

    /// Swap the rounded result at the start of `text` for the exact `Ans`,
    /// so 1÷3=, ×3= gives 1 rather than 0.999999999999999.
    fn exact(&self, text: &str) -> String {
        if let Some(rest) = text
            .strip_prefix(self.ans_text.as_str())
            .filter(|_| !self.ans_text.is_empty())
        {
            if !rest.starts_with(|c: char| c.is_ascii_digit() || c == '.' || c == 'E') {
                return format!("Ans{rest}");
            }
        }
        text.to_string()
    }

    pub fn evaluate(&self, text: &str) -> Result<f64, CalcError> {
        let options = Options {
            scientific: self.scientific,
            degrees: self.degrees,
            ans: self.ans,
        };
        engine::evaluate(&self.exact(text), &options)
    }

    // Keys ----------------------------------------------------------------

    /// Apply one key, written as "kind:argument" (for example "digit:7",
    /// "op:×", "func:sin(" or "eq:").
    pub fn press(&mut self, action: &str) -> Status {
        self.error = None;
        let (kind, arg) = action.split_once(':').unwrap_or((action, ""));
        match kind {
            "digit" => self.key_digit(arg),
            "dot" => self.key_dot(),
            "op" => self.key_op(arg),
            "pct" => self.key_pct(),
            "neg" => self.key_neg(),
            "clear" => self.key_clear(),
            "back" => self.key_back(),
            "func" | "const" => self.key_insert(arg),
            "post" => self.key_post(arg),
            "open" => self.key_insert("("),
            "close" => self.key_close(),
            "exp" => self.key_exp(),
            "eq" => self.key_eq(),
            _ => Status::Ignored,
        }
    }

    fn key_digit(&mut self, digit: &str) -> Status {
        if self.evaluated {
            self.start_fresh();
        }
        if self.current_number() == "0" && digit != "." {
            self.expr.pop();
        }
        self.expr.push_str(digit);
        Status::Ok
    }

    fn key_dot(&mut self) -> Status {
        if self.evaluated {
            self.start_fresh();
        }
        let number = self.current_number();
        if number.contains('.') || number.contains('E') {
            return Status::Ignored;
        }
        let text = if number.is_empty() { "0." } else { "." };
        self.expr.push_str(text);
        Status::Ok
    }

    fn key_op(&mut self, op: &str) -> Status {
        if self.evaluated {
            self.continue_from_result();
        }
        if self.expr.ends_with('E') || self.expr.is_empty() || self.expr.ends_with('(') {
            if op == "−" {
                self.expr.push_str(op);
                return Status::Ok;
            }
            return Status::Ignored;
        }
        if self.ends_with_op() {
            if op == "−" && self.expr.ends_with(['×', '÷', '^']) {
                self.expr.push_str(op);
                return Status::Ok;
            }
            // Replace the trailing operator(s), e.g. "5×−" then "+" gives "5+".
            while let Some(o) = BINARY_OPS.iter().find(|o| self.expr.ends_with(*o)) {
                self.expr.truncate(self.expr.len() - o.len());
            }
            if self.expr.is_empty() {
                return Status::Ignored;
            }
        }
        self.expr.push_str(op);
        Status::Ok
    }

    fn key_pct(&mut self) -> Status {
        if self.evaluated {
            self.continue_from_result();
        }
        if self.ends_operand() && !self.expr.ends_with('%') {
            self.expr.push('%');
            return Status::Ok;
        }
        Status::Ignored
    }

    fn key_neg(&mut self) -> Status {
        if self.evaluated {
            self.ans = -self.ans;
            self.show_ans();
            return Status::Ok;
        }
        let Some(m) = LAST_NUMBER.find(&self.expr) else {
            if self.expr.is_empty() || self.ends_with_op() || self.expr.ends_with('(') {
                self.expr.push_str("(−");
                return Status::Ok;
            }
            return Status::Ignored;
        };
        let (prefix, number) = self.expr.split_at(m.start());
        self.expr = match prefix.strip_suffix("(−") {
            Some(before) => format!("{before}{number}"),
            None => format!("{prefix}(−{number}"),
        };
        Status::Ok
    }

    fn key_clear(&mut self) -> Status {
        self.start_fresh();
        self.last_tail = None;
        Status::Ok
    }

    fn key_back(&mut self) -> Status {
        if self.evaluated {
            self.evaluated = false;
            self.prev_line.clear();
            return Status::Ok;
        }
        if self.expr.is_empty() {
            return Status::Ignored;
        }
        match MULTI_TOKENS.iter().find(|tok| self.expr.ends_with(*tok)) {
            Some(tok) => self.expr.truncate(self.expr.len() - tok.len()),
            None => {
                self.expr.pop();
            }
        }
        Status::Ok
    }

    /// Functions, constants and "(": start a new expression after a result.
    fn key_insert(&mut self, text: &str) -> Status {
        if self.evaluated {
            self.start_fresh();
        }
        self.expr.push_str(text);
        Status::Ok
    }

    fn key_post(&mut self, post: &str) -> Status {
        if self.evaluated {
            self.continue_from_result();
        }
        if self.ends_operand() {
            self.expr.push_str(post);
            return Status::Ok;
        }
        Status::Ignored
    }

    fn key_close(&mut self) -> Status {
        if self.expr.matches('(').count() > self.expr.matches(')').count() && self.ends_operand() {
            self.evaluated = false;
            self.expr.push(')');
            return Status::Ok;
        }
        Status::Ignored
    }

    fn key_exp(&mut self) -> Status {
        if self.evaluated {
            self.continue_from_result();
        }
        let number = self.current_number();
        if number.ends_with(|c: char| c.is_ascii_digit()) && !number.contains('E') {
            self.expr.push('E');
            return Status::Ok;
        }
        Status::Ignored
    }

    fn key_eq(&mut self) -> Status {
        let mut expr = if self.evaluated {
            match self.last_tail.as_deref() {
                None | Some("") => return Status::Ignored,
                Some(tail) => raw_number(self.ans) + tail,
            }
        } else {
            self.expr.clone()
        };
        if expr.is_empty() {
            return Status::Ignored;
        }
        let missing = expr
            .matches('(')
            .count()
            .saturating_sub(expr.matches(')').count());
        expr.push_str(&")".repeat(missing));
        let value = match self.evaluate(&expr) {
            Ok(value) => value,
            Err(err) => {
                self.error = Some(err.0);
                return Status::Error;
            }
        };
        self.last_tail = tail(&expr);
        self.prev_line = expr;
        self.ans = value;
        self.show_ans();
        self.evaluated = true;
        Status::Result
    }

    // Paste ---------------------------------------------------------------
    pub fn paste(&mut self, text: &str) -> Status {
        let mut text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        // The other symbol is a thousands separator when the decimal symbol is
        // also present ("1,234.5") or it splits off groups of three ("1,000");
        // otherwise it is read as a decimal point too ("1,5" with ".").
        let other = if self.decimal == '.' { ',' } else { '.' };
        if text.contains(self.decimal) || has_thousands(&text, other) {
            text = text.replace(other, "");
        }
        let chars: Vec<char> = text.chars().collect();
        let mut cleaned = String::with_capacity(text.len());
        for (i, &c) in chars.iter().enumerate() {
            cleaned.push(match c {
                '*' => '×',
                '/' => '÷',
                '-' | '–' => '−',
                ',' => '.',
                // "x" is multiplication, except in "exp".
                'x' if !(i > 0 && chars[i - 1] == 'e' && chars.get(i + 1) == Some(&'p')) => '×',
                c => c,
            });
        }
        let cleaned = cleaned.replace("mod", " mod ");
        if cleaned.is_empty() || !PASTE_ALLOWED.is_match(&cleaned) {
            return Status::Ignored;
        }
        if self.evaluated {
            self.start_fresh();
        }
        self.expr.push_str(&cleaned);
        Status::Ok
    }

    // Views ---------------------------------------------------------------
    pub fn display(&self, decimals: Option<usize>, grouping: bool) -> String {
        let text = if self.evaluated {
            format_number(
                self.ans,
                &Format {
                    decimals,
                    grouping,
                    ..Format::default()
                },
            )
        } else if self.expr.is_empty() {
            "0".to_string()
        } else {
            pretty(&self.expr, grouping)
        };
        localize(&text, self.decimal)
    }

    /// Live result while typing, or "" when there is nothing useful to show.
    pub fn preview(&self, decimals: Option<usize>, grouping: bool) -> String {
        if self.evaluated || self.expr.is_empty() || PLAIN_NUMBER.is_match(&self.expr) {
            return String::new();
        }
        match self.evaluate(&self.expr) {
            Ok(value) => {
                let text = format_number(
                    value,
                    &Format {
                        decimals,
                        grouping,
                        ..Format::default()
                    },
                );
                localize(&text, self.decimal)
            }
            Err(_) => String::new(),
        }
    }

    pub fn copy_text(&self, decimals: Option<usize>) -> String {
        let format = Format {
            decimals,
            grouping: false,
            pretty_minus: false,
            ..Format::default()
        };
        let value = if self.evaluated {
            Ok(self.ans)
        } else {
            self.evaluate(&self.expr)
        };
        let text = match value {
            Ok(value) => format_number(value, &format),
            Err(_) => self.expr.clone(),
        };
        localize(&text, self.decimal)
    }

    /// Insert a value (e.g. from history) into the expression.
    pub fn use_value(&mut self, value: f64) -> Status {
        let text = raw_number(value);
        if self.evaluated || self.expr.is_empty() {
            self.expr = text;
            self.evaluated = false;
            self.prev_line.clear();
            self.ans_text.clear();
        } else if self.ends_with_op() || self.expr.ends_with('(') {
            self.expr.push_str(&text);
        } else {
            self.expr.push('×');
            self.expr.push_str(&text);
        }
        Status::Ok
    }
}
