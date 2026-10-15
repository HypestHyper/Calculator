"""Preferences window."""

from gi.repository import Gdk, Gio, GLib, Graphene, Gsk, Gtk

from . import window_rules
from .config import APP_NAME, CONFIG_DIR, PALETTE_FILE, VERSION, WEBSITE
from .icons import icon
from .presets import ORDER as PRESET_ORDER
from .presets import PRESETS
from .theme import STYLE_NAMES, generate_palette, parse_hex, to_hex

SHORTCUTS = [
    ("Enter", "Calculate"), ("Esc", "Clear, or close history"), ("Backspace", "Delete last"),
    ("Ctrl+C / Ctrl+V", "Copy result / paste"), ("Ctrl+H", "History"),
    ("Ctrl+S", "Scientific mode"), ("Ctrl+,", "Preferences"),
    ("s c t l g r p e", "sin cos tan ln log √ π e (scientific)"),
]


class PreferencesWindow(Gtk.Window):
    def __init__(self, app, parent):
        super().__init__(title="Preferences", transient_for=parent, modal=False,
                         default_width=560, default_height=720)
        self.app = app
        self.settings = app.settings
        self.add_css_class("calc-prefs")
        if parent and parent.has_css_class("no-motion"):
            self.add_css_class("no-motion")

        header = Gtk.HeaderBar()
        header.set_title_widget(Gtk.Label(label="Preferences", css_classes=["title-text"]))
        self.set_titlebar(header)

        page = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["pref-page"])
        scroll = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER, vexpand=True)
        scroll.set_child(page)
        self.set_child(scroll)

        self._build_appearance(page)
        self._build_palette_file(page)
        self._build_display(page)
        self._build_behaviour(page)
        self._build_window(page)
        self._build_shortcuts(page)
        self._build_about(page)

        keys = Gtk.EventControllerKey()
        keys.connect("key-pressed", self._on_key)
        self.add_controller(keys)

        self._theme_cb = self._on_theme
        app.theme.connect(self._theme_cb)
        self._on_theme()

    def _on_key(self, _c, keyval, _code, mods):
        if keyval == Gdk.KEY_Escape or (
                mods & Gdk.ModifierType.CONTROL_MASK and keyval in (Gdk.KEY_w, Gdk.KEY_comma)):
            self.close()
            return True
        return False

    # ------------------------------------------------------------ helpers
    def _section(self, page, title):
        page.append(Gtk.Label(label=title, xalign=0, css_classes=["section-title"]))
        card = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["pref-card"])
        page.append(card)
        return card

    def _row(self, card, title, subtitle, widget, icon_name=None, vertical=False):
        row = Gtk.Box(spacing=14, css_classes=["pref-row"])
        if icon_name:
            ic = icon(icon_name, 20)
            ic.set_valign(Gtk.Align.CENTER)
        else:
            ic = Gtk.Box()  # keeps titles aligned
        ic.set_size_request(22, -1)
        row.append(ic)
        texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, hexpand=True, valign=Gtk.Align.CENTER)
        texts.append(Gtk.Label(label=title, xalign=0, wrap=True, css_classes=["pref-title"]))
        sub = None
        if subtitle is not None:
            sub = Gtk.Label(label=subtitle, xalign=0, wrap=True, css_classes=["pref-subtitle"])
            texts.append(sub)
        if vertical:
            outer = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=10, css_classes=["pref-row"])
            row.remove_css_class("pref-row")
            row.append(texts)
            outer.append(row)
            if widget is not None:
                outer.append(widget)
            card.append(outer)
            outer._subtitle = sub
            return outer
        row.append(texts)
        if widget is not None:
            widget.set_valign(Gtk.Align.CENTER)
            row.append(widget)
        card.append(row)
        row._subtitle = sub
        return row

    def _segmented(self, options, key):
        group = Gtk.Box(css_classes=["connected-group", "small"])
        first = None
        for value, label in options:
            b = Gtk.ToggleButton(label=label)
            if first is None:
                first = b
            else:
                b.set_group(first)
            b.set_active(self.settings[key] == value)
            b.connect("toggled", lambda btn, v=value: btn.get_active() and self.settings.set(key, v))
            group.append(b)
        return group

    def _dropdown(self, options, key, on_change=None):
        values = [v for v, _ in options]
        dd = Gtk.DropDown.new_from_strings([label for _, label in options])
        current = self.settings[key]
        dd.set_selected(values.index(current) if current in values else 0)

        def changed(widget, _pspec):
            value = values[widget.get_selected()]
            self.settings.set(key, value)
            if on_change:
                on_change(value)
        dd.connect("notify::selected", changed)
        return dd

    def _switch(self, key):
        sw = Gtk.Switch(active=self.settings[key])
        sw.connect("notify::active", lambda w, _p: self.settings.set(key, w.get_active()))
        return sw

    # ------------------------------------------------------------ sections
    def _build_appearance(self, page):
        card = self._section(page, "Colours")
        self._row(card, "Theme", None, self._segmented(
            [("system", "System"), ("light", "Light"), ("dark", "Dark")], "theme"), "contrast")

        self.source_values = ["desktop", "preset", "custom", "palette"]
        self.source_dd = self._dropdown(
            [("desktop", "Desktop accent"), ("preset", "Preset palette"),
             ("custom", "Custom colour"), ("palette", "Palette file")],
            "color_source", self._update_source_rows)
        self._row(card, "Colour source", "Where the colours come from", self.source_dd, "palette")

        self.preset_row = self._row(card, "Preset", "Material palettes and popular themes",
                                    self._preset_dropdown())

        if hasattr(Gtk, "ColorDialogButton"):
            self.color_button = Gtk.ColorDialogButton(dialog=Gtk.ColorDialog(with_alpha=False))
            rgba = Gdk.RGBA()
            rgba.parse(self.settings["custom_accent"])
            self.color_button.set_rgba(rgba)
            self.color_button.connect("notify::rgba", self._on_color)
            color_widget = self.color_button
        else:
            entry = Gtk.Entry(text=self.settings["custom_accent"], max_width_chars=8)
            entry.connect("activate", lambda e: parse_hex(e.get_text()) and
                          self.settings.set("custom_accent", to_hex(parse_hex(e.get_text()))))
            color_widget = entry
        self.custom_row = self._row(card, "Custom colour", "Seed for the generated palette", color_widget)

        self.style_row = self._row(card, "Palette style", "How colourful the generated palette is",
                                   self._dropdown(list(STYLE_NAMES.items()), "palette_style"), "tune")
        self._row(card, "Contrast", None, self._dropdown(
            [("system", "Follow system"), ("standard", "Standard"), ("medium", "Medium"),
             ("high", "High")], "contrast"))
        self._row(card, "Match app icon", "Colour the launcher icon with your palette",
                  self._switch("themed_icon"), "calculate")
        self.status_row = self._row(card, "Active colours", "", None, "info")
        self._update_source_rows(self.settings["color_source"])

    def _build_palette_file(self, page):
        card = self._section(page, "Palette file")
        self.palette_row = self._row(
            card, "palette.toml",
            f"{_home(PALETTE_FILE)}\nSet a seed, a style or any colour role by hand. "
            "Changes apply as soon as the file is saved.", None, vertical=True)
        buttons = Gtk.Box(spacing=8)
        self.palette_create = Gtk.Button(label="Create from current colours", css_classes=["tonal"])
        self.palette_create.connect("clicked", self._create_palette)
        self.palette_open = Gtk.Button(label="Open", css_classes=["tonal"])
        self.palette_open.connect("clicked", lambda *_: _open_path(self, PALETTE_FILE))
        folder = Gtk.Button(label="Open folder", css_classes=["text-btn"])
        folder.connect("clicked", lambda *_: _open_path(self, CONFIG_DIR))
        for b in (self.palette_create, self.palette_open, folder):
            buttons.append(b)
        self.palette_row.append(buttons)
        self._update_palette_buttons()

    def _build_display(self, page):
        card = self._section(page, "Display")
        self._row(card, "Text size", None, self._segmented(
            [("small", "S"), ("medium", "M"), ("large", "L")], "text_size"), "text_fields")
        self._row(card, "Button size", None, self._segmented(
            [("compact", "Compact"), ("standard", "Standard"), ("large", "Large")], "button_size"),
            "grid_view")
        self._row(card, "Live preview", "Show the result while typing", self._switch("live_preview"))
        self._row(card, "Digit grouping", "Separate thousands with a thin space",
                  self._switch("digit_grouping"))
        self._row(card, "Decimal places", None, self._dropdown(
            [(-1, "Auto")] + [(n, str(n)) for n in range(0, 11)], "decimals"))

    def _build_behaviour(self, page):
        card = self._section(page, "Behaviour")
        self._row(card, "Angle unit", "Used by sin, cos, tan and their inverses", self._segmented(
            [("deg", "Degrees"), ("rad", "Radians")], "angle_unit"), "square_foot")
        self._row(card, "Animations", "Expressive springs bounce; Standard does not",
                  self._dropdown([("system", "Follow system"), ("expressive", "Expressive"),
                                  ("standard", "Standard"), ("off", "Off")], "motion"), "animation")
        self._row(card, "Save history", "Keep past calculations between sessions",
                  self._switch("save_history"), "history")
        clear = Gtk.Button(label="Clear", css_classes=["danger"])
        clear.connect("clicked", lambda *_: self.app.history.clear())
        self._row(card, "Clear history", None, clear)

    def _build_window(self, page):
        card = self._section(page, "Window")
        self._row(card, "Remember size", "Separately for standard and scientific mode",
                  self._switch("remember_size"), "desktop_windows")
        reset = Gtk.Button(label="Reset", css_classes=["tonal"])
        reset.connect("clicked", self._reset_size)
        self._row(card, "Reset window size", None, reset)

        detected = window_rules.detect()
        sub = ("Tiling compositors decide whether a window floats. "
               "Add this rule once and Calculator always opens floating. "
               "Drag the top of the window to move it.")
        if detected:
            sub = f"Detected: {window_rules.RULES[detected]['name']}. " + sub
        float_row = self._row(card, "Open floating", sub, None, "open_in_new", vertical=True)

        tabs = Gtk.Box(css_classes=["connected-group", "small"])
        self.rule_label = Gtk.Label(xalign=0, selectable=True, wrap=False, css_classes=["code"])
        self.rule_hint = Gtk.Label(xalign=0, wrap=True, css_classes=["pref-subtitle"])
        first = None
        for key in window_rules.ORDER:
            b = Gtk.ToggleButton(label=window_rules.RULES[key]["name"])
            if first is None:
                first = b
            else:
                b.set_group(first)
            b.connect("toggled", lambda btn, k=key: btn.get_active() and self._show_rule(k))
            tabs.append(b)
            if key == (detected or window_rules.ORDER[0]):
                b.set_active(True)
        self._show_rule(detected or window_rules.ORDER[0])

        copy = Gtk.Button(css_classes=["filled"], halign=Gtk.Align.START)
        copy_box = Gtk.Box(spacing=6)
        copy_box.append(icon("content_copy", 18))
        copy_box.append(Gtk.Label(label="Copy rule"))
        copy.set_child(copy_box)
        copy.connect("clicked", self._copy_rule)

        top = Gtk.Box(spacing=8)
        top.append(tabs)
        float_row.append(top)
        code_scroll = Gtk.ScrolledWindow(vscrollbar_policy=Gtk.PolicyType.NEVER,
                                         propagate_natural_height=True)
        code_scroll.set_child(self.rule_label)
        float_row.append(code_scroll)
        float_row.append(self.rule_hint)
        float_row.append(copy)

    def _build_shortcuts(self, page):
        card = self._section(page, "Keyboard")
        grid = Gtk.Grid(column_spacing=18, row_spacing=6, css_classes=["pref-row"])
        for i, (keys, what) in enumerate(SHORTCUTS):
            grid.attach(Gtk.Label(label=keys, xalign=0, css_classes=["pref-title"]), 0, i, 1, 1)
            grid.attach(Gtk.Label(label=what, xalign=0, wrap=True, css_classes=["pref-subtitle"]),
                        1, i, 1, 1)
        card.append(grid)

    def _build_about(self, page):
        card = self._section(page, "About")
        link = Gtk.Button(label="Website", css_classes=["text-btn"])
        link.connect("clicked", lambda *_: _launch_uri(self, WEBSITE))
        self._row(card, f"{APP_NAME} {VERSION}",
                  "MIT licence. Icons: Material Symbols and Material Design Icons, Apache-2.0.",
                  link, "calculate")

    # ------------------------------------------------------------ actions
    def _on_color(self, button, _pspec):
        rgba = button.get_rgba()
        self.settings.set("custom_accent", to_hex((rgba.red * 255, rgba.green * 255, rgba.blue * 255)))

    def _update_source_rows(self, source):
        self.custom_row.set_sensitive(source == "custom")
        self.style_row.set_sensitive(source in ("desktop", "custom"))

    # Preset picker: a drop-down whose items show colour swatches.
    def _preset_dropdown(self):
        model = Gtk.StringList.new(PRESET_ORDER)
        factory = Gtk.SignalListItemFactory()
        factory.connect("setup", self._preset_setup)
        factory.connect("bind", self._preset_bind)
        dd = Gtk.DropDown(model=model, factory=factory)
        dd.set_selected(PRESET_ORDER.index(self.settings["preset"]))
        dd.connect("notify::selected", self._on_preset)
        self.preset_dd = dd
        self._swatch_cache = {}
        return dd

    def _preset_setup(self, _factory, item):
        box = Gtk.Box(spacing=10)
        swatch = Swatch(self._swatch_colors)
        box.append(swatch)
        box.append(Gtk.Label(xalign=0))
        item.set_child(box)

    def _preset_bind(self, _factory, item):
        key = item.get_item().get_string()
        box = item.get_child()
        swatch, label = box.get_first_child(), box.get_last_child()
        swatch.set_key(key)
        label.set_label(PRESETS[key]["name"])

    def _swatch_colors(self, key):
        dark = self.app.theme.dark
        cached = self._swatch_cache.get((key, dark))
        if cached:
            return cached
        preset = PRESETS[key]
        roles = preset.get("dark" if dark else "light")
        if not roles:
            roles, _ = generate_palette(parse_hex(preset["seed"]), dark, 0.0, preset["style"])
        colors = [roles[r] for r in ("surface_container_high", "primary", "secondary", "tertiary")]
        self._swatch_cache[(key, dark)] = colors
        return colors

    def _on_preset(self, dd, _pspec):
        key = PRESET_ORDER[dd.get_selected()]
        self.settings.set("preset", key)
        if self.settings["color_source"] != "preset":
            self.source_dd.set_selected(self.source_values.index("preset"))

    def _on_theme(self):
        if hasattr(self, "preset_dd"):
            self.preset_dd.queue_draw()
            for child in _descendants(self.preset_dd):
                if isinstance(child, Swatch):
                    child.queue_draw()
        info = self.app.theme.info
        mode = "dark" if self.app.theme.dark else "light"
        text = (f"{mode.capitalize()} mode from {info.get('mode')} · accent {to_hex(self.app.theme.seed)} "
                f"from {info.get('accent')} · palette by {info.get('generator')}")
        if self.status_row._subtitle is not None:
            self.status_row._subtitle.set_label(text)
        self._update_palette_buttons()

    def _update_palette_buttons(self):
        exists = PALETTE_FILE.exists()
        self.palette_open.set_sensitive(exists)
        self.palette_create.set_label("Overwrite from current colours" if exists
                                      else "Create from current colours")

    def _create_palette(self, *_):
        try:
            PALETTE_FILE.parent.mkdir(parents=True, exist_ok=True)
            PALETTE_FILE.write_text(self.app.theme.export_template(), encoding="utf-8")
        except OSError as err:
            print(f"calculator: could not write palette file: {err}")
            return
        self._update_palette_buttons()
        _open_path(self, PALETTE_FILE)

    def _show_rule(self, key):
        rule = window_rules.RULES[key]
        self._rule_key = key
        self.rule_label.set_label(rule["snippet"])
        self.rule_hint.set_label(f"Add to {rule['file']}. {rule['note']}")

    def _copy_rule(self, *_):
        self.get_clipboard().set(window_rules.RULES[self._rule_key]["snippet"] + "\n")
        parent = self.get_transient_for()
        if parent is not None and hasattr(parent, "show_snackbar"):
            parent.show_snackbar("Window rule copied")

    def _reset_size(self, *_):
        self.app.window_state.reset()
        self.app.window_state.save()
        parent = self.get_transient_for()
        if parent is not None:
            parent._apply_size_for(parent._mode)


