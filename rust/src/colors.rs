//! Material 3 colours: palette generation, presets and palette.toml.
//!
//! Port of the GTK-free parts of `calculator/theme.py` and
//! `calculator/presets.py`. Palettes come from the vendored material-colors
//! crate with the 2025 colour specification, matching what the Python app
//! gets from materialyoucolor.

use std::collections::HashMap;
use std::path::Path;

use material_colors::color::Rgb;
use material_colors::dynamic_color::{DynamicScheme, Platform, SpecVersion};
use material_colors::hct::Hct;
use material_colors::scheme::variant::{
    SchemeContent, SchemeExpressive, SchemeFidelity, SchemeFruitSalad, SchemeMonochrome,
    SchemeNeutral, SchemeRainbow, SchemeTonalSpot, SchemeVibrant,
};

pub type Seed = (u8, u8, u8);

pub const FALLBACK_SEED: Seed = (0x67, 0x50, 0xA4);

pub const ROLES: [&str; 36] = [
    "primary",
    "on_primary",
    "primary_container",
    "on_primary_container",
    "secondary",
    "on_secondary",
    "secondary_container",
    "on_secondary_container",
    "tertiary",
    "on_tertiary",
    "tertiary_container",
    "on_tertiary_container",
    "error",
    "on_error",
    "error_container",
    "on_error_container",
    "background",
    "on_background",
    "surface",
    "on_surface",
    "surface_variant",
    "on_surface_variant",
    "surface_dim",
    "surface_bright",
    "surface_container_lowest",
    "surface_container_low",
    "surface_container",
    "surface_container_high",
    "surface_container_highest",
    "outline",
    "outline_variant",
    "inverse_surface",
    "inverse_on_surface",
    "inverse_primary",
    "scrim",
    "shadow",
];

/// Palette styles: (key, display name).
pub const STYLES: [(&str, &str); 9] = [
    ("tonal_spot", "Tonal spot"),
    ("vibrant", "Vibrant"),
    ("expressive", "Expressive"),
    ("fidelity", "Fidelity"),
    ("content", "Content"),
    ("neutral", "Neutral"),
    ("monochrome", "Monochrome"),
    ("rainbow", "Rainbow"),
    ("fruit_salad", "Fruit salad"),
];

/// GNOME's accent names (org.gnome.desktop.interface accent-color).
pub fn gnome_accent(name: &str) -> Option<Seed> {
    let hex = match name {
        "blue" => "#3584e4",
        "teal" => "#2190a4",
        "green" => "#3a944a",
        "yellow" => "#c88800",
        "orange" => "#ed5b00",
        "red" => "#e62d42",
        "pink" => "#d56199",
        "purple" => "#9141ac",
        "slate" => "#6f8396",
        _ => return None,
    };
    parse_hex(hex)
}

pub fn contrast_level(name: &str) -> Option<f64> {
    match name {
        "standard" => Some(0.0),
        "medium" => Some(0.5),
        "high" => Some(1.0),
        _ => None,
    }
}

// --------------------------------------------------------------------------
// Colour helpers
// --------------------------------------------------------------------------

/// "#rgb" or "#rrggbb", with or without "#".
pub fn parse_hex(text: &str) -> Option<Seed> {
    let t = text.trim().trim_start_matches('#');
    let t: String = if t.len() == 3 {
        t.chars().flat_map(|c| [c, c]).collect()
    } else {
        t.to_string()
    };
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    Some((byte(0)?, byte(2)?, byte(4)?))
}

/// Python's round(): halves go to the even neighbour.
fn py_round(x: f64) -> f64 {
    x.round_ties_even()
}

pub fn to_hex(rgb: (f64, f64, f64)) -> String {
    let c = |v: f64| py_round(v).clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", c(rgb.0), c(rgb.1), c(rgb.2))
}

pub fn seed_hex(seed: Seed) -> String {
    to_hex((f64::from(seed.0), f64::from(seed.1), f64::from(seed.2)))
}

/// Blend hex colour a towards b by t (0..1).
pub fn mix(a: &str, b: &str, t: f64) -> String {
    let (a, b) = (parse_hex(a).unwrap(), parse_hex(b).unwrap());
    let m = |x: u8, y: u8| py_round(f64::from(x) + (f64::from(y) - f64::from(x)) * t);
    format!(
        "#{:02x}{:02x}{:02x}",
        m(a.0, b.0) as u8,
        m(a.1, b.1) as u8,
        m(a.2, b.2) as u8
    )
}

