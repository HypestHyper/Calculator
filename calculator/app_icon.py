"""App icon that follows the user's colours.

Launchers can't restyle an app icon, so the app writes its own icon, coloured
from the current palette, to the user's icon folder and rewrites it when the
colours change. The shipped icon is the fallback until the app first runs.

Icon: "calculator" from Material Design Icons by Pictogrammers, Apache 2.0.
"""

import shutil
from pathlib import Path

from gi.repository import Gio, GLib

from .config import APP_ID, _xdg, data_dirs

PATH_D = ("M7,2H17A2,2 0 0,1 19,4V20A2,2 0 0,1 17,22H7A2,2 0 0,1 5,20V4A2,2 0 0,1 7,2"
          "M7,4V8H17V4H7M7,10V12H9V10H7M11,10V12H13V10H11M15,10V12H17V10H15"
          "M7,14V16H9V14H7M11,14V16H13V14H11M15,14V16H17V14H15"
          "M7,18V20H9V18H7M11,18V20H13V18H11M15,18V20H17V18H15Z")

DEFAULT_COLORS = ("#8f7ae0", "#5a3fa8")


def render(top, bottom):
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 24 24">\n'
        '  <!-- "calculator" from Material Design Icons (Pictogrammers), Apache License 2.0 -->\n'
        '  <defs><linearGradient id="g" x1="0" y1="2" x2="0" y2="22" gradientUnits="userSpaceOnUse">'
        f'<stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/>'
        '</linearGradient></defs>\n'
        f'  <path fill="url(#g)" d="{PATH_D}"/>\n'
        '</svg>\n'
    )


def _mix(a, b, t):
    a, b = a.lstrip("#"), b.lstrip("#")
    ca = [int(a[i:i + 2], 16) for i in (0, 2, 4)]
    cb = [int(b[i:i + 2], 16) for i in (0, 2, 4)]
    return "#" + "".join(f"{round(x + (y - x) * t):02x}" for x, y in zip(ca, cb))


def colors_for(roles):
    """A gentle top-to-bottom gradient from the palette's primary colour."""
    primary = roles.get("primary", DEFAULT_COLORS[1])
    return _mix(primary, "#ffffff", 0.18), _mix(primary, "#000000", 0.12)


def _icons_dir():
    return _xdg("XDG_DATA_HOME", ".local/share") / "icons" / "hicolor"


def _is_installed():
    """Only manage the icon for an installed app, not a source checkout."""
    for d in data_dirs()[1:]:
        if (d.parent / "applications" / f"{APP_ID}.desktop").is_file():
            return True
    return False


def update(roles, themed=True):
    """Write the icon if it changed. Safe to call on every theme change."""
    if not _is_installed():
        return
    colors = colors_for(roles) if themed else DEFAULT_COLORS
    target = _icons_dir() / "scalable" / "apps" / f"{APP_ID}.svg"
    svg = render(*colors)
    try:
        if target.is_file() and target.read_text(encoding="utf-8") == svg:
            return
        target.parent.mkdir(parents=True, exist_ok=True)
        tmp = target.with_suffix(".tmp")
        tmp.write_text(svg, encoding="utf-8")
        tmp.replace(target)
    except OSError as err:
        print(f"calculator: could not update the app icon: {err}")
        return
    _refresh_cache(_icons_dir())


def _refresh_cache(theme_dir):
    if not (Path(theme_dir) / "icon-theme.cache").exists():
        return  # no cache: icon lookups read the folder directly
    tool = shutil.which("gtk4-update-icon-cache") or shutil.which("gtk-update-icon-cache")
    if tool is None:
        return
    try:
        Gio.Subprocess.new([tool, "-q", "-t", "-f", str(theme_dir)], Gio.SubprocessFlags.NONE)
    except GLib.Error:
        pass
