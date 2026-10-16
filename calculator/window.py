"""Main calculator window."""

import os

from gi.repository import Gdk, GLib, Gtk, Pango

from . import compositor
from .config import APP_NAME, DEFAULT_SIZES
from .engine import format_number, localize
from .icons import icon, icon_button
from .logic import Calculator
from .motion import Animated, KeyMotion, motion

# (label or icon:name, action, css classes, tooltip)
STANDARD_KEYS = [
    [("AC", "clear:", "clear", "Clear (Esc)"), ("±", "neg:", "util", "Change sign"),
     ("%", "pct:", "util", "Percent"), ("÷", "op:÷", "op", "Divide")],
    [("7", "digit:7", "digit", None), ("8", "digit:8", "digit", None),
     ("9", "digit:9", "digit", None), ("×", "op:×", "op", "Multiply")],
    [("4", "digit:4", "digit", None), ("5", "digit:5", "digit", None),
     ("6", "digit:6", "digit", None), ("−", "op:−", "op", "Subtract")],
    [("1", "digit:1", "digit", None), ("2", "digit:2", "digit", None),
     ("3", "digit:3", "digit", None), ("+", "op:+", "op", "Add")],
    [("0", "digit:0", "digit", None), (".", "dot:", "digit", None),
     ("icon:backspace", "back:", "util", "Backspace"), ("=", "eq:", "equals", "Equals (Enter)")],
]

# Normal and inverse (INV) variants: (label, action, tooltip)
SCI_KEYS = [
    [("INV", "inv"), ("(", "open:"), (")", "close:"), ("mod", "op: mod ")],
    [(("sin", "func:sin(", "Sine"), ("sin⁻¹", "func:asin(", "Arcsine")),
     (("cos", "func:cos(", "Cosine"), ("cos⁻¹", "func:acos(", "Arccosine")),
     (("tan", "func:tan(", "Tangent"), ("tan⁻¹", "func:atan(", "Arctangent")),
     ("π", "const:π")],
    [(("ln", "func:ln(", "Natural logarithm"), ("eˣ", "func:e^(", "e to the power")),
     (("log", "func:log(", "Logarithm, base 10"), ("10ˣ", "func:10^(", "10 to the power")),
     (("√", "func:√(", "Square root"), ("∛", "func:∛(", "Cube root")),
     ("e", "const:e")],
    [(("x²", "post:²", "Square"), ("x³", "post:³", "Cube")),
     ("xʸ", "op:^"), ("x!", "post:!"), ("1/x", "post:⁻¹")],
    [("|x|", "func:abs("), ("EXP", "exp:"), ("Ans", "const:Ans"), ("DEG", "angle")],
]

SCI_TOOLTIPS = {
    "inv": "Inverse functions", "open:": "Open bracket", "close:": "Close bracket",
    "op: mod ": "Remainder", "const:π": "Pi", "const:e": "Euler’s number",
    "op:^": "Power", "post:!": "Factorial", "post:⁻¹": "Reciprocal",
    "func:abs(": "Absolute value", "exp:": "Times ten to the power", "const:Ans": "Last answer",
    "angle": "Switch between degrees and radians",
}

KEYMAP = {
    "plus": "op:+", "KP_Add": "op:+", "minus": "op:−", "KP_Subtract": "op:−",
    "asterisk": "op:×", "KP_Multiply": "op:×", "slash": "op:÷", "KP_Divide": "op:÷",
    "period": "dot:", "comma": "dot:", "KP_Decimal": "dot:", "KP_Separator": "dot:",
    "percent": "pct:", "Return": "eq:", "KP_Enter": "eq:", "equal": "eq:",
    "BackSpace": "back:", "Delete": "clear:", "KP_Delete": "clear:",
    "parenleft": "open:", "parenright": "close:", "asciicircum": "op:^",
    "exclam": "post:!",
}
SCI_LETTERS = {
    "s": "func:sin(", "c": "func:cos(", "t": "func:tan(", "l": "func:ln(",
    "g": "func:log(", "r": "func:√(", "p": "const:π", "e": "const:e", "E": "exp:",
}
DEBUG = bool(os.environ.get("CALCULATOR_DEBUG"))