pub fn luminance(rgb: (f64, f64, f64)) -> f64 {
    let lin = |c: f64| {
        let c = c / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgb.0) + 0.7152 * lin(rgb.1) + 0.0722 * lin(rgb.2)
}

/// "onPrimary" or "on-primary" -> "on_primary".
pub fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.replace('-', "_").chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Colour roles by name. Always holds all 36 roles once generated.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Roles(HashMap<String, String>);

impl Roles {
    pub fn get(&self, role: &str) -> Option<&str> {
        self.0.get(role).map(String::as_str)
    }

    pub fn set(&mut self, role: &str, hex: &str) {
        self.0.insert(role.to_string(), hex.to_string());
    }

    pub fn update(&mut self, other: &Roles) {
        for (role, hex) in &other.0 {
            self.0.insert(role.clone(), hex.clone());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// (role, hex) in the standard role order.
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> {
        ROLES
            .iter()
            .filter_map(|role| self.get(role).map(|hex| (*role, hex)))
    }
}

// --------------------------------------------------------------------------
// Palette generation
// --------------------------------------------------------------------------

fn scheme(seed: Seed, dark: bool, contrast: f64, style: &str) -> DynamicScheme {
    let hct = Hct::new(Rgb::new(seed.0, seed.1, seed.2));
    let (c, spec, platform) = (Some(contrast), SpecVersion::Spec2025, Platform::Phone);
    match style {
        "vibrant" => SchemeVibrant::with_spec(hct, dark, c, spec, platform).scheme,
        "expressive" => SchemeExpressive::with_spec(hct, dark, c, spec, platform).scheme,
        "fidelity" => SchemeFidelity::with_spec(hct, dark, c, spec, platform).scheme,
        "content" => SchemeContent::with_spec(hct, dark, c, spec, platform).scheme,
        "neutral" => SchemeNeutral::with_spec(hct, dark, c, spec, platform).scheme,
        "monochrome" => SchemeMonochrome::with_spec(hct, dark, c, spec, platform).scheme,
        "rainbow" => SchemeRainbow::with_spec(hct, dark, c, spec, platform).scheme,
        "fruit_salad" => SchemeFruitSalad::with_spec(hct, dark, c, spec, platform).scheme,
        _ => SchemeTonalSpot::with_spec(hct, dark, c, spec, platform).scheme,
    }
}

/// A full Material 3 palette from a seed colour.
pub fn generate_palette(seed: Seed, dark: bool, contrast: f64, style: &str) -> Roles {
    let s = scheme(seed, dark, contrast, style);
    let colors: [Rgb; 36] = [
        s.primary(),
        s.on_primary(),
        s.primary_container(),
        s.on_primary_container(),
        s.secondary(),
        s.on_secondary(),
        s.secondary_container(),
        s.on_secondary_container(),
        s.tertiary(),
        s.on_tertiary(),
        s.tertiary_container(),
        s.on_tertiary_container(),
        s.error(),
        s.on_error(),
        s.error_container(),
        s.on_error_container(),
        s.background(),
        s.on_background(),
        s.surface(),
        s.on_surface(),
        s.surface_variant(),
        s.on_surface_variant(),
        s.surface_dim(),
        s.surface_bright(),
        s.surface_container_lowest(),
        s.surface_container_low(),
        s.surface_container(),
        s.surface_container_high(),
        s.surface_container_highest(),
        s.outline(),
        s.outline_variant(),
        s.inverse_surface(),
        s.inverse_on_surface(),
        s.inverse_primary(),
        s.scrim(),
        s.shadow(),
    ];
    let mut roles = Roles::default();
    for (role, rgb) in ROLES.iter().zip(colors) {
        roles.set(
            role,
            &format!("#{:02x}{:02x}{:02x}", rgb.red, rgb.green, rgb.blue),
        );
    }
    roles
}

// --------------------------------------------------------------------------
// Presets
// --------------------------------------------------------------------------

pub struct Preset {
    pub key: &'static str,
    pub name: &'static str,
    pub seed: &'static str,
    pub style: &'static str,
    /// Hand-mapped theme colours for (light, dark); None means generated.
    pub light: Option<[&'static str; 14]>,
    pub dark: Option<[&'static str; 14]>,
}

impl Preset {
    /// The preset's own roles for a mode, if it defines that mode.
    pub fn roles(&self, dark: bool) -> Option<Roles> {
        let colors = if dark { self.dark } else { self.light }?;
        Some(theme_roles(dark, &colors))
    }
}

/// Map a theme's colours onto Material 3 roles: lowest, low, surface,
/// container, high, highest, text, subtext, outline, outline_variant,
/// primary, secondary, tertiary, error.
fn theme_roles(dark: bool, c: &[&str; 14]) -> Roles {
    let [lowest, low, surface, container, high, highest, text, subtext, outline, outline_variant, primary, secondary, tertiary, error] =
        *c;
    let ink = if dark { lowest } else { "#ffffff" }; // text on accent fills
    let mut roles = Roles::default();
    let pairs = [
        ("background", surface),
        ("on_background", text),
        ("surface", surface),
        ("on_surface", text),
        ("surface_variant", highest),
        ("on_surface_variant", subtext),
        ("surface_dim", if dark { low } else { highest }),
        ("surface_bright", if dark { highest } else { lowest }),
        ("surface_container_lowest", lowest),
        ("surface_container_low", low),
        ("surface_container", container),
        ("surface_container_high", high),
        ("surface_container_highest", highest),
        ("outline", outline),
        ("outline_variant", outline_variant),
        ("inverse_surface", text),
        ("inverse_on_surface", surface),
        ("scrim", "#000000"),
        ("shadow", "#000000"),
    ];
    for (role, hex) in pairs {
        roles.set(role, hex);
    }
    roles.set("inverse_primary", &mix(primary, surface, 0.45));
    for (name, accent) in [
        ("primary", primary),
        ("secondary", secondary),
        ("tertiary", tertiary),
        ("error", error),
    ] {
        roles.set(name, accent);
        roles.set(&format!("on_{name}"), ink);
        roles.set(
            &format!("{name}_container"),
            &mix(high, accent, if dark { 0.30 } else { 0.22 }),
        );
        roles.set(
            &format!("on_{name}_container"),
            &mix(accent, text, if dark { 0.35 } else { 0.55 }),
        );
    }
    roles
}

const fn seed_preset(
    key: &'static str,
    name: &'static str,
    seed: &'static str,
    style: &'static str,
) -> Preset {
    Preset {
        key,
        name,
        seed,
        style,
        light: None,
        dark: None,
    }
}

pub const PRESETS: [Preset; 15] = [
    // ---- Material seeds ----------------------------------------------------
    seed_preset("lavender", "Lavender", "#6750A4", "tonal_spot"),
    seed_preset("ocean", "Ocean", "#0061A4", "tonal_spot"),
    seed_preset("forest", "Forest", "#386A20", "tonal_spot"),
    seed_preset("teal", "Teal", "#006A6A", "tonal_spot"),
    seed_preset("rose", "Rose", "#C2185B", "tonal_spot"),
    seed_preset("sunset", "Sunset", "#E8590C", "vibrant"),
    seed_preset("honey", "Honey", "#E0A100", "tonal_spot"),
    seed_preset("candy", "Candy", "#9C27B0", "expressive"),
    seed_preset("graphite", "Graphite", "#5F6368", "monochrome"),
    // ---- Themes -------------------------------------------------------------
    Preset {
        key: "catppuccin",
        name: "Catppuccin",
        seed: "#8839EF",
        style: "tonal_spot",
        dark: Some([
            "#11111b", "#181825", "#1e1e2e", "#262637", "#313244", "#45475a", "#cdd6f4", "#a6adc8",
            "#7f849c", "#45475a", "#cba6f7", "#89b4fa", "#fab387", "#f38ba8",
        ]),
        light: Some([
            "#ffffff", "#e6e9ef", "#eff1f5", "#e6e9ef", "#dce0e8", "#ccd0da", "#4c4f69", "#6c6f85",
            "#8c8fa1", "#bcc0cc", "#8839ef", "#1e66f5", "#fe640b", "#d20f39",
        ]),
    },
    Preset {
        key: "nord",
        name: "Nord",
        seed: "#5E81AC",
        style: "tonal_spot",
        dark: Some([
            "#242933", "#292e39", "#2e3440", "#353b49", "#3b4252", "#434c5e", "#eceff4", "#d8dee9",
            "#616e88", "#4c566a", "#88c0d0", "#81a1c1", "#b48ead", "#bf616a",
        ]),
        light: Some([
            "#ffffff", "#e5e9f0", "#eceff4", "#e5e9f0", "#d8dee9", "#cbd2de", "#2e3440", "#4c566a",
            "#7b88a1", "#d8dee9", "#5e81ac", "#81a1c1", "#b48ead", "#bf616a",
        ]),
    },
    Preset {
        key: "gruvbox",
        name: "Gruvbox",
        seed: "#D79921",
        style: "tonal_spot",
        dark: Some([
            "#1d2021", "#232323", "#282828", "#32302f", "#3c3836", "#504945", "#ebdbb2", "#bdae93",
            "#928374", "#504945", "#fabd2f", "#83a598", "#fe8019", "#fb4934",
        ]),
        light: Some([
            "#f9f5d7", "#f2e5bc", "#fbf1c7", "#f5eac3", "#efe1b6", "#e6d5a8", "#3c3836", "#665c54",
            "#928374", "#d5c4a1", "#b57614", "#076678", "#af3a03", "#9d0006",
        ]),
    },
    Preset {
        key: "tokyo_night",
        name: "Tokyo Night",
        seed: "#7AA2F7",
        style: "tonal_spot",
        dark: Some([
            "#13131a", "#16161e", "#1a1b26", "#1f2335", "#292e42", "#343a55", "#c0caf5", "#a9b1d6",
            "#565f89", "#3b4261", "#7aa2f7", "#bb9af7", "#ff9e64", "#f7768e",
        ]),
        light: Some([
            "#f2f3f7", "#e9e9ed", "#e1e2e7", "#d8dae3", "#d0d5e3", "#c4c8da", "#3760bf", "#6172b0",
            "#848cb5", "#c4c8da", "#2e7de9", "#9854f1", "#b15c00", "#f52a65",
        ]),
    },
    Preset {
        key: "rose_pine",
        name: "Rosé Pine",
        seed: "#907AA9",
        style: "tonal_spot",
        dark: Some([
            "#121019", "#16141f", "#191724", "#1f1d2e", "#26233a", "#403d52", "#e0def4", "#908caa",
            "#6e6a86", "#403d52", "#c4a7e7", "#9ccfd8", "#f6c177", "#eb6f92",
        ]),
        light: Some([
            "#fffaf3", "#f4ede8", "#faf4ed", "#f2e9e1", "#ebe3dc", "#dfdad9", "#575279", "#797593",
            "#9893a5", "#dfdad9", "#907aa9", "#56949f", "#ea9d34", "#b4637a",
        ]),
    },
    Preset {
        key: "dracula",
        name: "Dracula",
        seed: "#BD93F9",
        style: "vibrant",
        dark: Some([
            "#191a21", "#21222c", "#282a36", "#2f3241", "#343746", "#44475a", "#f8f8f2", "#bfbfca",
            "#6272a4", "#44475a", "#bd93f9", "#ff79c6", "#8be9fd", "#ff5555",
        ]),
        light: None,
    },
];

pub fn preset(key: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.key == key)
}

// --------------------------------------------------------------------------
// palette.toml
// --------------------------------------------------------------------------

#[derive(Debug, Default, Clone, PartialEq)]
pub struct PaletteFile {
    pub seed: Option<Seed>,
    pub style: Option<String>,
    pub mode: Option<String>,
    pub roles: Roles,
    pub light: Roles,
    pub dark: Roles,
}

/// Read palette.toml. None when the file is missing or unreadable; problems
/// with single entries are reported and the entry skipped.
pub fn read_palette_file(path: &Path) -> Option<PaletteFile> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            eprintln!("calculator: palette.toml ignored: {err}");
            return None;
        }
    };
    parse_palette(&text)
}

