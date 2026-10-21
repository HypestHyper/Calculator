"""Application constants, file locations and the settings store."""

import json
import os
import tomllib
from pathlib import Path

APP_ID = "io.github.HypestHyper.Calculator"
APP_NAME = "Calculator"
VERSION = "1.0.3"
WEBSITE = "https://github.com/HypestHyper/Calculator"

from .presets import ORDER as PRESET_KEYS  # noqa: E402

PACKAGE_DIR = Path(__file__).resolve().parent


def _xdg(var, fallback):
    value = os.environ.get(var)
    return Path(value) if value and os.path.isabs(value) else Path.home() / fallback


CONFIG_DIR = _xdg("XDG_CONFIG_HOME", ".config") / APP_ID
STATE_DIR = _xdg("XDG_STATE_HOME", ".local/state") / APP_ID
SETTINGS_FILE = CONFIG_DIR / "settings.toml"
PALETTE_FILE = CONFIG_DIR / "palette.toml"
WINDOW_FILE = STATE_DIR / "window.json"
HISTORY_FILE = STATE_DIR / "history.json"


def data_dirs():
    """Where bundled data (fonts) may live: source tree first, then installs."""
    # Source checkout, then the install dir next to the package, then XDG dirs.
    dirs = [PACKAGE_DIR.parent / "data", PACKAGE_DIR.parent]
    xdg_data = os.environ.get("XDG_DATA_DIRS", "/usr/local/share:/usr/share").split(":")
    for base in [str(_xdg("XDG_DATA_HOME", ".local/share"))] + xdg_data:
        if base:
            dirs.append(Path(base) / APP_ID)
    return dirs


# --------------------------------------------------------------------------
# Settings
# --------------------------------------------------------------------------

# key -> (default, allowed values or a type)
SCHEMA = {
    "theme": ("system", ("system", "light", "dark")),
    "color_source": ("desktop", ("desktop", "preset", "custom", "palette")),
    "preset": ("lavender", tuple(PRESET_KEYS)),
    "custom_accent": ("#6750A4", str),
    "palette_style": (
        "tonal_spot",
        ("tonal_spot", "vibrant", "expressive", "fidelity", "content",
         "neutral", "monochrome", "rainbow", "fruit_salad"),
    ),
    "contrast": ("system", ("system", "standard", "medium", "high")),
    "text_size": ("medium", ("small", "medium", "large")),
    "button_size": ("standard", ("compact", "standard", "large")),
    "live_preview": (True, bool),
    "digit_grouping": (True, bool),
    "decimals": (-1, int),  # -1 = auto
    "decimal_separator": ("point", ("point", "comma")),
    "angle_unit": ("deg", ("deg", "rad")),
    "motion": ("system", ("system", "expressive", "standard", "off")),
    "save_history": (True, bool),
    "remember_size": (True, bool),
    "themed_icon": (True, bool),
    "scientific": (False, bool),
}


def _valid(key, value):
    default, rule = SCHEMA[key]
    if isinstance(rule, tuple):
        return value in rule
    if rule is bool:
        return isinstance(value, bool)
    if rule is int:
        return isinstance(value, int) and not isinstance(value, bool) and -1 <= value <= 12
    if rule is str:
        return isinstance(value, str) and _is_hex(value)
    return False


def _is_hex(text):
    t = text.lstrip("#")
    return len(t) == 6 and all(c in "0123456789abcdefABCDEF" for c in t)


def _toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    return json.dumps(value)  # a valid TOML basic string for our values


class Settings:
    """Small observable settings store backed by settings.toml."""

    def __init__(self, path=SETTINGS_FILE):
        self.path = Path(path)
        self._values = {k: v[0] for k, v in SCHEMA.items()}
        self._listeners = []
        self.load()

    def load(self):
        try:
            with open(self.path, "rb") as fh:
                data = tomllib.load(fh)
        except FileNotFoundError:
            return
        except (OSError, tomllib.TOMLDecodeError) as err:
            print(f"calculator: ignoring unreadable settings: {err}")
            return
        for key, value in data.items():
            if key in SCHEMA and _valid(key, value):
                self._values[key] = value

    def save(self):
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            lines = ["# Calculator settings. Edited from Preferences; safe to edit by hand.\n"]
            lines += [f"{k} = {_toml_value(v)}\n" for k, v in self._values.items()]
            tmp = self.path.with_suffix(".tmp")
            tmp.write_text("".join(lines), encoding="utf-8")
            tmp.replace(self.path)
        except OSError as err:
            print(f"calculator: could not save settings: {err}")

    def __getitem__(self, key):
        return self._values[key]

    def set(self, key, value):
        if not _valid(key, value):
            raise ValueError(f"invalid value for {key}: {value!r}")
        if self._values[key] == value:
            return
        self._values[key] = value
        self.save()
        for cb in list(self._listeners):
            cb(key, value)

    def connect(self, callback):
        self._listeners.append(callback)


# --------------------------------------------------------------------------
# Window state and history (JSON in the state dir)
# --------------------------------------------------------------------------

def _read_json(path, fallback):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return fallback


def _write_json(path, data):
    try:
        path = Path(path)
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(".tmp")
        tmp.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
        tmp.replace(path)
    except OSError as err:
        print(f"calculator: could not write {path}: {err}")


DEFAULT_SIZES = {"standard": (400, 640), "scientific": (720, 640)}


class WindowState:
    def __init__(self, path=WINDOW_FILE):
        self.path = path
        data = _read_json(path, {})
        self.sizes = dict(DEFAULT_SIZES)
        for mode in DEFAULT_SIZES:
            size = data.get(mode)
            if isinstance(size, list) and len(size) == 2 and all(isinstance(n, int) and n > 100 for n in size):
                self.sizes[mode] = tuple(size)

    def size_for(self, mode):
        return self.sizes.get(mode, DEFAULT_SIZES[mode])

    def remember(self, mode, width, height):
        if width > 100 and height > 100:
            self.sizes[mode] = (int(width), int(height))

    def reset(self):
        self.sizes = dict(DEFAULT_SIZES)

    def save(self):
        _write_json(self.path, {k: list(v) for k, v in self.sizes.items()})


class History:
    LIMIT = 100

    def __init__(self, path=HISTORY_FILE, enabled=True):
        self.path = path
        self.enabled = enabled
        self.items = []  # newest first: {"expr": str, "result": str, "value": float}
        self._listeners = []
        if enabled:
            data = _read_json(path, [])
            if isinstance(data, list):
                self.items = [
                    d for d in data
                    if isinstance(d, dict) and {"expr", "result", "value"} <= d.keys()
                ][: self.LIMIT]

    def connect(self, callback):
        self._listeners.append(callback)

    def _changed(self):
        if self.enabled:
            _write_json(self.path, self.items)
        for cb in list(self._listeners):
            cb()

    def add(self, expr, result, value):
        if self.items and self.items[0]["expr"] == expr:
            return
        self.items.insert(0, {"expr": expr, "result": result, "value": value})
        del self.items[self.LIMIT:]
        self._changed()

    def clear(self):
        self.items = []
        self._changed()
        if not self.enabled:
            try:
                Path(self.path).unlink()
            except OSError:
                pass

    def set_enabled(self, enabled):
        self.enabled = enabled
        if not enabled:
            try:
                Path(self.path).unlink()
            except OSError:
                pass
        else:
            self._changed()
