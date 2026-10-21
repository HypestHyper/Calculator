"""Compositor-specific window resizing.

Some compositors ignore a floating window's own resize requests. Hyprland is
one of them, but it can be asked through hyprctl. Everything here is
asynchronous and best-effort: on any failure the callback gets False and the
window falls back to the generic method.
"""

import json
import os
import shutil

from gi.repository import Gio, GLib

from .config import APP_ID


def is_hyprland():
    return bool(os.environ.get("HYPRLAND_INSTANCE_SIGNATURE")) and shutil.which("hyprctl") is not None


def _run(argv, callback):
    """Run a command; callback(ok, stdout)."""
    try:
        proc = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_MERGE)
    except GLib.Error:
        callback(False, "")
        return

    def done(p, result):
        try:
            _ok, out, _err = p.communicate_utf8_finish(result)
        except GLib.Error:
            callback(False, "")
            return
        callback(p.get_successful(), out or "")
    proc.communicate_utf8_async(None, None, done)


def _dispatch(lua_expr, classic_args, callback, debug):
    """Try Hyprland's Lua dispatch (0.55+), then the classic syntax."""
    def classic(ok, out):
        debug(f"hyprctl (lua) -> {out.strip()!r}")
        if ok and out.strip() == "ok":
            callback(True)
            return

        def after(ok2, out2):
            debug(f"hyprctl (classic) -> {out2.strip()!r}")
            callback(ok2 and out2.strip() == "ok")
        _run(["hyprctl", "dispatch"] + classic_args, after)
    _run(["hyprctl", "dispatch", lua_expr], classic)


def _set_no_anim(target, value, debug, callback=lambda ok: None):
    """Turn Hyprland's animations for one window off ("1") or back ("unset")."""
    _dispatch(f'hl.dsp.window.set_prop({{ prop = "no_anim", value = "{value}", window = "{target}" }})',
              ["setprop", f"{target} noanim {value}"], callback, debug)


def hyprland_resize(width, height, callback, debug=lambda m: None, animate=True):
    """Resize this app's window to width x height, keeping its centre.

    With animate=False the change is instant. Hyprland animates a size change
    by drawing the window's new contents scaled to the in-between sizes, which
    stretches the keypad while the window shrinks."""
    def got_clients(ok, out):
        if not ok:
            callback(False)
            return
        try:
            clients = json.loads(out)
        except ValueError:
            callback(False)
            return
        pid = os.getpid()
        mine = [c for c in clients if c.get("pid") == pid and c.get("class") == APP_ID]
        if not mine:
            debug("hyprctl: window not found in clients")
            callback(False)
            return
        c = mine[0]
        address = c.get("address", "")
        (x, y), (w, h) = c.get("at", (0, 0)), c.get("size", (0, 0))
        nx, ny = round(x + (w - width) / 2), round(y + (h - height) / 2)
        target = f"address:{address}"

        def finish(ok):
            if not animate:
                # Back to the user's animations once the change has happened.
                GLib.timeout_add(250, lambda: _set_no_anim(target, "unset", debug) and False)
            callback(ok)

        def moved(_ok):
            finish(True)

        def resized(ok2):
            if not ok2:
                finish(False)
                return
            _dispatch(
                f'hl.dsp.window.move({{ x = {nx}, y = {ny}, relative = false, window = "{target}" }})',
                ["movewindowpixel", f"exact {nx} {ny},{target}"], moved, debug)

        def resize(_ok=True):
            _dispatch(
                f'hl.dsp.window.resize({{ x = {width}, y = {height}, relative = false, window = "{target}" }})',
                ["resizewindowpixel", f"exact {width} {height},{target}"], resized, debug)

        if animate:
            resize()
        else:
            _set_no_anim(target, "1", debug, resize)  # resize either way

    _run(["hyprctl", "clients", "-j"], got_clients)
