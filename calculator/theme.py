"""Desktop theme detection, Material 3 palette generation and CSS loading.

Accent and dark/light are read from standard sources only, in this order:
  1. XDG Desktop Portal  (org.freedesktop.appearance)
  2. GSettings           (org.gnome.desktop.interface), when the schema exists
  3. GTK settings        (settings.ini / XSettings via Gtk.Settings)
  4. Named colours of the active GTK theme
  5. Built-in fallback
User choices in Preferences or palette.toml take priority over all of these.
"""

import math
import re
import tomllib
import warnings

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

from .config import PACKAGE_DIR, PALETTE_FILE  # noqa: E402
from .presets import PRESETS  # noqa: E402

FALLBACK_SEED = (0x67, 0x50, 0xA4)

# GNOME's accent names (org.gnome.desktop.interface accent-color).
GNOME_ACCENTS = {
    "blue": "#3584e4", "teal": "#2190a4", "green": "#3a944a",
    "yellow": "#c88800", "orange": "#ed5b00", "red": "#e62d42",
    "pink": "#d56199", "purple": "#9141ac", "slate": "#6f8396",
}

ROLES = [
    "primary", "on_primary", "primary_container", "on_primary_container",
    "secondary", "on_secondary", "secondary_container", "on_secondary_container",
    "tertiary", "on_tertiary", "tertiary_container", "on_tertiary_container",
    "error", "on_error", "error_container", "on_error_container",
    "background", "on_background",
    "surface", "on_surface", "surface_variant", "on_surface_variant",
    "surface_dim", "surface_bright",
    "surface_container_lowest", "surface_container_low", "surface_container",
    "surface_container_high", "surface_container_highest",
    "outline", "outline_variant", "inverse_surface", "inverse_on_surface",
    "inverse_primary", "scrim", "shadow",
]

STYLE_NAMES = {
    "tonal_spot": "Tonal spot", "vibrant": "Vibrant", "expressive": "Expressive",
    "fidelity": "Fidelity", "content": "Content", "neutral": "Neutral",
    "monochrome": "Monochrome", "rainbow": "Rainbow", "fruit_salad": "Fruit salad",
}

CONTRAST_LEVELS = {"standard": 0.0, "medium": 0.5, "high": 1.0}


# --------------------------------------------------------------------------
# Colour helpers
# --------------------------------------------------------------------------

def parse_hex(text):
    if not isinstance(text, str):
        return None
    t = text.strip().lstrip("#")
    if len(t) == 3:
        t = "".join(c * 2 for c in t)
    if len(t) != 6 or not re.fullmatch(r"[0-9a-fA-F]{6}", t):
        return None
    return tuple(int(t[i:i + 2], 16) for i in (0, 2, 4))


def to_hex(rgb):
    return "#{:02x}{:02x}{:02x}".format(*(max(0, min(255, round(c))) for c in rgb))


def _snake(name):
    name = name.replace("-", "_")
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def _camel(name):
    head, *rest = name.split("_")
    return head + "".join(p.capitalize() for p in rest)


