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
/// The largest factor, for big windows.
pub const MAX_SCALE: f64 = 2.0;
/// Factors go in steps of 1%: fine enough to follow the window smoothly.
pub const STEP: f64 = 0.01;
/// Text never gets smaller than this, in px.
pub const MIN_FONT: f64 = 12.0;

/// How much text scales at a given factor. Text shrinks less than the rest,
/// so it stays readable in a small window: at half size it is 70%.
pub fn font_factor(scale: f64) -> f64 {
    if scale >= 1.0 {
        scale
    } else {
        scale + (1.0 - scale) * 0.4
    }
}

/// Smaller text is drawn bolder, so its strokes don't get too thin.
fn font_weight(weight: f64, scale: f64) -> f64 {
    if scale >= 1.0 {
        weight
    } else {
        (weight + (1.0 - scale) * 240.0).min(700.0)
    }
}

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

/// The size rules of `css` with every px length multiplied by `scale`
/// (text by `font_factor`), and text weights adjusted to match. Empty at
/// full size, so nothing changes there.
pub fn scaled_css(css: &str, scale: f64) -> String {
    if (scale - 1.0).abs() < 1e-9 {
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
            if prop == "font-weight" {
                if let (Ok(weight), true) = (value.parse::<f64>(), scale < 1.0) {
                    declarations.push(format!(
                        "font-weight: {};",
                        font_weight(weight, scale).round()
                    ));
                }
                continue;
            }
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
                    new = (px * font_factor(scale)).max(px.min(MIN_FONT));
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
/// in `available` on both axes, from MIN_SCALE to MAX_SCALE.
pub fn choose_scale(available: [f64; 2], fixed: [f64; 2], per_scale: [f64; 2]) -> f64 {
    let mut scale = MAX_SCALE;
    for i in 0..2 {
        if per_scale[i] > 0.0 {
            scale = scale.min((available[i] - fixed[i]) / per_scale[i]);
        }
    }
    let stepped = (scale / STEP + 1e-9).floor() * STEP;
    (stepped.clamp(MIN_SCALE, MAX_SCALE) * 100.0).round() / 100.0
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
        // Sizes halve; text shrinks less (70%) and gets bolder.
        assert!(css.contains("window.calculator button.key { min-height: 32px; min-width: 38px; font-size: 19.6px; font-weight: 570; }"));
        assert!(css.contains("window.calculator .keypad-sheet { border-radius: 18px 18px 0 0; padding: 8px 8px 9px 8px; }"));
        assert!(css.contains("window.calculator .expr { font-size: 40.6px; font-weight: 520; }"));
        // Text has a floor; weights stop at bold.
        assert!(css.contains("window.calculator .prev-line { font-size: 12px; }"));
        assert!(css.contains("window.calculator button.key.clear { font-weight: 700; }"));
        assert!(!css.contains("calc-prefs"));
    }

    #[test]
    fn sizes_grow_in_big_windows() {
        let css = scaled_css(STYLE, 1.5);
        // Everything, text too, grows alike; weights stay.
        assert!(css.contains(
            "window.calculator button.key { min-height: 96px; min-width: 114px; font-size: 42px; }"
        ));
        assert!(!css.contains("font-weight"));
    }

    #[test]
    fn text_shrinks_less_than_the_rest() {
        assert_eq!(font_factor(1.0), 1.0);
        assert_eq!(font_factor(2.0), 2.0);
        assert!((font_factor(0.5) - 0.7).abs() < 1e-9);
    }

    #[test]
    fn scale_fits_the_space() {
        // 100 fixed + 300 per unit of scale: 400 fits at 1, 250 at 0.5.
        let fit = |width| choose_scale([width, 10_000.0], [100.0, 0.0], [300.0, 100.0]);
        assert_eq!(fit(400.0), 1.0);
        assert_eq!(fit(340.0), 0.8);
        assert_eq!(fit(100.0), MIN_SCALE);
        assert_eq!(fit(550.0), 1.5);
        assert_eq!(fit(5000.0), MAX_SCALE);
    }
}
