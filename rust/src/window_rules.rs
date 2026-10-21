//! Window-rule snippets that make the calculator open floating.
//! Port of `calculator/window_rules.py`.
//!
//! Wayland has no way for an app to ask to float, so tiling compositors need a
//! one-line rule matching the app ID. No size is set, so the window opens at
//! the size the calculator remembers.

use crate::config::APP_ID;

pub struct Rule {
    pub key: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub snippet: String,
    pub note: &'static str,
}

pub const ORDER: [&str; 2] = ["hyprland", "niri"];

pub fn rule(key: &str) -> Option<Rule> {
    let escaped = APP_ID.replace('.', r"\.");
    match key {
        "niri" => Some(Rule {
            key: "niri",
            name: "niri",
            file: "~/.config/niri/config.kdl",
            snippet: format!("window-rule {{\n    match app-id=r#\"^{escaped}$\"#\n    open-floating true\n}}"),
            note: "niri reloads its config automatically.",
        }),
        "hyprland" => Some(Rule {
            key: "hyprland",
            name: "Hyprland",
            file: "~/.config/hypr/hyprland.lua",
            snippet: format!(
                "hl.window_rule({{\n    name = \"calculator-float\",\n    match = {{ class = \"^{}$\" }},\n    float = true,\n    center = true,\n}})",
                escaped.replace('\\', "\\\\")
            ),
            note: "For Hyprland's Lua config. Reload with “hyprctl reload”.",
        }),
        _ => None,
    }
}

/// Best guess of the running compositor.
pub fn detect() -> Option<&'static str> {
    let set = |var: &str| std::env::var_os(var).is_some_and(|v| !v.is_empty());
    if set("HYPRLAND_INSTANCE_SIGNATURE") {
        return Some("hyprland");
    }
    if set("NIRI_SOCKET") {
        return Some("niri");
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    ORDER.into_iter().find(|key| desktop.contains(key))
}

pub fn text_for(key: &str) -> Option<String> {
    let rule = rule(key)?;
    Some(format!(
        "# {}: add to {}\n{}\n",
        rule.name, rule.file, rule.snippet
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_match_the_readme() {
        assert_eq!(
            rule("hyprland").unwrap().snippet,
            "hl.window_rule({\n    name = \"calculator-float\",\n    match = { class = \"^io\\\\.github\\\\.HypestHyper\\\\.Calculator$\" },\n    float = true,\n    center = true,\n})"
        );
        assert_eq!(
            rule("niri").unwrap().snippet,
            "window-rule {\n    match app-id=r#\"^io\\.github\\.HypestHyper\\.Calculator$\"#\n    open-floating true\n}"
        );
        assert!(rule("sway").is_none());
    }
}