class Swatch(Gtk.Widget):
    """A pill showing a preset's surface with three accent dots. Drawn with
    GtkSnapshot, so it needs no cairo bindings."""

    __gtype_name__ = "CalcSwatch"

    def __init__(self, colors_for):
        super().__init__(valign=Gtk.Align.CENTER)
        self._colors_for = colors_for
        self._key = None

    def set_key(self, key):
        self._key = key
        self.queue_draw()

    def do_measure(self, orientation, _for_size):
        size = 52 if orientation == Gtk.Orientation.HORIZONTAL else 24
        return size, size, -1, -1

    def do_snapshot(self, snapshot):
        if self._key is None:
            return
        w, h = self.get_width(), self.get_height()
        bg, *dots = self._colors_for(self._key)
        _fill_round(snapshot, bg, 0, 0, w, h, h / 2)
        r = h / 2 - 5
        step = (w - h) / 2
        for i, color in enumerate(dots):
            cx = h / 2 + i * step
            _fill_round(snapshot, color, cx - r, h / 2 - r, 2 * r, 2 * r, r)


def _fill_round(snapshot, hex_color, x, y, w, h, radius):
    rgba = Gdk.RGBA()
    rgba.parse(hex_color)
    rect = Graphene.Rect().init(x, y, w, h)
    rounded = Gsk.RoundedRect()
    rounded.init_from_rect(rect, radius)
    snapshot.push_rounded_clip(rounded)
    snapshot.append_color(rgba, rect)
    snapshot.pop()


def _descendants(widget):
    child = widget.get_first_child()
    while child is not None:
        yield child
        yield from _descendants(child)
        child = child.get_next_sibling()


def _home(path):
    text = str(path)
    home = GLib.get_home_dir()
    return "~" + text[len(home):] if text.startswith(home) else text


def _launch_uri(window, uri):
    if hasattr(Gtk, "UriLauncher"):
        Gtk.UriLauncher.new(uri).launch(window, None, None, None)
    else:
        Gio.AppInfo.launch_default_for_uri(uri, None)


def _open_path(window, path):
    try:
        (path if not path.suffix else path.parent).mkdir(parents=True, exist_ok=True)
    except OSError:
        pass
    gfile = Gio.File.new_for_path(str(path))
    if hasattr(Gtk, "FileLauncher"):
        Gtk.FileLauncher.new(gfile).launch(window, None, None, None)
    else:
        try:
            Gio.AppInfo.launch_default_for_uri(gfile.get_uri(), None)
        except GLib.Error as err:
            print(f"calculator: could not open {path}: {err}")