for _d in "0123456789":
    KEYMAP[_d] = f"digit:{_d}"
    KEYMAP[f"KP_{_d}"] = f"digit:{_d}"


def _debug(message):
    if DEBUG:
        print(f"calculator: {message}", flush=True)


class CalculatorWindow(Gtk.ApplicationWindow):
    def __init__(self, app):
        super().__init__(application=app, title=APP_NAME)
        self.app = app
        self.settings = app.settings
        self.history = app.history
        self.state = app.window_state
        self.calc = Calculator()
        self.calc.scientific = self.settings["scientific"]
        self.calc.degrees = self.settings["angle_unit"] == "deg"
        self.inverse = False
        self.key_widgets = {}  # action -> KeyMotion
        self.sci_buttons = []  # (Animated, Button, normal, inverse) for INV
        self._mode = "scientific" if self.calc.scientific else "standard"

        self.add_css_class("calculator")
        self._apply_size_for(self._mode, initial=True)

        self._build_header()
        self._build_body()
        self._connect_input()
        self._apply_settings()

        self.settings.connect(self._on_setting)
        self.history.connect(self._rebuild_history)
        app.theme.connect(self._on_theme)
        self.connect("close-request", self._on_close)
        self.connect("notify::default-width", self._on_resize)
        self.connect("notify::default-height", self._on_resize)
        self.refresh()

    # ------------------------------------------------------------ layout
    def _build_header(self):
        header = Gtk.HeaderBar()
        header.set_title_widget(Gtk.Box())  # clean top bar; title is for the WM

        group = Gtk.Box(css_classes=["connected-group"])
        self.std_toggle = Gtk.ToggleButton(child=icon("calculate"), tooltip_text="Standard")
        self.sci_toggle = Gtk.ToggleButton(child=icon("function"), tooltip_text="Scientific (Ctrl+S)")
        self.sci_toggle.set_group(self.std_toggle)
        for b in (self.std_toggle, self.sci_toggle):
            b.set_focus_on_click(False)
            group.append(b)
        (self.sci_toggle if self.calc.scientific else self.std_toggle).set_active(True)
        self.sci_toggle.connect("toggled", self._on_mode_toggled)
        header.pack_start(group)

        prefs = icon_button("settings", "Preferences (Ctrl+,)")
        prefs.connect("clicked", lambda *_: self.app.show_preferences())
        header.pack_end(prefs)

        self.history_btn = Gtk.ToggleButton(child=icon("history"), tooltip_text="History (Ctrl+H)")
        self.history_btn.add_css_class("icon-btn")
        self.history_btn.set_focus_on_click(False)
        self.history_btn.connect("toggled", lambda b: self._set_history_open(b.get_active()))
        header.pack_end(self.history_btn)
        self.set_titlebar(header)

    def _build_body(self):
        overlay = Gtk.Overlay()
        self.set_child(overlay)

        root = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        overlay.set_child(root)

        # Display. Wrapped in a WindowHandle so it also drags the window.
        handle = Gtk.WindowHandle(vexpand=True)
        display = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, valign=Gtk.Align.END,
                          spacing=2, css_classes=["display"])
        handle.set_child(display)
        root.append(handle)

        top = Gtk.Box()
        self.angle_chip = Gtk.Label(css_classes=["chip"], valign=Gtk.Align.CENTER)
        top.append(self.angle_chip)
        top.append(Gtk.Box(hexpand=True))
        self.prev_line = Gtk.Label(xalign=1, ellipsize=Pango.EllipsizeMode.START,
                                   css_classes=["prev-line"], hexpand=True)
        self.prev_anim = Animated(self.prev_line, hexpand=True)
        top.append(self.prev_anim)
        display.append(top)

        self.expr_label = Gtk.Label(xalign=1, halign=Gtk.Align.END, css_classes=["expr"],
                                    selectable=False)
        self._expr_px = None
        self.expr_scroll = Gtk.ScrolledWindow(
            hscrollbar_policy=Gtk.PolicyType.EXTERNAL, vscrollbar_policy=Gtk.PolicyType.NEVER,
            propagate_natural_height=True, hexpand=True)
        self.expr_scroll.set_child(self.expr_label)
        # Refit the text whenever the display's width changes.
        self.expr_scroll.get_hadjustment().connect(
            "notify::page-size", lambda *_: self._fit_text(self.expr_label.get_label()))
        self.expr_anim = Animated(self.expr_scroll)
        display.append(self.expr_anim)

        self.preview_label = Gtk.Label(xalign=1, ellipsize=Pango.EllipsizeMode.START,
                                       css_classes=["preview"])
        self.preview_anim = Animated(self.preview_label)
        display.append(self.preview_anim)

        # Keypad
        sheet = Gtk.Box(css_classes=["keypad-sheet"])
        self.sci_revealer = Gtk.Revealer(
            transition_type=Gtk.RevealerTransitionType.SLIDE_RIGHT,
            transition_duration=320, reveal_child=self.calc.scientific,
            hexpand=self.calc.scientific)
        self.sci_revealer.connect("notify::child-revealed", self._on_sci_revealed)
        self.sci_revealer.set_child(self._build_sci_grid())
        sheet.append(self.sci_revealer)
        sheet.append(self._build_standard_grid())
        root.append(sheet)

        # History sheet, scrim and snackbar on top
        self.scrim = Gtk.Box(css_classes=["scrim"], hexpand=True, vexpand=True, visible=False)
        self.scrim_anim = Animated(self.scrim)
        click = Gtk.GestureClick()
        click.connect("released", lambda *_: self.history_btn.set_active(False))
        self.scrim.add_controller(click)
        overlay.add_overlay(self.scrim_anim)
        self.scrim_anim.set_can_target(False)

        self.history_sheet = self._build_history_sheet()
        self.sheet_anim = Animated(self.history_sheet, valign=Gtk.Align.END)
        self.sheet_anim.set_visible(False)
        overlay.add_overlay(self.sheet_anim)

        self.snack_label = Gtk.Label(wrap=True)
        snack = Gtk.Box(css_classes=["snackbar"])
        snack.append(self.snack_label)
        self.snack_anim = Animated(snack, valign=Gtk.Align.END, halign=Gtk.Align.CENTER)
        self.snack_anim.set_visible(False)
        self.snack_anim.set_can_target(False)
        overlay.add_overlay(self.snack_anim)
        self._snack_timeout = 0

    def _make_key(self, label, action, css, tooltip):
        button = Gtk.Button(hexpand=True, vexpand=True, focus_on_click=False)
        button._action = action
        if label.startswith("icon:"):
            button.set_child(icon(label[5:]))
        else:
            button.set_label(label)
        button.add_css_class("key")
        button.add_css_class(css)
        if tooltip:
            button.set_tooltip_text(tooltip)
        button.connect("clicked", lambda b: self.press(b._action))
        if action == "dot:":
            self.dot_button = button
        # "=" rests as a rounded square and turns round when pressed: the
        # inverse of the other keys.
        rest, pressed = (0.5, 1.0) if css == "equals" else (1.0, 0.5)
        anim = Animated(button, shape=rest, hexpand=True, vexpand=True)
        self.key_widgets[action] = KeyMotion(anim, button, rest=rest, pressed=pressed)
        return anim, button

    def _build_standard_grid(self):
        grid = Gtk.Grid(row_spacing=10, column_spacing=10, row_homogeneous=True,
                        column_homogeneous=True, hexpand=True, vexpand=True)
        for r, row in enumerate(STANDARD_KEYS):
            for c, (label, action, css, tip) in enumerate(row):
                anim, _ = self._make_key(label, action, css, tip)
                grid.attach(anim, c, r, 1, 1)
        return grid

    def _build_sci_grid(self):
        grid = Gtk.Grid(row_spacing=10, column_spacing=10, row_homogeneous=True,
                        column_homogeneous=True, vexpand=True, css_classes=["sci-panel"])
        grid.set_size_request(280, -1)
        self.sci_anims = []
        for r, row in enumerate(SCI_KEYS):
            for c, spec in enumerate(row):
                if isinstance(spec[0], tuple):  # has an inverse variant
                    normal, inverse = spec
                    anim, button = self._make_key(normal[0], normal[1], "sci", normal[2])
                    self.sci_buttons.append((button, normal, inverse))
                elif spec[1] in ("inv", "angle"):
                    button = Gtk.ToggleButton(label=spec[0], hexpand=True, vexpand=True,
                                              focus_on_click=False, css_classes=["key", "sci"],
                                              tooltip_text=SCI_TOOLTIPS[spec[1]])
                    anim = Animated(button, shape=1.0, hexpand=True, vexpand=True)
                    self.key_widgets[spec[1]] = KeyMotion(anim, button)
                    if spec[1] == "inv":
                        self.inv_button = button
                        button.connect("toggled", self._on_inverse)
                    else:
                        self.angle_button = button
                        button.set_active(not self.calc.degrees)
                        button.connect("toggled", self._on_angle_toggled)
                else:
                    anim, button = self._make_key(spec[0], spec[1], "sci", SCI_TOOLTIPS.get(spec[1]))
                grid.attach(anim, c, r, 1, 1)
                self.sci_anims.append(anim)
        return grid

    def _build_history_sheet(self):
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, css_classes=["sheet"])

        handle_area = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        handle_area.append(Gtk.Box(css_classes=["drag-handle"], halign=Gtk.Align.CENTER))
        header = Gtk.Box(spacing=4)
        header.set_margin_start(16)
        header.set_margin_end(4)
        header.set_margin_bottom(4)
        header.append(Gtk.Label(label="History", xalign=0, hexpand=True, css_classes=["sheet-title"]))
        self.clear_history_btn = icon_button("delete_sweep", "Clear history")
        self.clear_history_btn.connect("clicked", lambda *_: self._clear_history())
        header.append(self.clear_history_btn)
        close = icon_button("close", "Close (Esc)")
        close.connect("clicked", lambda *_: self.history_btn.set_active(False))
        header.append(close)
        handle_area.append(header)
        box.append(handle_area)

        # Drag the handle down to dismiss.
        drag = Gtk.GestureDrag()
        drag.connect("drag-update", self._on_sheet_drag)
        drag.connect("drag-end", self._on_sheet_drag_end)
        handle_area.add_controller(drag)

        self.history_list = Gtk.ListBox(selection_mode=Gtk.SelectionMode.NONE)
        self.history_list.connect("row-activated", self._on_history_row)
        scroll = Gtk.ScrolledWindow(vexpand=True, hscrollbar_policy=Gtk.PolicyType.NEVER)
        scroll.set_child(self.history_list)

        empty = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8, valign=Gtk.Align.CENTER,
                        css_classes=["empty-state"])
        big = icon("history_toggle_off", 48)
        big.add_css_class("big")
        empty.append(big)
        empty.append(Gtk.Label(label="No calculations yet"))
        self.history_empty_hint = Gtk.Label(css_classes=["pref-subtitle"], wrap=True,
                                            justify=Gtk.Justification.CENTER)
        empty.append(self.history_empty_hint)

        self.history_stack = Gtk.Stack(transition_type=Gtk.StackTransitionType.CROSSFADE, vexpand=True)
        self.history_stack.add_named(scroll, "list")
        self.history_stack.add_named(empty, "empty")
        box.append(self.history_stack)
        self._rebuild_history()
        return box

    # ----------------------------------------------------------- input
    def _connect_input(self):
        keys = Gtk.EventControllerKey()
        keys.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
        keys.connect("key-pressed", self._on_key)
        self.add_controller(keys)

    def _on_key(self, _ctrl, keyval, _code, mods):
        name = Gdk.keyval_name(keyval) or ""
        ctrl = bool(mods & Gdk.ModifierType.CONTROL_MASK)
        if ctrl:
            lower = name.lower()
            if lower == "c":
                self.copy_result()
            elif lower == "v":
                self.paste()
            elif lower == "h":
                self.history_btn.set_active(not self.history_btn.get_active())
            elif lower == "s":
                self.sci_toggle.set_active(not self.sci_toggle.get_active())
                if not self.sci_toggle.get_active():
                    self.std_toggle.set_active(True)
            elif name == "comma":
                self.app.show_preferences()
            elif lower in ("q", "w"):
                self.close()
            else:
                return False
            return True
        if mods & Gdk.ModifierType.ALT_MASK:
            return False
        if name == "Escape":
            if self.history_btn.get_active():
                self.history_btn.set_active(False)
            else:
                self.press("clear:")
            return True
        action = KEYMAP.get(name)
        if action is None and self.calc.scientific:
            action = SCI_LETTERS.get(name)
        if action is None:
            return False
        key = self.key_widgets.get(action)
        if key is not None and key.anim.get_mapped():
            key.pulse()
        self.press(action)
        return True

    def press(self, action):
        status = self.calc.press(action)
        if action.startswith("func:") and self.inverse:
            self.inv_button.set_active(False)
        if status == "error":
            self.refresh(error=self.calc.error)
            self.expr_anim.shake()
            return
        if status == "result":
            entry = self.calc.prev_line
            value = self.calc.ans
            self.history.add(entry, format_number(value, grouping=False), value)
            self.refresh(animate_result=True)
            return
        if status == "ok":
            self.refresh()

    # ----------------------------------------------------------- display
    def refresh(self, error=None, animate_result=False):
        s = self.settings
        decimals = None if s["decimals"] < 0 else s["decimals"]
        grouping = s["digit_grouping"]
        text = self.calc.display(decimals, grouping)
        self.expr_label.set_label(text)
        self._fit_text(text)
        if self.calc.evaluated:
            self.expr_label.add_css_class("result")
        else:
            self.expr_label.remove_css_class("result")

        if self.calc.evaluated and self.calc.prev_line:
            from .engine import pretty
            self.prev_line.set_label(
                localize(pretty(self.calc.prev_line, grouping), self.calc.decimal) + " =")
        else:
            self.prev_line.set_label("")

        if error:
            self.preview_label.set_label(error)
            self.preview_label.add_css_class("error")
        else:
            self.preview_label.remove_css_class("error")
            preview = self.calc.preview(decimals, grouping) if s["live_preview"] else ""
            self.preview_label.set_label(preview)

        if animate_result:
            self.expr_anim.enter_from(dy=26, scale=0.97)
            self.prev_anim.opacity.set(0.0)
            self.prev_anim.opacity.animate_to(1.0, "effects")
        GLib.idle_add(self._scroll_to_end)

    TEXT_SIZES = {"small": 46, "medium": 58, "large": 70}

    def _fit_text(self, text):
        """Shrink the expression smoothly to fit the width, down to 45% of the
        chosen size; past that it scrolls. Measured on the label's own layout,
        so CSS font features such as tabular digits are included."""
        width = self.expr_scroll.get_width()
        if width <= 1:
            return
        base = self.TEXT_SIZES[self.settings["text_size"]]

        def measure(px):
            attrs = Pango.AttrList()
            attrs.insert(Pango.attr_size_new_absolute(int(px * Pango.SCALE)))
            self.expr_label.set_attributes(attrs)
            return self.expr_label.get_layout().get_pixel_size()[0] + 4

        px = base
        text_w = measure(px)
        for _ in range(3):
            if text_w <= width or px <= base * 0.45:
                break
            px = max(base * 0.45, px * width / text_w)
            text_w = measure(px)
        self._expr_px = px

    def _scroll_to_end(self):
        adj = self.expr_scroll.get_hadjustment()
        adj.set_value(adj.get_upper() - adj.get_page_size())
        return GLib.SOURCE_REMOVE

    # ----------------------------------------------------------- modes
    def _on_mode_toggled(self, button):
        scientific = button.get_active()
        if scientific != self.settings["scientific"]:
            self.settings.set("scientific", scientific)

    def _set_mode(self, scientific):
        new_mode = "scientific" if scientific else "standard"
        if new_mode == self._mode:
            return
        self._store_size()
        self._mode = new_mode
        self._mode_changed_at = GLib.get_monotonic_time()
        self._shrunk_for_switch = False
        _debug(f"mode -> {new_mode}; window {self.get_width()}x{self.get_height()}")
        self.calc.scientific = scientific
        if self.sci_toggle.get_active() != scientific:
            (self.sci_toggle if scientific else self.std_toggle).set_active(True)
        if scientific:
            self._apply_size_for(new_mode)  # grow first, then slide in
            self.sci_revealer.set_hexpand(True)
            self.sci_revealer.set_reveal_child(True)
            self._stagger_in()
        else:
            self.sci_revealer.set_reveal_child(False)  # shrink after the slide
            # Don't rely only on the revealer's signal: shrink after the slide's
            # duration even if the signal never arrives.
            delay = self.sci_revealer.get_transition_duration() + 60
            GLib.timeout_add(delay, self._shrink_after_slide)
        self._update_chip()
        self.refresh()

    def _stagger_in(self):
        if not motion.enabled:
            return
        for i, anim in enumerate(self.sci_anims):
            anim.scale.set(0.85)
            anim.opacity.set(0.0)

            def go(a=anim):
                a.scale.animate_to(1.0, "default")
                a.opacity.animate_to(1.0, "effects")
                return GLib.SOURCE_REMOVE
            GLib.timeout_add(60 + (i % 4) * 18 + (i // 4) * 24, go)

    def _on_sci_revealed(self, revealer, _pspec):
        _debug(f"scientific panel revealed={revealer.get_child_revealed()}, mode={self._mode}")
        if not revealer.get_child_revealed() and self._mode == "standard":
            self._shrink_after_slide()

    def _shrink_after_slide(self):
        if self._mode != "standard" or getattr(self, "_shrunk_for_switch", False):
            return GLib.SOURCE_REMOVE
        self._shrunk_for_switch = True
        self.sci_revealer.set_hexpand(False)  # expand flags propagate from the keys
        self._shrink_to_mode("standard")
        return GLib.SOURCE_REMOVE

    def _shrink_to_mode(self, mode):
        """Ask for the smaller size. Some compositors (Hyprland, for one) keep a
        floating window's size no matter what the app asks; if the window is
        still too wide shortly after, it is unmapped and shown again at the new
        size, which every compositor accepts.

        The compositor's "tiled" flag is deliberately not consulted: Hyprland
        sets it on floating windows too, so it can't tell the two apart."""
        if self._fills_screen():
            _debug("shrink skipped: window fills the screen")
            return
        self._apply_size_for(mode)
        width, height = self.get_default_size()
        _debug(f"shrink: asked for {width}x{height}, now {self.get_width()}x{self.get_height()}")

        def check():
            if self._mode != mode or not self.get_mapped():
                return GLib.SOURCE_REMOVE
            _debug(f"shrink check: wanted {width}, got {self.get_width()}")
            if self.get_width() > width + 24:
                _debug("compositor kept the old size; reopening the window")
                self._remap(width, height)
            return GLib.SOURCE_REMOVE
        GLib.timeout_add(600 if compositor.is_hyprland() else 250, check)

    def _remap(self, width, height):
        self._remapping = True
        self._mode_changed_at = GLib.get_monotonic_time()
        self.set_visible(False)
        self.set_default_size(width, height)

        def show():
            self.present()
            self._remapping = False
            return GLib.SOURCE_REMOVE
        GLib.idle_add(show)

    def _on_inverse(self, button):
        self.inverse = button.get_active()
        for btn, normal, inverse in self.sci_buttons:
            label, action, tip = inverse if self.inverse else normal
            btn.set_label(label)
            btn.set_tooltip_text(tip)
            btn._action = action

    def _on_angle_toggled(self, button):
        unit = "rad" if button.get_active() else "deg"
        if unit != self.settings["angle_unit"]:
            self.settings.set("angle_unit", unit)

    def _update_chip(self):
        rad = self.settings["angle_unit"] == "rad"
        self.angle_chip.set_label("RAD" if rad else "DEG")
        self.angle_chip.set_visible(self.calc.scientific)
        if hasattr(self, "angle_button"):
            self.angle_button.set_label("RAD" if rad else "DEG")
            if self.angle_button.get_active() != rad:
                self.angle_button.set_active(rad)

    # ----------------------------------------------------------- history
    def _rebuild_history(self):
        child = self.history_list.get_first_child()
        while child is not None:
            nxt = child.get_next_sibling()
            self.history_list.remove(child)
            child = nxt
        grouping = self.settings["digit_grouping"]
        decimal = self.calc.decimal
        from .engine import pretty
        for item in self.history.items:
            row = Gtk.ListBoxRow()
            row._value = item["value"]
            box = Gtk.Box(spacing=8, css_classes=["history-row"])
            texts = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, hexpand=True)
            texts.append(Gtk.Label(label=localize(pretty(item["expr"], grouping), decimal) + " =",
                                   xalign=1,
                                   ellipsize=Pango.EllipsizeMode.START, css_classes=["h-expr"]))
            texts.append(Gtk.Label(label=localize(format_number(item["value"], grouping=grouping),
                                                  decimal), xalign=1,
                                   ellipsize=Pango.EllipsizeMode.END, css_classes=["h-result"],
                                   selectable=False))
            box.append(texts)
            copy = icon_button("content_copy", "Copy result")
            copy.set_valign(Gtk.Align.CENTER)
            copy.connect("clicked", lambda _b, v=item["value"]: self._copy_value(v))
            box.append(copy)
            row.set_child(box)
            row.set_tooltip_text("Use this result")
            self.history_list.append(row)
        has_items = bool(self.history.items)
        self.history_stack.set_visible_child_name("list" if has_items else "empty")
        self.clear_history_btn.set_sensitive(has_items)
        self.history_empty_hint.set_label(
            "Results appear here after you press =" if self.settings["save_history"]
            else "History is kept only until you close the app. You can change this in Preferences.")

    def _on_history_row(self, _list, row):
        self.calc.use_value(row._value)
        self.history_btn.set_active(False)
        self.refresh()

    def _clear_history(self):
        self.history.clear()
        self.show_snackbar("History cleared")

    def _set_history_open(self, opening):
        height = max(self.get_height(), 300)
        sheet_h = int(height * 0.74)
        self.history_sheet.set_size_request(-1, sheet_h)
        self._sheet_open = opening
        if opening:
            self.sheet_anim.set_visible(True)
            self.scrim.set_visible(True)
            self.scrim_anim.set_can_target(True)
            self.sheet_anim.y.set(sheet_h)
            self.sheet_anim.y.animate_to(0.0, "default")
            self.scrim_anim.opacity.set(0.0)
            self.scrim_anim.opacity.animate_to(1.0, "effects")
        else:
            self.scrim_anim.set_can_target(False)

            def hide():
                if not self._sheet_open:
                    self.sheet_anim.set_visible(False)
                    self.scrim.set_visible(False)
            self.sheet_anim.y.animate_to(sheet_h + 40, "fast", done=hide)
            self.scrim_anim.opacity.animate_to(0.0, "effects")

    def _on_sheet_drag(self, _gesture, _dx, dy):
        self.sheet_anim.y.set(max(0.0, dy))

    def _on_sheet_drag_end(self, _gesture, _dx, dy):
        if dy > 90:
            self.history_btn.set_active(False)
        else:
            self.sheet_anim.y.animate_to(0.0, "default")

    # ----------------------------------------------------------- clipboard
    def copy_result(self):
        decimals = None if self.settings["decimals"] < 0 else self.settings["decimals"]
        text = self.calc.copy_text(decimals)
        self.get_clipboard().set(text)
        self.show_snackbar(f"Copied {text}")

    def _copy_value(self, value):
        text = localize(format_number(value, grouping=False, pretty_minus=False), self.calc.decimal)
        self.get_clipboard().set(text)
        self.show_snackbar(f"Copied {text}")

    def paste(self):
        def done(clipboard, result):
            try:
                text = clipboard.read_text_finish(result)
            except GLib.Error:
                return
            if text and self.calc.paste(text) == "ok":
                self.refresh()
        self.get_clipboard().read_text_async(None, done)

    def show_snackbar(self, text):
        self.snack_label.set_label(text)
        self.snack_anim.set_visible(True)
        self.snack_anim.enter_from(dy=60, scale=0.92)
        if self._snack_timeout:
            GLib.source_remove(self._snack_timeout)

        def hide():
            self._snack_timeout = 0
            self.snack_anim.opacity.animate_to(0.0, "effects",
                                               done=lambda: self.snack_anim.set_visible(False))
            self.snack_anim.y.animate_to(30.0, "default")
            return GLib.SOURCE_REMOVE
        self._snack_timeout = GLib.timeout_add(2400, hide)

    # ----------------------------------------------------------- settings
    def _apply_settings(self):
        s = self.settings
        for size in ("small", "medium", "large"):
            self.remove_css_class(f"text-{size}")
        self.add_css_class(f"text-{s['text_size']}")
        for size in ("compact", "standard", "large"):
            self.remove_css_class(f"keys-{size}")
        self.add_css_class(f"keys-{s['button_size']}")
        self.calc.degrees = s["angle_unit"] == "deg"
        self.calc.decimal = "," if s["decimal_separator"] == "comma" else "."
        self.dot_button.set_label(self.calc.decimal)
        self._update_chip()
        self._apply_motion()

    def _apply_motion(self):
        choice = self.settings["motion"]
        if choice == "system":
            scheme = None if self.app.theme.reduced_motion() else "expressive"
        else:
            scheme = None if choice == "off" else choice
        motion.scheme = scheme
        if scheme is None:
            self.add_css_class("no-motion")
        else:
            self.remove_css_class("no-motion")
        duration = 320 if scheme else 0
        self.sci_revealer.set_transition_duration(duration)

    def _on_setting(self, key, value):
        if key == "scientific":
            self._set_mode(value)
        elif key == "decimal_separator":
            self._apply_settings()
            self._rebuild_history()
            self.refresh()
        elif key in ("digit_grouping", "save_history"):
            self._rebuild_history()
            self.refresh()
        elif key in ("decimals", "live_preview"):
            self.refresh()
        else:
            self._apply_settings()
            self.refresh()

    def _on_theme(self):
        if self.settings["motion"] == "system":
            self._apply_motion()

    # ----------------------------------------------------------- size
    def _apply_size_for(self, mode, initial=False):
        if self.settings["remember_size"]:
            width, height = self.state.size_for(mode)
        else:
            width, height = DEFAULT_SIZES[mode]
        if initial:
            self.set_default_size(width, height)
            return
        if self._fills_screen():
            return
        self.set_default_size(width, height)
        if compositor.is_hyprland() and self.get_mapped():
            # Hyprland ignores the app's own request for floating windows.
            _debug(f"asking Hyprland for {width}x{height}")
            compositor.hyprland_resize(
                width, height, lambda ok: _debug(f"Hyprland resize {'done' if ok else 'failed'}"), _debug)

    def _fills_screen(self):
        """True only when the window really is maximized or fullscreen.

        Some compositors (Hyprland) report floating windows as maximized, so
        the flag alone can't be trusted: a window that is reported maximized
        but covers well under the monitor is treated as a normal window."""
        if self.is_fullscreen():
            return True
        if not self.is_maximized():
            return False
        surface = self.get_surface()
        display = self.get_display()
        monitor = display.get_monitor_at_surface(surface) if surface is not None else None
        if monitor is None:
            return True
        geo = monitor.get_geometry()
        width, height = self.get_width(), self.get_height()
        fills = width >= geo.width * 0.85 and height >= geo.height * 0.85
        if not fills:
            _debug(f"reported maximized but {width}x{height} on a {geo.width}x{geo.height} "
                   "monitor; treating as floating")
        return fills

    def _store_size(self):
        if getattr(self, "_remapping", False):
            return
        # Right after a mode switch the size belongs to the transition, and a
        # compositor that refused the resize would otherwise get its wide size
        # saved as the standard size.
        if GLib.get_monotonic_time() - getattr(self, "_mode_changed_at", 0) < 1_500_000:
            return
        if self._fills_screen():
            return
        if not self.get_mapped():
            return
        if self.sci_revealer.get_reveal_child() != self.sci_revealer.get_child_revealed():
            return  # mid-transition; the size is not the user's
        # default-size tracks the real size in GTK4 and uses the same units
        # as set_default_size(), so restoring never drifts.
        width, height = self.get_default_size()
        if width > 0 and height > 0:
            self.state.remember(self._mode, width, height)

    def _on_resize(self, *_):
        if not hasattr(self, "_resize_save"):
            self._resize_save = 0
        if self._resize_save:
            GLib.source_remove(self._resize_save)

        def save():
            self._resize_save = 0
            self._store_size()
            if self.settings["remember_size"]:
                self.state.save()
            self.refresh()
            return GLib.SOURCE_REMOVE
        self._resize_save = GLib.timeout_add(400, save)

    def _on_close(self, *_):
        self._store_size()
        if self.settings["remember_size"]:
            self.state.save()
        return False
