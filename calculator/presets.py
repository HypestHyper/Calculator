"""Built-in colour presets.

Two kinds:
  * Material seeds: one colour plus a palette style; the full Material 3
    palette is generated from it, in light and dark.
  * Themes: hand-mapped from well-known editor and desktop colour schemes.
    Each mode they define gets their real colours; a mode they don't define
    (Dracula has no light theme) is generated from their accent.
"""


def _mix(a, b, t):
    """Blend hex colour a towards b by t (0..1)."""
    a, b = a.lstrip("#"), b.lstrip("#")
    ca = [int(a[i:i + 2], 16) for i in (0, 2, 4)]
    cb = [int(b[i:i + 2], 16) for i in (0, 2, 4)]
    return "#" + "".join(f"{round(x + (y - x) * t):02x}" for x, y in zip(ca, cb))


def _theme(dark, lowest, low, surface, container, high, highest, text, subtext,
           outline, outline_variant, primary, secondary, tertiary, error):
    """Map a theme's colours onto Material 3 roles."""
    ink = lowest if dark else "#ffffff"   # text on accent fills
    roles = {
        "background": surface, "on_background": text,
        "surface": surface, "on_surface": text,
        "surface_variant": highest, "on_surface_variant": subtext,
        "surface_dim": low if dark else highest, "surface_bright": highest if dark else lowest,
        "surface_container_lowest": lowest, "surface_container_low": low,
        "surface_container": container, "surface_container_high": high,
        "surface_container_highest": highest,
        "outline": outline, "outline_variant": outline_variant,
        "inverse_surface": text, "inverse_on_surface": surface,
        "inverse_primary": _mix(primary, surface, 0.45),
        "scrim": "#000000", "shadow": "#000000",
    }
    for name, accent in (("primary", primary), ("secondary", secondary),
                         ("tertiary", tertiary), ("error", error)):
        roles[name] = accent
        roles[f"on_{name}"] = ink
        roles[f"{name}_container"] = _mix(high, accent, 0.30 if dark else 0.22)
        roles[f"on_{name}_container"] = _mix(accent, text, 0.35 if dark else 0.55)
    return roles


PRESETS = {
    # ---- Material seeds ----------------------------------------------------
    "lavender": {"name": "Lavender", "seed": "#6750A4", "style": "tonal_spot"},
    "ocean": {"name": "Ocean", "seed": "#0061A4", "style": "tonal_spot"},
    "forest": {"name": "Forest", "seed": "#386A20", "style": "tonal_spot"},
    "teal": {"name": "Teal", "seed": "#006A6A", "style": "tonal_spot"},
    "rose": {"name": "Rose", "seed": "#C2185B", "style": "tonal_spot"},
    "sunset": {"name": "Sunset", "seed": "#E8590C", "style": "vibrant"},
    "honey": {"name": "Honey", "seed": "#E0A100", "style": "tonal_spot"},
    "candy": {"name": "Candy", "seed": "#9C27B0", "style": "expressive"},
    "graphite": {"name": "Graphite", "seed": "#5F6368", "style": "monochrome"},

    # ---- Themes -------------------------------------------------------------
    "catppuccin": {
        "name": "Catppuccin", "seed": "#8839EF", "style": "tonal_spot",
        "dark": _theme(True, "#11111b", "#181825", "#1e1e2e", "#262637", "#313244", "#45475a",
                       "#cdd6f4", "#a6adc8", "#7f849c", "#45475a",
                       "#cba6f7", "#89b4fa", "#fab387", "#f38ba8"),
        "light": _theme(False, "#ffffff", "#e6e9ef", "#eff1f5", "#e6e9ef", "#dce0e8", "#ccd0da",
                        "#4c4f69", "#6c6f85", "#8c8fa1", "#bcc0cc",
                        "#8839ef", "#1e66f5", "#fe640b", "#d20f39"),
    },
    "nord": {
        "name": "Nord", "seed": "#5E81AC", "style": "tonal_spot",
        "dark": _theme(True, "#242933", "#292e39", "#2e3440", "#353b49", "#3b4252", "#434c5e",
                       "#eceff4", "#d8dee9", "#616e88", "#4c566a",
                       "#88c0d0", "#81a1c1", "#b48ead", "#bf616a"),
        "light": _theme(False, "#ffffff", "#e5e9f0", "#eceff4", "#e5e9f0", "#d8dee9", "#cbd2de",
                        "#2e3440", "#4c566a", "#7b88a1", "#d8dee9",
                        "#5e81ac", "#81a1c1", "#b48ead", "#bf616a"),
    },
    "gruvbox": {
        "name": "Gruvbox", "seed": "#D79921", "style": "tonal_spot",
        "dark": _theme(True, "#1d2021", "#232323", "#282828", "#32302f", "#3c3836", "#504945",
                       "#ebdbb2", "#bdae93", "#928374", "#504945",
                       "#fabd2f", "#83a598", "#fe8019", "#fb4934"),
        "light": _theme(False, "#f9f5d7", "#f2e5bc", "#fbf1c7", "#f5eac3", "#efe1b6", "#e6d5a8",
                        "#3c3836", "#665c54", "#928374", "#d5c4a1",
                        "#b57614", "#076678", "#af3a03", "#9d0006"),
    },
    "tokyo_night": {
        "name": "Tokyo Night", "seed": "#7AA2F7", "style": "tonal_spot",
        "dark": _theme(True, "#13131a", "#16161e", "#1a1b26", "#1f2335", "#292e42", "#343a55",
                       "#c0caf5", "#a9b1d6", "#565f89", "#3b4261",
                       "#7aa2f7", "#bb9af7", "#ff9e64", "#f7768e"),
        "light": _theme(False, "#f2f3f7", "#e9e9ed", "#e1e2e7", "#d8dae3", "#d0d5e3", "#c4c8da",
                        "#3760bf", "#6172b0", "#848cb5", "#c4c8da",
                        "#2e7de9", "#9854f1", "#b15c00", "#f52a65"),
    },
    "rose_pine": {
        "name": "Rosé Pine", "seed": "#907AA9", "style": "tonal_spot",
        "dark": _theme(True, "#121019", "#16141f", "#191724", "#1f1d2e", "#26233a", "#403d52",
                       "#e0def4", "#908caa", "#6e6a86", "#403d52",
                       "#c4a7e7", "#9ccfd8", "#f6c177", "#eb6f92"),
        "light": _theme(False, "#fffaf3", "#f4ede8", "#faf4ed", "#f2e9e1", "#ebe3dc", "#dfdad9",
                        "#575279", "#797593", "#9893a5", "#dfdad9",
                        "#907aa9", "#56949f", "#ea9d34", "#b4637a"),
    },
    "dracula": {
        "name": "Dracula", "seed": "#BD93F9", "style": "vibrant",
        "dark": _theme(True, "#191a21", "#21222c", "#282a36", "#2f3241", "#343746", "#44475a",
                       "#f8f8f2", "#bfbfca", "#6272a4", "#44475a",
                       "#bd93f9", "#ff79c6", "#8be9fd", "#ff5555"),
    },
}

ORDER = list(PRESETS)
