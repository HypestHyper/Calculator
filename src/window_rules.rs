//! Window-rule snippets that make the calculator open floating.
//!
//! Wayland has no way for an app to ask to float, so tiling compositors need a
//! one-line rule matching the app ID. No size or position is set: the window
//! opens at the size and place the calculator remembers. (Hyprland opens new
//! floating windows in the middle anyway, so `center` would only override
//! that.)

use crate::config::APP_ID;

pub struct Rule {
    pub key: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub snippet: String,
    pub note: &'static str,
}

pub const ORDER: [&str; 3] = ["hyprland", "niri", "sway"];

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
        "sway" => Some(Rule {
            key: "sway",
            name: "Sway",
            file: "~/.config/sway/config",
            snippet: format!("for_window [app_id=\"^{escaped}$\"] floating enable"),
            note: "Reload with “swaymsg reload”.",
        }),
        "hyprland" => Some(Rule {
            key: "hyprland",
            name: "Hyprland",
            file: "~/.config/hypr/hyprland.lua",
            snippet: format!(
                "hl.window_rule({{\n    name = \"calculator-float\",\n    match = {{ class = \"^{}$\" }},\n    float = true,\n}})",
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
    if set("SWAYSOCK") {
        return Some("sway");
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
            "hl.window_rule({\n    name = \"calculator-float\",\n    match = { class = \"^io\\\\.github\\\\.HypestHyper\\\\.Calculator$\" },\n    float = true,\n})"
        );
        assert_eq!(
            rule("niri").unwrap().snippet,
            "window-rule {\n    match app-id=r#\"^io\\.github\\.HypestHyper\\.Calculator$\"#\n    open-floating true\n}"
        );
        assert_eq!(
            rule("sway").unwrap().snippet,
            "for_window [app_id=\"^io\\.github\\.HypestHyper\\.Calculator$\"] floating enable"
        );
        assert!(rule("kwin").is_none());
    }
}
