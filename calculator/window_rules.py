"""Window-rule snippets that make the calculator open floating.

Wayland has no way for an app to ask to float, so tiling compositors need a
one-line rule matching the app ID. No size is set, so the window opens at the
size the calculator remembers.
"""

import os

from .config import APP_ID

_ESCAPED = APP_ID.replace(".", r"\.")

RULES = {
    "niri": {
        "name": "niri",
        "file": "~/.config/niri/config.kdl",
        "snippet": (
            "window-rule {\n"
            f'    match app-id=r#"^{_ESCAPED}$"#\n'
            "    open-floating true\n"
            "}"
        ),
        "note": "niri reloads its config automatically.",
    },
    "hyprland": {
        "name": "Hyprland",
        "file": "~/.config/hypr/hyprland.lua",
        "snippet": (
            "hl.window_rule({\n"
            '    name = "calculator-float",\n'
            '    match = { class = "^' + _ESCAPED.replace("\\", "\\\\") + '$" },\n'
            "    float = true,\n"
            "    center = true,\n"
            "})"
        ),
        "note": "For Hyprland's Lua config. Reload with “hyprctl reload”.",
    },
}

ORDER = ["hyprland", "niri"]


def detect():
    """Best guess of the running compositor, or None."""
    if os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
        return "hyprland"
    if os.environ.get("NIRI_SOCKET"):
        return "niri"
    desktop = os.environ.get("XDG_CURRENT_DESKTOP", "").lower()
    for key in ORDER:
        if key in desktop:
            return key
    return None


def text_for(key):
    rule = RULES[key]
    return f"# {rule['name']}: add to {rule['file']}\n{rule['snippet']}\n"