def luminance(rgb):
    def lin(c):
        c /= 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (lin(c) for c in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


# --------------------------------------------------------------------------
# Palette generation
# --------------------------------------------------------------------------

def _generate_with_library(seed, dark, contrast, style):
    import importlib
    from materialyoucolor.dynamiccolor.material_dynamic_colors import MaterialDynamicColors
    from materialyoucolor.hct import Hct

    cls_name = "Scheme" + "".join(p.capitalize() for p in style.split("_"))
    module = importlib.import_module(f"materialyoucolor.scheme.scheme_{style}")
    argb = 0xFF000000 | (seed[0] << 16) | (seed[1] << 8) | seed[2]
    scheme = getattr(module, cls_name)(Hct.from_int(argb), dark, contrast, spec_version="2025")
    colors = MaterialDynamicColors(spec="2025")
    out = {}
    for role in ROLES:
        dyn = getattr(colors, _camel(role), None)
        if dyn is None:
            continue
        value = dyn.get_argb(scheme)
        out[role] = to_hex(((value >> 16) & 255, (value >> 8) & 255, value & 255))
    return out


# Fallback when materialyoucolor is missing: CIE LCh tones (HCT's tone is L*).
def _lab_to_rgb(L, a, b):
    fy = (L + 16) / 116
    fx, fz = fy + a / 500, fy - b / 200

    def finv(t):
        return t ** 3 if t ** 3 > 0.008856 else (t - 16 / 116) / 7.787

    X, Y, Z = 0.95047 * finv(fx), 1.0 * finv(fy), 1.08883 * finv(fz)
    rl = 3.2406 * X - 1.5372 * Y - 0.4986 * Z
    gl = -0.9689 * X + 1.8758 * Y + 0.0415 * Z
    bl = 0.0557 * X - 0.2040 * Y + 1.0570 * Z

    def gamma(c):
        return 12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055
    return [gamma(c) * 255 for c in (rl, gl, bl)]


def _rgb_to_lch(rgb):
    def lin(c):
        c /= 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = (lin(c) for c in rgb)
    X = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047
    Y = 0.2126 * r + 0.7152 * g + 0.0722 * b
    Z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883

    def f(t):
        return t ** (1 / 3) if t > 0.008856 else 7.787 * t + 16 / 116
    L = 116 * f(Y) - 16
    a, bb = 500 * (f(X) - f(Y)), 200 * (f(Y) - f(Z))
    return L, math.hypot(a, bb), math.degrees(math.atan2(bb, a)) % 360


def _tone(hue, chroma, tone):
    # Reduce chroma until the colour fits in sRGB.
    for _ in range(40):
        a = chroma * math.cos(math.radians(hue))
        b = chroma * math.sin(math.radians(hue))
        rgb = _lab_to_rgb(tone, a, b)
        if all(-0.5 <= c <= 255.5 for c in rgb):
            return to_hex(rgb)
        chroma *= 0.85
    return to_hex(_lab_to_rgb(tone, 0, 0))


_FALLBACK_STYLE = {  # (primary, secondary, tertiary hue shift, tertiary, neutral, neutral variant)
    "tonal_spot": (48, 16, 60, 24, 4, 8),
    "vibrant": (80, 28, 60, 36, 10, 12),
    "expressive": (64, 24, 120, 32, 8, 12),
    "neutral": (16, 8, 60, 12, 2, 2),
    "monochrome": (0, 0, 0, 0, 0, 0),
}


def _generate_fallback(seed, dark, contrast, style):
    _, seed_chroma, hue = _rgb_to_lch(seed)
    pc, sc, shift, tc, nc, nvc = _FALLBACK_STYLE.get(style, _FALLBACK_STYLE["tonal_spot"])
    if style in ("fidelity", "content"):
        pc = max(seed_chroma, 30)
    pals = {
        "p": (hue, pc), "s": (hue, sc), "t": ((hue + shift) % 360, tc),
        "n": (hue, nc), "nv": (hue, nvc), "e": (25, 70),
    }

    def t(pal, tone):
        h, c = pals[pal]
        return _tone(h, c, tone)

    k = contrast * 10  # push foreground tones further apart at higher contrast
    if dark:
        spec = {
            "primary": ("p", 80), "on_primary": ("p", 20 - k), "primary_container": ("p", 30),
            "on_primary_container": ("p", 90 + k / 2),
            "secondary": ("s", 80), "on_secondary": ("s", 20 - k), "secondary_container": ("s", 30),
            "on_secondary_container": ("s", 90 + k / 2),
            "tertiary": ("t", 80), "on_tertiary": ("t", 20 - k), "tertiary_container": ("t", 30),
            "on_tertiary_container": ("t", 90 + k / 2),
            "error": ("e", 80), "on_error": ("e", 20), "error_container": ("e", 30),
            "on_error_container": ("e", 90),
            "background": ("n", 6), "on_background": ("n", 90), "surface": ("n", 6),
            "on_surface": ("n", 90 + k), "surface_variant": ("nv", 30),
            "on_surface_variant": ("nv", 80 + k), "surface_dim": ("n", 6), "surface_bright": ("n", 24),
            "surface_container_lowest": ("n", 4), "surface_container_low": ("n", 10),
            "surface_container": ("n", 12), "surface_container_high": ("n", 17),
            "surface_container_highest": ("n", 22), "outline": ("nv", 60 + k),
            "outline_variant": ("nv", 30), "inverse_surface": ("n", 90),
            "inverse_on_surface": ("n", 20), "inverse_primary": ("p", 40),
            "scrim": ("n", 0), "shadow": ("n", 0),
        }
    else:
        spec = {
            "primary": ("p", 40 - k), "on_primary": ("p", 100), "primary_container": ("p", 90),
            "on_primary_container": ("p", 10 - k / 2),
            "secondary": ("s", 40 - k), "on_secondary": ("s", 100), "secondary_container": ("s", 90),
            "on_secondary_container": ("s", 10 - k / 2),
            "tertiary": ("t", 40 - k), "on_tertiary": ("t", 100), "tertiary_container": ("t", 90),
            "on_tertiary_container": ("t", 10 - k / 2),
            "error": ("e", 40), "on_error": ("e", 100), "error_container": ("e", 90),
            "on_error_container": ("e", 10),
            "background": ("n", 98), "on_background": ("n", 10), "surface": ("n", 98),
            "on_surface": ("n", 10 - k), "surface_variant": ("nv", 90),
            "on_surface_variant": ("nv", 30 - k), "surface_dim": ("n", 87), "surface_bright": ("n", 98),
            "surface_container_lowest": ("n", 100), "surface_container_low": ("n", 96),
            "surface_container": ("n", 94), "surface_container_high": ("n", 92),
            "surface_container_highest": ("n", 90), "outline": ("nv", 50 - k),
            "outline_variant": ("nv", 80), "inverse_surface": ("n", 20),
            "inverse_on_surface": ("n", 95), "inverse_primary": ("p", 80),
            "scrim": ("n", 0), "shadow": ("n", 0),
        }
    return {role: t(pal, max(0, min(100, tone))) for role, (pal, tone) in spec.items()}


def generate_palette(seed, dark, contrast=0.0, style="tonal_spot"):
    try:
        return _generate_with_library(seed, dark, contrast, style), "materialyoucolor"
    except ImportError:
        return _generate_fallback(seed, dark, contrast, style), "built-in"
    except Exception as err:  # unexpected library behaviour: never break the app
        print(f"calculator: palette generation failed ({err}); using built-in generator")
        return _generate_fallback(seed, dark, contrast, style), "built-in"


# --------------------------------------------------------------------------
# palette.toml
# --------------------------------------------------------------------------

def read_palette_file(path=PALETTE_FILE):
    """Returns dict(seed, style, mode, roles, light, dark) or None."""
    try:
        with open(path, "rb") as fh:
            data = tomllib.load(fh)
    except FileNotFoundError:
        return None
    except (OSError, tomllib.TOMLDecodeError) as err:
        print(f"calculator: palette.toml ignored: {err}")
        return None

    result = {"seed": None, "style": None, "mode": None, "roles": {}, "light": {}, "dark": {}}
    seed = parse_hex(data.get("seed"))
    if data.get("seed") is not None and seed is None:
        print(f"calculator: palette.toml: invalid seed {data.get('seed')!r}")
    result["seed"] = seed
    if data.get("style") in STYLE_NAMES:
        result["style"] = data["style"]
    if data.get("mode") in ("light", "dark"):
        result["mode"] = data["mode"]
    for table in ("roles", "light", "dark"):
        for key, value in (data.get(table) or {}).items():
            role = _snake(key)
            rgb = parse_hex(value)
            if role not in ROLES:
                print(f"calculator: palette.toml: unknown role {key!r}")
            elif rgb is None:
                print(f"calculator: palette.toml: invalid colour for {key}: {value!r}")
            else:
                result[table][role] = to_hex(rgb)
    return result


def palette_template(roles, seed, style, dark):
    mode = "dark" if dark else "light"
    lines = [
        "# Calculator custom palette",
        "# Used when Preferences > Colours > Source is set to “Palette file”.",
        "# Every entry is optional. Colours are #RRGGBB.",
        "",
        f'seed = "{to_hex(seed)}"     # palette is generated from this colour',
        f'style = "{style}"     # ' + ", ".join(STYLE_NAMES),
        '# mode = "dark"        # force "light" or "dark"; omit to follow Preferences',
        "",
        "# Override individual roles. [roles] applies to both modes;",
        "# [light] and [dark] apply to one mode only. Uncomment to use.",
        "[roles]",
        "",
        f"[{mode}]",
    ]
    for role in ROLES:
        if role in roles:
            lines.append(f'# {role} = "{roles[role]}"')
    other = "light" if dark else "dark"
    lines += ["", f"[{other}]", ""]
    return "\n".join(lines)


# --------------------------------------------------------------------------
# Desktop settings
# --------------------------------------------------------------------------

class DesktopSettings:
    """Watches the desktop's colour scheme, accent, contrast and motion."""

    def __init__(self, on_change):
        self._on_change = on_change
        self.portal = {}
        self._portal_proxy = None
        self._gsettings = None
        self._gsettings_keys = set()
        gtk_settings = Gtk.Settings.get_default()
        # Read before we start setting prefer-dark ourselves.
        self._initial_prefer_dark = bool(gtk_settings.props.gtk_application_prefer_dark_theme)
        self._read_theme_colors()
        self._setup_portal()
        self._setup_gsettings()
        gtk_settings.connect("notify::gtk-theme-name", self._on_theme_name)
        gtk_settings.connect("notify::gtk-enable-animations", lambda *_: self._on_change())

    def _read_theme_colors(self):
        # Cached: once we set prefer-dark ourselves the theme's colours follow us.
        self._theme_bg = self._theme_color("window_bg_color", "theme_bg_color")
        self._theme_accent = self._theme_color(
            "accent_bg_color", "accent_color", "theme_selected_bg_color")

    def _on_theme_name(self, *_):
        self._read_theme_colors()
        self._on_change()

    # Portal --------------------------------------------------------------
    def _setup_portal(self):
        try:
            proxy = Gio.DBusProxy.new_for_bus_sync(
                Gio.BusType.SESSION, Gio.DBusProxyFlags.DO_NOT_LOAD_PROPERTIES, None,
                "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings", None)
            reply = proxy.call_sync(
                "ReadAll", GLib.Variant("(as)", (["org.freedesktop.appearance"],)),
                Gio.DBusCallFlags.NONE, 1500, None)
            data = reply.unpack()[0]
            self.portal = dict(data.get("org.freedesktop.appearance", {}))
            proxy.connect("g-signal", self._on_portal_signal)
            self._portal_proxy = proxy
        except GLib.Error:
            self.portal = {}

    def _on_portal_signal(self, _proxy, _sender, signal, params):
        if signal != "SettingChanged":
            return
        namespace, key, value = params.unpack()
        if namespace == "org.freedesktop.appearance":
            self.portal[key] = value
            self._on_change()

    # GSettings -----------------------------------------------------------
    def _setup_gsettings(self):
        source = Gio.SettingsSchemaSource.get_default()
        schema = source.lookup("org.gnome.desktop.interface", True) if source else None
        if schema is None:
            return
        self._gsettings = Gio.Settings.new("org.gnome.desktop.interface")
        self._gsettings_keys = {
            k for k in ("color-scheme", "accent-color", "gtk-theme", "enable-animations")
            if schema.has_key(k)
        }
        self._gsettings.connect("changed", self._on_gsettings_changed)

    def _on_gsettings_changed(self, _settings, key):
        if key in self._gsettings_keys:
            self._on_change()

    def _gs(self, key):
        if self._gsettings is None or key not in self._gsettings_keys:
            return None
        return self._gsettings.get_value(key).unpack()

    # Queries -------------------------------------------------------------
    def dark(self):
        """(is_dark, source)"""
        scheme = self.portal.get("color-scheme")
        if scheme in (1, 2):
            return scheme == 1, "portal"
        value = self._gs("color-scheme")
        if value in ("prefer-dark", "prefer-light"):
            return value == "prefer-dark", "GSettings"
        if self._initial_prefer_dark:
            return True, "GTK settings"
        theme_name = (Gtk.Settings.get_default().props.gtk_theme_name or "").lower()
        if theme_name.endswith("-dark") or ":dark" in theme_name:
            return True, "GTK theme"
        if self._theme_bg is not None:
            return luminance(self._theme_bg) < 0.2, "GTK theme"
        return False, "default"

    def accent(self):
        """(rgb or None, source)"""
        acc = self.portal.get("accent-color")
        if isinstance(acc, tuple) and len(acc) == 3 and all(0.0 <= c <= 1.0 for c in acc):
            return tuple(round(c * 255) for c in acc), "portal"
        name = self._gs("accent-color")
        if name in GNOME_ACCENTS:
            return parse_hex(GNOME_ACCENTS[name]), "GSettings"
        if self._theme_accent is not None:
            return tuple(round(c) for c in self._theme_accent), "GTK theme"
        return None, "default"

    def high_contrast(self):
        return self.portal.get("contrast") == 1

    def reduced_motion(self):
        if self.portal.get("reduced-motion") == 1:
            return True
        if self._gs("enable-animations") is False:
            return True
        return not Gtk.Settings.get_default().props.gtk_enable_animations

    def _theme_color(self, *names):
        widget = Gtk.Label()
        ctx = widget.get_style_context()
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", DeprecationWarning)
            for name in names:
                found, rgba = ctx.lookup_color(name)
                if found and rgba.alpha > 0.5:
                    return (rgba.red * 255, rgba.green * 255, rgba.blue * 255)
        return None


# --------------------------------------------------------------------------
# Theme manager
# --------------------------------------------------------------------------

class ThemeManager:
    def __init__(self, settings):
        self.settings = settings
        self.desktop = DesktopSettings(self.queue_update)
        self.provider = Gtk.CssProvider()
        self._css_template = (PACKAGE_DIR / "style.css").read_text(encoding="utf-8")
        self._pending = False
        self._listeners = []
        self.roles = {}
        self.dark = False
        self.seed = FALLBACK_SEED
        self.style = "tonal_spot"
        self.info = {}
        display = Gdk.Display.get_default()
        Gtk.StyleContext.add_provider_for_display(
            display, self.provider, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self._palette_monitor = self._watch_palette()
        settings.connect(self._on_setting)
        self.update()

    def connect(self, callback):
        self._listeners.append(callback)

    def _watch_palette(self):
        try:
            PALETTE_FILE.parent.mkdir(parents=True, exist_ok=True)
            monitor = Gio.File.new_for_path(str(PALETTE_FILE)).monitor_file(
                Gio.FileMonitorFlags.WATCH_MOVES, None)
            monitor.connect("changed", lambda *_: self.queue_update())
            return monitor
        except (GLib.Error, OSError):
            return None

    def _on_setting(self, key, _value):
        if key in ("theme", "color_source", "preset", "custom_accent", "palette_style", "contrast"):
            self.queue_update()

    def queue_update(self):
        if not self._pending:
            self._pending = True
            GLib.timeout_add(60, self._do_update)

    def _do_update(self):
        self._pending = False
        self.update()
        return GLib.SOURCE_REMOVE

    def compute(self):
        s = self.settings
        palette = read_palette_file() if s["color_source"] == "palette" else None
        info = {}

        # Light or dark
        if palette and palette["mode"]:
            dark, info["mode"] = palette["mode"] == "dark", "palette file"
        elif s["theme"] != "system":
            dark, info["mode"] = s["theme"] == "dark", "Preferences"
        else:
            dark, info["mode"] = self.desktop.dark()

        preset = PRESETS[s["preset"]] if s["color_source"] == "preset" else None

        # Seed colour
        seed = None
        if preset:
            seed, info["accent"] = parse_hex(preset["seed"]), f"preset “{preset['name']}”"
        elif s["color_source"] == "custom":
            seed, info["accent"] = parse_hex(s["custom_accent"]), "Preferences"
        elif palette and palette["seed"]:
            seed, info["accent"] = palette["seed"], "palette file"
        if seed is None:
            seed, info["accent"] = self.desktop.accent()
        if seed is None:
            seed, info["accent"] = FALLBACK_SEED, "default"

        style = (palette and palette["style"]) or (preset and preset["style"]) or s["palette_style"]
        if s["contrast"] == "system":
            contrast = 1.0 if self.desktop.high_contrast() else 0.0
        else:
            contrast = CONTRAST_LEVELS[s["contrast"]]

        roles, info["generator"] = generate_palette(seed, dark, contrast, style)
        if preset and preset.get("dark" if dark else "light"):
            roles.update(preset["dark" if dark else "light"])
            info["generator"] = f"the {preset['name']} theme"
        if palette:
            roles.update(palette["roles"])
            roles.update(palette["dark" if dark else "light"])
        return roles, dark, seed, style, info

    def update(self):
        self.roles, self.dark, self.seed, self.style, self.info = self.compute()
        defines = "".join(f"@define-color {k} {v};\n" for k, v in self.roles.items())
        css = defines + self._css_template
        if hasattr(self.provider, "load_from_string"):
            self.provider.load_from_string(css)
        else:  # GTK < 4.12
            self.provider.load_from_data(css, -1)
        gtk_settings = Gtk.Settings.get_default()
        if gtk_settings.props.gtk_application_prefer_dark_theme != self.dark:
            gtk_settings.props.gtk_application_prefer_dark_theme = self.dark
        for cb in list(self._listeners):
            cb()

    def reduced_motion(self):
        return self.desktop.reduced_motion()

    def export_template(self):
        return palette_template(self.roles, self.seed, self.style, self.dark)
