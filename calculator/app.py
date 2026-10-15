"""Application entry point."""

import os
import sys

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
gi.require_version("PangoCairo", "1.0")
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

from . import icons, window_rules  # noqa: E402
from .config import APP_ID, APP_NAME, VERSION, History, Settings, WindowState  # noqa: E402

GLib.set_prgname(APP_ID)  # X11 WM_CLASS; Wayland uses the application ID
GLib.set_application_name(APP_NAME)


class CalculatorApp(Gtk.Application):
    def __init__(self):
        flags = Gio.ApplicationFlags.DEFAULT_FLAGS
        if os.environ.get("CALCULATOR_DEBUG"):
            # Always a fresh process, so the debug output shows up here even
            # when another Calculator window is already open.
            flags |= Gio.ApplicationFlags.NON_UNIQUE
        super().__init__(application_id=APP_ID, flags=flags)
        self.settings = None
        self.window = None
        self.prefs = None
        self.add_main_option("print-window-rules", 0, GLib.OptionFlags.NONE, GLib.OptionArg.STRING,
                             "Print the floating-window rule for a compositor and exit",
                             "|".join(window_rules.ORDER))
        self.add_main_option("print-palette", 0, GLib.OptionFlags.NONE, GLib.OptionArg.NONE,
                             "Print the active palette as palette.toml and exit", None)
        self.add_main_option("version", 0, GLib.OptionFlags.NONE, GLib.OptionArg.NONE,
                             "Show the version and exit", None)
        self._print_palette = False

    def do_handle_local_options(self, options):
        opts = options.end().unpack()
        if "version" in opts:
            print(f"{APP_NAME} {VERSION}")
            return 0
        if "print-window-rules" in opts:
            key = opts["print-window-rules"].lower()
            if key not in window_rules.RULES:
                print(f"Unknown compositor “{key}”. Choose one of: {', '.join(window_rules.ORDER)}",
                      file=sys.stderr)
                return 2
            print(window_rules.text_for(key), end="")
            return 0
        if "print-palette" in opts:
            self._print_palette = True
        return -1

    def do_startup(self):
        Gtk.Application.do_startup(self)
        if os.environ.get("CALCULATOR_DEBUG"):
            display = Gdk.Display.get_default()
            print(f"calculator: {APP_NAME} {VERSION} debug on, running from {__file__}, "
                  f"display {type(display).__name__}, "
                  f"desktop {os.environ.get('XDG_CURRENT_DESKTOP', '?')}", flush=True)
        icons.load_font()
        from .theme import ThemeManager
        self.settings = Settings()
        self.history = History(enabled=self.settings["save_history"])
        self.window_state = WindowState()
        self.theme = ThemeManager(self.settings)
        self.settings.connect(self._on_setting)
        self.theme.connect(self._update_app_icon)
        self._update_app_icon()
        if self._print_palette:
            print(self.theme.export_template())
            self.quit()
            return
        self.set_accels_for_action("app.quit", ["<Ctrl>q"])
        quit_action = Gio.SimpleAction.new("quit", None)
        quit_action.connect("activate", lambda *_: self.quit())
        self.add_action(quit_action)

    def _on_setting(self, key, value):
        if key == "save_history":
            self.history.set_enabled(value)
        elif key == "themed_icon":
            self._update_app_icon()

    def _update_app_icon(self):
        from . import app_icon
        app_icon.update(self.theme.roles, self.settings["themed_icon"])

    def do_activate(self):
        if self._print_palette:
            return
        if self.window is None:
            from .window import CalculatorWindow
            self.window = CalculatorWindow(self)
            self.window.connect("close-request", self._on_main_close)
        self.window.present()

    def _on_main_close(self, *_):
        # The hidden preferences window would otherwise keep the app running.
        if self.prefs is not None:
            self.prefs.destroy()
            self.prefs = None
        return False

    def show_preferences(self):
        if self.prefs is None:
            from .preferences import PreferencesWindow
            self.prefs = PreferencesWindow(self, self.window)
            self.prefs.set_hide_on_close(True)
        self.prefs.present()


def main():
    return CalculatorApp().run(sys.argv)