pub fn parse_palette(text: &str) -> Option<PaletteFile> {
    let data: toml::Table = match text.parse() {
        Ok(data) => data,
        Err(err) => {
            eprintln!("calculator: palette.toml ignored: {err}");
            return None;
        }
    };
    let mut result = PaletteFile::default();
    if let Some(seed) = data.get("seed") {
        result.seed = seed.as_str().and_then(parse_hex);
        if result.seed.is_none() {
            eprintln!("calculator: palette.toml: invalid seed {seed}");
        }
    }
    if let Some(style) = data.get("style").and_then(|v| v.as_str()) {
        if STYLES.iter().any(|(key, _)| *key == style) {
            result.style = Some(style.to_string());
        }
    }
    if let Some(mode) = data.get("mode").and_then(|v| v.as_str()) {
        if mode == "light" || mode == "dark" {
            result.mode = Some(mode.to_string());
        }
    }
    for table in ["roles", "light", "dark"] {
        let Some(entries) = data.get(table).and_then(|v| v.as_table()) else {
            continue;
        };
        for (key, value) in entries {
            let role = snake(key);
            let rgb = value.as_str().and_then(parse_hex);
            if !ROLES.contains(&role.as_str()) {
                eprintln!("calculator: palette.toml: unknown role {key:?}");
            } else if let Some(rgb) = rgb {
                let target = match table {
                    "roles" => &mut result.roles,
                    "light" => &mut result.light,
                    _ => &mut result.dark,
                };
                target.set(&role, &seed_hex(rgb));
            } else {
                eprintln!("calculator: palette.toml: invalid colour for {key}: {value}");
            }
        }
    }
    Some(result)
}

