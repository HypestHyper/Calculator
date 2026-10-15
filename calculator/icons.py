"""Material Symbols icons.

The Material Symbols Rounded variable font (Apache-2.0) ships with the app
and is registered privately with fontconfig, so nothing is installed system
wide. If the font cannot be loaded, icons fall back to symbolic theme icons.
"""

import ctypes
import ctypes.util

from gi.repository import Gtk, PangoCairo

from .config import data_dirs

FONT_FAMILY = "Material Symbols Rounded"
FONT_FILE = "MaterialSymbolsRounded.ttf"

# name -> (codepoint, fallback symbolic icon)
ICONS = {
    "backspace": (0xE14A, "edit-clear-symbolic"),
    "history": (0xE8B3, "document-open-recent-symbolic"),
    "settings": (0xE8B8, "emblem-system-symbolic"),
    "calculate": (0xEA5F, "accessories-calculator-symbolic"),
    "function": (0xF866, "view-more-symbolic"),
    "science": (0xEA4B, "view-more-symbolic"),
    "content_copy": (0xE14D, "edit-copy-symbolic"),
    "delete_sweep": (0xE16C, "user-trash-symbolic"),
    "close": (0xE5CD, "window-close-symbolic"),
    "check": (0xE668, "object-select-symbolic"),
    "palette": (0xE40A, "applications-graphics-symbolic"),
    "history_toggle_off": (0xF17D, "document-open-recent-symbolic"),
    "dark_mode": (0xE51C, "weather-clear-night-symbolic"),
    "light_mode": (0xE518, "weather-clear-symbolic"),
    "contrast": (0xEB37, "display-brightness-symbolic"),
    "animation": (0xE71C, "media-playback-start-symbolic"),
    "tune": (0xE429, "preferences-system-symbolic"),
    "info": (0xE88E, "help-about-symbolic"),
    "desktop_windows": (0xE30C, "video-display-symbolic"),
    "open_in_new": (0xE89E, "document-open-symbolic"),
    "text_fields": (0xE262, "format-text-bold-symbolic"),
    "grid_view": (0xE9B0, "view-grid-symbolic"),
    "square_foot": (0xEA49, "view-grid-symbolic"),
    "replay": (0xE042, "view-refresh-symbolic"),
    "keyboard": (0xE312, "input-keyboard-symbolic"),
    "drag_indicator": (0xE945, "list-drag-handle-symbolic"),
}

_font_ok = None


def load_font():
    """Register the bundled font with fontconfig. Call before any text is drawn."""
    global _font_ok
    if _font_ok is not None:
        return _font_ok
    # Register before Pango builds its font list, so no reload is needed.
    path = next((d / "fonts" / FONT_FILE for d in data_dirs() if (d / "fonts" / FONT_FILE).is_file()), None)
    lib = ctypes.util.find_library("fontconfig")
    if path is not None and lib is not None:
        try:
            fc = ctypes.CDLL(lib)
            fc.FcConfigAppFontAddFile.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
            fc.FcConfigAppFontAddFile.restype = ctypes.c_int
            fc.FcConfigAppFontAddFile(None, str(path).encode())
        except (OSError, AttributeError):
            pass
    _font_ok = _family_available()
    return _font_ok


def _family_available():
    try:
        families = PangoCairo.FontMap.get_default().list_families()
    except Exception:
        return False
    return any(f.get_name() == FONT_FAMILY for f in families)


def icon(name, size=None, css_classes=()):
    """A widget showing the named Material Symbol."""
    codepoint, fallback = ICONS[name]
    if _font_ok:
        widget = Gtk.Label(label=chr(codepoint))
        widget.add_css_class("msym")
    else:
        widget = Gtk.Image.new_from_icon_name(fallback)
        if size:
            widget.set_pixel_size(size)
    for cls in css_classes:
        widget.add_css_class(cls)
    widget.set_can_target(False)
    return widget


def icon_button(name, tooltip, css_classes=("icon-btn",)):
    button = Gtk.Button()
    button.set_child(icon(name, 20))
    button.set_tooltip_text(tooltip)
    for cls in css_classes:
        button.add_css_class(cls)
    return button
