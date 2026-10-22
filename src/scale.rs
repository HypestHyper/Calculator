//! Smaller windows: the calculator's sizes scale down so the window can be
//! made much smaller than the layout's normal minimum.
//!
//! [`scaled_css`] turns style.css into the same size rules multiplied by a
//! factor (fonts, key sizes, paddings, corners), scoped to the calculator
//! window so Preferences is not affected. [`choose_scale`] picks the largest
//! factor at which the contents fit.

use std::sync::LazyLock;

use regex::Regex;

/// The smallest factor: below it keys and text get too small to use.
pub const MIN_SCALE: f64 = 0.5;
/// Factors go in steps, so resizing restyles only now and then.
pub const STEP: f64 = 0.05;
/// Text never gets smaller than this, in px.
pub const MIN_FONT: f64 = 10.0;

const SCALED_PROPERTIES: [&str; 7] = [
    "font-size",
    "min-height",
    "min-width",
    "padding",
    "margin",
    "border-radius",
    "border-spacing",
];

static PX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+(?:\.\d+)?)px").unwrap());
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());

fn number(value: f64) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// Limit a selector to the calculator window; None for Preferences rules.
fn scoped(selector: &str) -> Option<String> {
    if selector.is_empty() || selector.contains("calc-prefs") {
        return None;
    }
    if selector.starts_with("window.calculator") {
        return Some(selector.to_string());
    }
    if selector.starts_with(".calculator") {
        return Some(format!("window{selector}"));
    }
    if [".text-", ".keys-", ".no-motion"]
        .iter()
        .any(|p| selector.starts_with(p))
    {
        // classes on the window itself
        return Some(format!("window.calculator{selector}"));
    }
    Some(format!("window.calculator {selector}"))
}

/// The size rules of `css` with every px length multiplied by `scale`.
/// Empty at full size, so nothing changes there.
pub fn scaled_css(css: &str, scale: f64) -> String {
    if scale >= 1.0 {
        return String::new();
    }
    let css = COMMENT.replace_all(css, "");
    let mut out = String::new();
    for block in css.split('}') {
        let Some((selectors, body)) = block.split_once('{') else {
            continue;
        };
        let mut declarations = Vec::new();
        for declaration in body.split(';') {
            let Some((prop, value)) = declaration.split_once(':') else {
                continue;
            };
            let (prop, value) = (prop.trim(), value.trim());
            if !value.contains("px") {
                continue;
            }
            if !SCALED_PROPERTIES.contains(&prop)
                && !prop.starts_with("padding-")
                && !prop.starts_with("margin-")
            {
                continue;
            }
            let scaled = PX.replace_all(value, |caps: &regex::Captures| {
                let px: f64 = caps[1].parse().unwrap();
                let mut new = px * scale;
                if prop == "font-size" {
                    new = new.max(px.min(MIN_FONT));
                }
                format!("{}px", number(new))
            });
            declarations.push(format!("{prop}: {scaled};"));
        }
        let selectors: Vec<String> = selectors
            .split(',')
            .filter_map(|s| scoped(s.trim()))
            .collect();
        if !declarations.is_empty() && !selectors.is_empty() {
            out.push_str(&format!(
                "{} {{ {} }}\n",
                selectors.join(", "),
                declarations.join(" ")
            ));
        }
    }
    out
}

/// The largest factor, in steps, at which `fixed + factor * per_scale` fits
/// in `available` on both axes.
pub fn choose_scale(available: [f64; 2], fixed: [f64; 2], per_scale: [f64; 2]) -> f64 {
    let mut scale: f64 = 1.0;
    for i in 0..2 {
        if per_scale[i] > 0.0 {
            scale = scale.min((available[i] - fixed[i]) / per_scale[i]);
        }
    }
    let stepped = (scale / STEP + 1e-9).floor() * STEP;
    (stepped.clamp(MIN_SCALE, 1.0) * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const STYLE: &str = include_str!("style.css");

    #[test]
    fn full_size_changes_nothing() {
        assert_eq!(scaled_css(STYLE, 1.0), "");
    }

    #[test]
    fn sizes_scale_and_stay_in_the_calculator_window() {
        let css = scaled_css(STYLE, 0.5);
        assert!(css.contains(
            "window.calculator button.key { min-height: 32px; min-width: 28px; font-size: 14px; }"
        ));
        assert!(css.contains(
            "window.calculator.keys-compact button.key { min-height: 25px; font-size: 12px; }"
        ));
        assert!(css.contains("window.calculator .keypad-sheet { border-radius: 18px 18px 0 0; padding: 8px 8px 9px 8px; }"));
        // Text has a floor; small text keeps its size.
        assert!(css.contains("window.calculator .prev-line { font-size: 10px; }"));
        assert!(css.contains(
            "window.calculator .chip { border-radius: 4px; font-size: 10px; padding: 1px 4px; }"
        ));
        assert!(!css.contains("calc-prefs"));
    }

    #[test]
    fn scale_fits_the_space() {
        // 100 fixed + 300 per unit of scale: 400 fits at 1, 250 at 0.5.
        assert_eq!(
            choose_scale([400.0, 1000.0], [100.0, 0.0], [300.0, 100.0]),
            1.0
        );
        assert_eq!(
            choose_scale([340.0, 1000.0], [100.0, 0.0], [300.0, 100.0]),
            0.8
        );
        assert_eq!(
            choose_scale([100.0, 1000.0], [100.0, 0.0], [300.0, 100.0]),
            MIN_SCALE
        );
    }
}