/// A palette.toml with the current colours as commented-out overrides.
pub fn palette_template(roles: &Roles, seed: Seed, style: &str, dark: bool) -> String {
    let mode = if dark { "dark" } else { "light" };
    let styles: Vec<&str> = STYLES.iter().map(|(key, _)| *key).collect();
    let mut lines = vec![
        "# Calculator custom palette".to_string(),
        "# Used when Preferences > Colours > Source is set to “Palette file”.".to_string(),
        "# Every entry is optional. Colours are #RRGGBB.".to_string(),
        String::new(),
        format!(
            "seed = \"{}\"     # palette is generated from this colour",
            seed_hex(seed)
        ),
        format!("style = \"{style}\"     # {}", styles.join(", ")),
        "# mode = \"dark\"        # force \"light\" or \"dark\"; omit to follow Preferences"
            .to_string(),
        String::new(),
        "# Override individual roles. [roles] applies to both modes;".to_string(),
        "# [light] and [dark] apply to one mode only. Uncomment to use.".to_string(),
        "[roles]".to_string(),
        String::new(),
        format!("[{mode}]"),
    ];
    for (role, hex) in roles.iter() {
        lines.push(format!("# {role} = \"{hex}\""));
    }
    let other = if dark { "light" } else { "dark" };
    lines.extend([String::new(), format!("[{other}]"), String::new()]);
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_helpers_match_python() {
        assert_eq!(parse_hex("#6750A4"), Some((0x67, 0x50, 0xa4)));
        assert_eq!(parse_hex("abc"), Some((0xaa, 0xbb, 0xcc)));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#gggggg"), None);
        assert_eq!(to_hex((0.5, 1.5, 255.6)), "#0002ff");
        assert_eq!(mix("#000000", "#ffffff", 0.5), "#808080");
        assert_eq!(snake("onPrimary"), "on_primary");
        assert_eq!(snake("on-primary"), "on_primary");
    }

    #[test]
    fn default_palette_matches_materialyoucolor() {
        let roles = generate_palette(FALLBACK_SEED, false, 0.0, "tonal_spot");
        assert_eq!(roles.get("primary"), Some("#655789"));
        assert_eq!(roles.get("surface_container_low"), Some("#f8f1fa"));
        let dark = generate_palette(FALLBACK_SEED, true, 0.0, "tonal_spot");
        assert_eq!(dark.get("primary"), Some("#cdc0ec"));
        assert_eq!(dark.iter().count(), 36);
    }

    #[test]
    fn palette_file_parsing() {
        let file = parse_palette(
            "seed = \"#2e7d32\"\nstyle = \"vibrant\"\nmode = \"dark\"\n[roles]\nonPrimary = \"#fff\"\nbogus = \"#000000\"\n[dark]\nsurface = \"nope\"\n",
        )
        .unwrap();
        assert_eq!(file.seed, Some((0x2e, 0x7d, 0x32)));
        assert_eq!(file.style.as_deref(), Some("vibrant"));
        assert_eq!(file.mode.as_deref(), Some("dark"));
        assert_eq!(file.roles.get("on_primary"), Some("#ffffff"));
        assert!(file.dark.is_empty());
        assert!(parse_palette("seed = ").is_none());
    }
}
