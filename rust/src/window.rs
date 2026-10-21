//! Main calculator window. Port of `calculator/window.py`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use calculator::engine::{format_number, localize, pretty, Format};
use calculator::logic::{Calculator, Status};
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};

use crate::compositor;
use crate::config::{History, Mode, Settings, Value, WindowState, APP_NAME};
use crate::icons::{icon, icon_button};
use crate::motion::{self, Anim, Animated, KeyMotion, Kind, Scheme};
use crate::theme::Theme;

/// A closure that holds the window weakly and does nothing once it is gone.
macro_rules! on {
    ($rc:expr, $win:ident, || $body:expr) => {{
        let weak: Weak<Win> = Rc::downgrade($rc);
        move || {
            if let Some($win) = weak.upgrade() {
                let $win = &$win;
                $body
            }
        }
    }};
    ($rc:expr, $win:ident, |$($arg:pat_param),*| $body:expr) => {{
        let weak: Weak<Win> = Rc::downgrade($rc);
        move |$($arg),*| {
            if let Some($win) = weak.upgrade() {
                let $win = &$win;
                $body
            }
        }
    }};
}

// (label or icon:name, action, css class, tooltip)
type KeySpec = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
);

const STANDARD_KEYS: [[KeySpec; 4]; 5] = [
    [
        ("AC", "clear:", "clear", Some("Clear (Esc)")),
        ("±", "neg:", "util", Some("Change sign")),
        ("%", "pct:", "util", Some("Percent")),
        ("÷", "op:÷", "op", Some("Divide")),
    ],
    [
        ("7", "digit:7", "digit", None),
        ("8", "digit:8", "digit", None),
        ("9", "digit:9", "digit", None),
        ("×", "op:×", "op", Some("Multiply")),
    ],
    [
        ("4", "digit:4", "digit", None),
        ("5", "digit:5", "digit", None),
        ("6", "digit:6", "digit", None),
        ("−", "op:−", "op", Some("Subtract")),
    ],
    [
        ("1", "digit:1", "digit", None),
        ("2", "digit:2", "digit", None),
        ("3", "digit:3", "digit", None),
        ("+", "op:+", "op", Some("Add")),
    ],
    [
        ("0", "digit:0", "digit", None),
        (".", "dot:", "digit", None),
        ("icon:backspace", "back:", "util", Some("Backspace")),
        ("=", "eq:", "equals", Some("Equals (Enter)")),
    ],
];

/// (label, action, tooltip)
type Variant = (&'static str, &'static str, &'static str);

enum SciKey {
    Plain(&'static str, &'static str),
    /// Normal and inverse (INV) variants.
    Variants(Variant, Variant),
    /// INV or DEG/RAD.
    Toggle(&'static str, &'static str),
}

use SciKey::{Plain, Toggle, Variants};

const SCI_KEYS: [[SciKey; 4]; 5] = [
    [
        Toggle("INV", "inv"),
        Plain("(", "open:"),
        Plain(")", "close:"),
        Plain("mod", "op: mod "),
    ],
    [
        Variants(
            ("sin", "func:sin(", "Sine"),
            ("sin⁻¹", "func:asin(", "Arcsine"),
        ),
        Variants(
            ("cos", "func:cos(", "Cosine"),
            ("cos⁻¹", "func:acos(", "Arccosine"),
        ),
        Variants(
            ("tan", "func:tan(", "Tangent"),
            ("tan⁻¹", "func:atan(", "Arctangent"),
        ),
        Plain("π", "const:π"),
    ],
    [
        Variants(
            ("ln", "func:ln(", "Natural logarithm"),
            ("eˣ", "func:e^(", "e to the power"),
        ),
        Variants(
            ("log", "func:log(", "Logarithm, base 10"),
            ("10ˣ", "func:10^(", "10 to the power"),
        ),
        Variants(
            ("√", "func:√(", "Square root"),
            ("∛", "func:∛(", "Cube root"),
        ),
        Plain("e", "const:e"),
    ],
    [
        Variants(("x²", "post:²", "Square"), ("x³", "post:³", "Cube")),
        Plain("xʸ", "op:^"),
        Plain("x!", "post:!"),
        Plain("1/x", "post:⁻¹"),
    ],
    [
        Plain("|x|", "func:abs("),
        Plain("EXP", "exp:"),
        Plain("Ans", "const:Ans"),
        Toggle("DEG", "angle"),
    ],
];

fn sci_tooltip(action: &str) -> Option<&'static str> {
    Some(match action {
        "inv" => "Inverse functions",
        "open:" => "Open bracket",
        "close:" => "Close bracket",
        "op: mod " => "Remainder",
        "const:π" => "Pi",
        "const:e" => "Euler’s number",
        "op:^" => "Power",
        "post:!" => "Factorial",
        "post:⁻¹" => "Reciprocal",
        "func:abs(" => "Absolute value",
        "exp:" => "Times ten to the power",
        "const:Ans" => "Last answer",
        "angle" => "Switch between degrees and radians",
        _ => return None,
    })
}

/// Keyboard key name -> calculator action.
fn keymap(name: &str) -> Option<String> {
    let action = match name {
        "plus" | "KP_Add" => "op:+",
        "minus" | "KP_Subtract" => "op:−",
        "asterisk" | "KP_Multiply" => "op:×",
        "slash" | "KP_Divide" => "op:÷",
        "period" | "comma" | "KP_Decimal" | "KP_Separator" => "dot:",
        "percent" => "pct:",
        "Return" | "KP_Enter" | "equal" => "eq:",
        "BackSpace" => "back:",
        "Delete" | "KP_Delete" => "clear:",
        "parenleft" => "open:",
        "parenright" => "close:",
        "asciicircum" => "op:^",
        "exclam" => "post:!",
        _ => {
            let digit = name.strip_prefix("KP_").unwrap_or(name);
            if digit.len() == 1 && digit.chars().all(|c| c.is_ascii_digit()) {
                return Some(format!("digit:{digit}"));
            }
            return None;
        }
    };
    Some(action.to_string())
}

/// Letters that type functions in scientific mode.
fn sci_letter(name: &str) -> Option<&'static str> {
    Some(match name {
        "s" => "func:sin(",
        "c" => "func:cos(",
        "t" => "func:tan(",
        "l" => "func:ln(",
        "g" => "func:log(",
        "r" => "func:√(",
        "p" => "const:π",
        "e" => "const:e",
        "E" => "exp:",
        _ => return None,
    })
}

fn debug(message: &str) {
    if std::env::var_os("CALCULATOR_DEBUG").is_some() {
        println!("calculator: {message}");
    }
}

/// Minimum width of the scientific keys.
const SCI_PANEL_WIDTH: i32 = 280;

const TEXT_SIZES: [(&str, f64); 3] = [("small", 46.0), ("medium", 58.0), ("large", 70.0)];

/// A keypad key whose action can change (the INV variants).
struct Key {
    button: gtk::Button,
    action: Rc<RefCell<String>>,
}

struct SciVariantKey {
    key: Key,
    normal: Variant,
    inverse: Variant,
}

pub struct Win {
    pub window: gtk::ApplicationWindow,
    settings: Rc<Settings>,
    history: Rc<History>,
    state: RefCell<WindowState>,
    theme: Rc<Theme>,
    calc: RefCell<Calculator>,
    inverse: Cell<bool>,
    mode: Cell<Mode>,
    key_motion: HashMap<String, Rc<KeyMotion>>,
    keys: Vec<Key>,
    sci_variants: Vec<SciVariantKey>,
    sci_anims: Vec<Animated>,

    std_toggle: gtk::ToggleButton,
    sci_toggle: gtk::ToggleButton,
    history_btn: gtk::ToggleButton,
    prefs_btn: gtk::Button,
    angle_chip: gtk::Label,
    prev_line: gtk::Label,
    prev_anim: Animated,
    expr_label: gtk::Label,
    expr_scroll: gtk::ScrolledWindow,
    expr_anim: Animated,
    preview_label: gtk::Label,
    sci_revealer: gtk::Revealer,
    inv_button: gtk::ToggleButton,
    angle_button: gtk::ToggleButton,
    dot_button: gtk::Button,

    scrim: gtk::Box,
    scrim_anim: Animated,
    history_sheet: gtk::Box,
    sheet_anim: Animated,
    sheet_handle: gtk::Box,
    history_list: gtk::ListBox,
    history_values: RefCell<Vec<f64>>,
    history_stack: gtk::Stack,
    clear_history_btn: gtk::Button,
    close_history_btn: gtk::Button,
    history_empty_hint: gtk::Label,
    snack_label: gtk::Label,
    snack_anim: Animated,

    snack_timeout: RefCell<Option<glib::SourceId>>,
    on_preferences: RefCell<Option<Rc<dyn Fn()>>>,
    resize_save: RefCell<Option<glib::SourceId>>,
    mode_changed_at: Cell<i64>,
    shrunk_for_switch: Cell<bool>,
    remapping: Cell<bool>,
    sheet_open: Cell<bool>,
    standard_grid: gtk::Grid,
    /// The standard keys are pinned while the scientific panel closes.
    pinned: Cell<bool>,
}

/// Widgets made while building, before the window exists as an `Rc`.
#[derive(Default)]
struct Parts {
    key_motion: HashMap<String, Rc<KeyMotion>>,
    keys: Vec<Key>,
    sci_variants: Vec<SciVariantKey>,
    sci_anims: Vec<Animated>,
    toggles: Vec<(gtk::ToggleButton, &'static str)>,
    dot_button: Option<gtk::Button>,
}

fn make_key(
    parts: &mut Parts,
    label: &str,
    action: &str,
    css: &str,
    tooltip: Option<&str>,
) -> (Animated, Key) {
    let button = gtk::Button::builder()
        .hexpand(true)
        .vexpand(true)
        .focus_on_click(false)
        .build();
    match label.strip_prefix("icon:") {
        Some(name) => button.set_child(Some(&icon(name, 28))),
        None => button.set_label(label),
    }
    button.add_css_class("key");
    button.add_css_class(css);
    button.set_tooltip_text(tooltip);
    if action == "dot:" {
        parts.dot_button = Some(button.clone());
    }
    // "=" rests as a rounded square and turns round when pressed: the
    // inverse of the other keys.
    let (rest, pressed) = if css == "equals" {
        (0.5, 1.0)
    } else {
        (1.0, 0.5)
    };
    let anim = Animated::new(&button, Some(rest));
    anim.set_hexpand(true);
    anim.set_vexpand(true);
    parts.key_motion.insert(
        action.to_string(),
        KeyMotion::new(&anim, &button, rest, pressed),
    );
    (
        anim,
        Key {
            button,
            action: Rc::new(RefCell::new(action.to_string())),
        },
    )
}

fn key_grid() -> gtk::Grid {
    gtk::Grid::builder()
        .row_spacing(10)
        .column_spacing(10)
        .row_homogeneous(true)
        .column_homogeneous(true)
        .vexpand(true)
        .build()
}

fn build_standard_grid(parts: &mut Parts) -> gtk::Grid {
    let grid = key_grid();
    grid.set_hexpand(true);
    for (r, row) in STANDARD_KEYS.iter().enumerate() {
        for (c, &(label, action, css, tip)) in row.iter().enumerate() {
            let (anim, key) = make_key(parts, label, action, css, tip);
            grid.attach(&anim, c as i32, r as i32, 1, 1);
            parts.keys.push(key);
        }
    }
    grid
}

fn build_sci_grid(parts: &mut Parts) -> gtk::Grid {
    let grid = key_grid();
    grid.add_css_class("sci-panel");
    grid.set_size_request(SCI_PANEL_WIDTH, -1);
    for (r, row) in SCI_KEYS.iter().enumerate() {
        for (c, spec) in row.iter().enumerate() {
            let anim = match spec {
                Variants(normal, inverse) => {
                    let (anim, key) = make_key(parts, normal.0, normal.1, "sci", Some(normal.2));
                    parts.sci_variants.push(SciVariantKey {
                        key,
                        normal: *normal,
                        inverse: *inverse,
                    });
                    anim
                }
                Toggle(label, which) => {
                    let button = gtk::ToggleButton::builder()
                        .label(*label)
                        .hexpand(true)
                        .vexpand(true)
                        .focus_on_click(false)
                        .css_classes(["key", "sci"])
                        .build();
                    button.set_tooltip_text(sci_tooltip(which));
                    let anim = Animated::new(&button, Some(1.0));
                    anim.set_hexpand(true);
                    anim.set_vexpand(true);
                    parts
                        .key_motion
                        .insert(which.to_string(), KeyMotion::new(&anim, &button, 1.0, 0.55));
                    parts.toggles.push((button, which));
                    anim
                }
                Plain(label, action) => {
                    let (anim, key) = make_key(parts, label, action, "sci", sci_tooltip(action));
                    parts.keys.push(key);
                    anim
                }
            };
            grid.attach(&anim, c as i32, r as i32, 1, 1);
            parts.sci_anims.push(anim);
        }
    }
    grid
}

impl Win {
    pub fn new(
        app: &gtk::Application,
        settings: Rc<Settings>,
        history: Rc<History>,
        theme: Rc<Theme>,
    ) -> Rc<Self> {
        let window = gtk::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .build();
        window.add_css_class("calculator");
        let mut calc = Calculator::new();
        calc.scientific = settings.bool("scientific");
        calc.degrees = settings.str("angle_unit") == "deg";
        let mode = if calc.scientific {
            Mode::Scientific
        } else {
            Mode::Standard
        };

        // Header ------------------------------------------------------------
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0))); // clean top bar
        let group = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        group.add_css_class("connected-group");
        let std_toggle = gtk::ToggleButton::builder()
            .child(&icon("calculate", 20))
            .tooltip_text("Standard")
            .build();
        let sci_toggle = gtk::ToggleButton::builder()
            .child(&icon("function", 20))
            .tooltip_text("Scientific (Ctrl+S)")
            .build();
        sci_toggle.set_group(Some(&std_toggle));
        for b in [&std_toggle, &sci_toggle] {
            b.set_focus_on_click(false);
            group.append(b);
        }
        if calc.scientific {
            &sci_toggle
        } else {
            &std_toggle
        }
        .set_active(true);
        header.pack_start(&group);
        let prefs_btn = icon_button("settings", "Preferences (Ctrl+,)");
        header.pack_end(&prefs_btn);
        let history_btn = gtk::ToggleButton::builder()
            .child(&icon("history", 20))
            .tooltip_text("History (Ctrl+H)")
            .build();
        history_btn.add_css_class("icon-btn");
        history_btn.set_focus_on_click(false);
        header.pack_end(&history_btn);
        window.set_titlebar(Some(&header));

        // Body --------------------------------------------------------------
        let overlay = gtk::Overlay::new();
        window.set_child(Some(&overlay));
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        overlay.set_child(Some(&root));

        // Display. Wrapped in a WindowHandle so it also drags the window.
        let handle = gtk::WindowHandle::builder().vexpand(true).build();
        let display = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::End)
            .spacing(2)
            .css_classes(["display"])
            .build();
        handle.set_child(Some(&display));
        root.append(&handle);

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let angle_chip = gtk::Label::builder()
            .css_classes(["chip"])
            .valign(gtk::Align::Center)
            .build();
        top.append(&angle_chip);
        top.append(&gtk::Box::builder().hexpand(true).build());
        let prev_line = gtk::Label::builder()
            .xalign(1.0)
            .ellipsize(pango::EllipsizeMode::Start)
            .css_classes(["prev-line"])
            .hexpand(true)
            .build();
        let prev_anim = Animated::new(&prev_line, None);
        prev_anim.set_hexpand(true);
        top.append(&prev_anim);
        display.append(&top);

        let expr_label = gtk::Label::builder()
            .xalign(1.0)
            .halign(gtk::Align::End)
            .css_classes(["expr"])
            .selectable(false)
            .build();
        let expr_scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .hexpand(true)
            .child(&expr_label)
            .build();
        let expr_anim = Animated::new(&expr_scroll, None);
        display.append(&expr_anim);

        let preview_label = gtk::Label::builder()
            .xalign(1.0)
            .ellipsize(pango::EllipsizeMode::Start)
            .css_classes(["preview"])
            .build();
        display.append(&Animated::new(&preview_label, None));

        // Keypad
        let mut parts = Parts::default();
        let sheet = gtk::Box::builder().css_classes(["keypad-sheet"]).build();
        let sci_revealer = gtk::Revealer::builder()
            .transition_type(gtk::RevealerTransitionType::SlideRight)
            .transition_duration(320)
            .reveal_child(calc.scientific)
            .hexpand(calc.scientific)
            .build();
        sci_revealer.set_child(Some(&build_sci_grid(&mut parts)));
        sheet.append(&sci_revealer);
        let standard_grid = build_standard_grid(&mut parts);
        sheet.append(&standard_grid);
        root.append(&sheet);

        // History sheet, scrim and snackbar on top
        let scrim = gtk::Box::builder()
            .css_classes(["scrim"])
            .hexpand(true)
            .vexpand(true)
            .visible(false)
            .build();
        let scrim_anim = Animated::new(&scrim, None);
        overlay.add_overlay(&scrim_anim);
        scrim_anim.set_can_target(false);

        let history_sheet = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["sheet"])
            .build();
        let sheet_handle = gtk::Box::new(gtk::Orientation::Vertical, 0);
        sheet_handle.append(
            &gtk::Box::builder()
                .css_classes(["drag-handle"])
                .halign(gtk::Align::Center)
                .build(),
        );
        let sheet_header = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        sheet_header.set_margin_start(16);
        sheet_header.set_margin_end(4);
        sheet_header.set_margin_bottom(4);
        sheet_header.append(
            &gtk::Label::builder()
                .label("History")
                .xalign(0.0)
                .hexpand(true)
                .css_classes(["sheet-title"])
                .build(),
        );
        let clear_history_btn = icon_button("delete_sweep", "Clear history");
        sheet_header.append(&clear_history_btn);
        let close_history_btn = icon_button("close", "Close (Esc)");
        sheet_header.append(&close_history_btn);
        sheet_handle.append(&sheet_header);
        history_sheet.append(&sheet_handle);

        let history_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .build();
        let history_scroll = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&history_list)
            .build();
        let empty = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(8)
            .valign(gtk::Align::Center)
            .css_classes(["empty-state"])
            .build();
        let big = icon("history_toggle_off", 48);
        big.add_css_class("big");
        empty.append(&big);
        empty.append(&gtk::Label::new(Some("No calculations yet")));
        let history_empty_hint = gtk::Label::builder()
            .css_classes(["pref-subtitle"])
            .wrap(true)
            .justify(gtk::Justification::Center)
            .build();
        empty.append(&history_empty_hint);
        let history_stack = gtk::Stack::builder()
            .transition_type(gtk::StackTransitionType::Crossfade)
            .vexpand(true)
            .build();
        history_stack.add_named(&history_scroll, Some("list"));
        history_stack.add_named(&empty, Some("empty"));
        history_sheet.append(&history_stack);

        let sheet_anim = Animated::new(&history_sheet, None);
        sheet_anim.set_valign(gtk::Align::End);
        sheet_anim.set_visible(false);
        overlay.add_overlay(&sheet_anim);

        let snack_label = gtk::Label::builder().wrap(true).build();
        let snack = gtk::Box::builder().css_classes(["snackbar"]).build();
        snack.append(&snack_label);
        let snack_anim = Animated::new(&snack, None);
        snack_anim.set_valign(gtk::Align::End);
        snack_anim.set_halign(gtk::Align::Center);
        snack_anim.set_visible(false);
        snack_anim.set_can_target(false);
        overlay.add_overlay(&snack_anim);

        let toggle = |which: &str| {
            parts
                .toggles
                .iter()
                .find(|(_, w)| *w == which)
                .unwrap()
                .0
                .clone()
        };
        let (inv_button, angle_button) = (toggle("inv"), toggle("angle"));
        angle_button.set_active(!calc.degrees);

        let win = Rc::new(Win {
            window,
            settings,
            history,
            state: RefCell::new(WindowState::load_default()),
            theme,
            calc: RefCell::new(calc),
            inverse: Cell::new(false),
            mode: Cell::new(mode),
            key_motion: parts.key_motion,
            keys: parts.keys,
            sci_variants: parts.sci_variants,
            sci_anims: parts.sci_anims,
            std_toggle,
            sci_toggle,
            history_btn,
            prefs_btn,
            angle_chip,
            prev_line,
            prev_anim,
            expr_label,
            expr_scroll,
            expr_anim,
            preview_label,
            sci_revealer,
            inv_button,
            angle_button,
            dot_button: parts.dot_button.unwrap(),
            scrim,
            scrim_anim,
            history_sheet,
            sheet_anim,
            sheet_handle,
            history_list,
            history_values: RefCell::default(),
            history_stack,
            clear_history_btn,
            close_history_btn,
            history_empty_hint,
            snack_label,
            snack_anim,
            snack_timeout: RefCell::default(),
            on_preferences: RefCell::default(),
            resize_save: RefCell::default(),
            mode_changed_at: Cell::new(0),
            shrunk_for_switch: Cell::new(false),
            remapping: Cell::new(false),
            sheet_open: Cell::new(false),
            standard_grid,
            pinned: Cell::new(false),
        });
        win.apply_size_for(mode, true);
        win.connect_signals();
        win.rebuild_history();
        win.apply_settings();
        win.refresh(None, false);
        win
    }

    fn connect_signals(self: &Rc<Self>) {
        self.sci_toggle
            .connect_toggled(on!(self, win, |button| win.on_mode_toggled(button.is_active())));
        self.prefs_btn
            .connect_clicked(on!(self, win, |_| win.show_preferences()));
        self.history_btn
            .connect_toggled(on!(self, win, |button| win.set_history_open(button.is_active())));
        // Refit the text whenever the display's width changes.
        self.expr_scroll
            .hadjustment()
            .connect_page_size_notify(on!(self, win, |_| {
                win.fit_text(&win.expr_label.label());
            }));
        self.sci_revealer
            .connect_child_revealed_notify(on!(self, win, |revealer| {
                debug(&format!(
                    "scientific panel revealed={}, mode={:?}",
                    revealer.is_child_revealed(),
                    win.mode.get()
                ));
                if !revealer.is_child_revealed() && win.mode.get() == Mode::Standard {
                    win.shrink_after_slide();
                }
            }));
        for key in self
            .keys
            .iter()
            .chain(self.sci_variants.iter().map(|v| &v.key))
        {
            let action = key.action.clone();
            key.button.connect_clicked(on!(self, win, |_| {
                let action = action.borrow().clone();
                win.press(&action);
            }));
        }
        self.inv_button
            .connect_toggled(on!(self, win, |button| win.on_inverse(button.is_active())));
        self.angle_button.connect_toggled(on!(self, win, |button| {
            let unit = if button.is_active() { "rad" } else { "deg" };
            if unit != win.settings.str("angle_unit") {
                let _ = win.settings.set("angle_unit", Value::Str(unit.into()));
            }
        }));

        let click = gtk::GestureClick::new();
        click.connect_released(on!(self, win, |_, _, _, _| win
            .history_btn
            .set_active(false)));
        self.scrim.add_controller(click);
        self.clear_history_btn.connect_clicked(on!(self, win, |_| {
            win.history.clear();
            win.show_snackbar("History cleared");
        }));
        self.close_history_btn
            .connect_clicked(on!(self, win, |_| win.history_btn.set_active(false)));
        // Drag the handle down to dismiss.
        let drag = gtk::GestureDrag::new();
        drag.connect_drag_update(on!(self, win, |_, _, dy| win
            .sheet_anim
            .y()
            .set(dy.max(0.0))));
        drag.connect_drag_end(on!(self, win, |_, _, dy| {
            if dy > 90.0 {
                win.history_btn.set_active(false);
            } else {
                win.sheet_anim.y().animate_to(0.0, Kind::Default);
            }
        }));
        self.sheet_handle.add_controller(drag);
        self.history_list
            .connect_row_activated(on!(self, win, |_, row| {
                let value = win
                    .history_values
                    .borrow()
                    .get(row.index() as usize)
                    .copied();
                if let Some(value) = value {
                    win.calc.borrow_mut().use_value(value);
                    win.history_btn.set_active(false);
                    win.refresh(None, false);
                }
            }));

        self.theme.connect(on!(self, win, || {
            if win.settings.str("motion") == "system" {
                win.apply_motion();
            }
        }));

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, keyval, _, mods| match weak.upgrade() {
            Some(win) => win.on_key(keyval, mods),
            None => glib::Propagation::Proceed,
        });
        self.window.add_controller(keys);

        self.settings
            .connect(on!(self, win, |key| win.on_setting(key)));
        self.history
            .connect(on!(self, win, || win.rebuild_history()));
        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            if let Some(win) = weak.upgrade() {
                win.store_size();
                if win.settings.bool("remember_size") {
                    win.state.borrow().save();
                }
            }
            glib::Propagation::Proceed
        });
        self.window
            .connect_default_width_notify(on!(self, win, |_| win.on_resize()));
        self.window
            .connect_default_height_notify(on!(self, win, |_| win.on_resize()));
    }

    // ----------------------------------------------------------- input
    fn on_key(self: &Rc<Self>, keyval: gdk::Key, mods: gdk::ModifierType) -> glib::Propagation {
        let name = keyval.name().map(|n| n.to_string()).unwrap_or_default();
        if mods.contains(gdk::ModifierType::CONTROL_MASK) {
            match name.to_lowercase().as_str() {
                "c" => self.copy_result(),
                "v" => self.paste(),
                "h" => self.history_btn.set_active(!self.history_btn.is_active()),
                "s" => {
                    self.sci_toggle.set_active(!self.sci_toggle.is_active());
                    if !self.sci_toggle.is_active() {
                        self.std_toggle.set_active(true);
                    }
                }
                "comma" => self.show_preferences(),
                "q" | "w" => self.window.close(),
                _ => return glib::Propagation::Proceed,
            }
            return glib::Propagation::Stop;
        }
        if mods.contains(gdk::ModifierType::ALT_MASK) {
            return glib::Propagation::Proceed;
        }
        if name == "Escape" {
            if self.history_btn.is_active() {
                self.history_btn.set_active(false);
            } else {
                self.press("clear:");
            }
            return glib::Propagation::Stop;
        }
        let scientific = self.calc.borrow().scientific;
        let action = keymap(&name).or_else(|| {
            scientific
                .then(|| sci_letter(&name).map(str::to_string))
                .flatten()
        });
        let Some(action) = action else {
            return glib::Propagation::Proceed;
        };
        if let Some(key) = self.key_motion.get(&action) {
            if key.anim.is_mapped() {
                key.pulse();
            }
        }
        self.press(&action);
        glib::Propagation::Stop
    }

    pub fn press(self: &Rc<Self>, action: &str) {
        let status = self.calc.borrow_mut().press(action);
        if action.starts_with("func:") && self.inverse.get() {
            self.inv_button.set_active(false);
        }
        match status {
            Status::Error => {
                let error = self.calc.borrow().error.clone();
                self.refresh(error, false);
                self.expr_anim.shake();
            }
            Status::Result => {
                let (entry, value) = {
                    let calc = self.calc.borrow();
                    (calc.prev_line.clone(), calc.ans)
                };
                let result = format_number(
                    value,
                    &Format {
                        grouping: false,
                        ..Format::default()
                    },
                );
                self.history.add(&entry, &result, value);
                self.refresh(None, true);
            }
            Status::Ok => self.refresh(None, false),
            Status::Ignored => {}
        }
    }

    // ----------------------------------------------------------- display
    fn decimals(&self) -> Option<usize> {
        usize::try_from(self.settings.int("decimals")).ok()
    }

    pub fn refresh(self: &Rc<Self>, error: Option<String>, animate_result: bool) {
        let decimals = self.decimals();
        let grouping = self.settings.bool("digit_grouping");
        let calc = self.calc.borrow();
        let text = calc.display(decimals, grouping);
        self.expr_label.set_label(&text);
        self.fit_text(&text);
        if calc.evaluated {
            self.expr_label.add_css_class("result");
        } else {
            self.expr_label.remove_css_class("result");
        }

        if calc.evaluated && !calc.prev_line.is_empty() {
            self.prev_line.set_label(&format!(
                "{} =",
                localize(&pretty(&calc.prev_line, grouping), calc.decimal)
            ));
        } else {
            self.prev_line.set_label("");
        }

        match error {
            Some(error) => {
                self.preview_label.set_label(&error);
                self.preview_label.add_css_class("error");
            }
            None => {
                self.preview_label.remove_css_class("error");
                let preview = if self.settings.bool("live_preview") {
                    calc.preview(decimals, grouping)
                } else {
                    String::new()
                };
                self.preview_label.set_label(&preview);
            }
        }
        drop(calc);

        if animate_result {
            self.expr_anim.enter_from(26.0, 0.97);
            self.prev_anim.opacity().set(0.0);
            self.prev_anim.opacity().animate_to(1.0, Kind::Effects);
        }
        let scroll = self.expr_scroll.clone();
        glib::idle_add_local_once(move || {
            let adj = scroll.hadjustment();
            adj.set_value(adj.upper() - adj.page_size());
        });
    }

    /// Shrink the expression smoothly to fit the width, down to 45% of the
    /// chosen size; past that it scrolls. Measured on the label's own layout,
    /// so CSS font features such as tabular digits are included.
    fn fit_text(&self, _text: &str) {
        let width = f64::from(self.expr_scroll.width());
        if width <= 1.0 {
            return;
        }
        let size = self.settings.str("text_size");
        let base = TEXT_SIZES
            .iter()
            .find(|(name, _)| *name == size)
            .map_or(58.0, |(_, px)| *px);
        let measure = |px: f64| {
            let attrs = pango::AttrList::new();
            attrs.insert(pango::AttrSize::new_size_absolute(
                (px * f64::from(pango::SCALE)) as i32,
            ));
            self.expr_label.set_attributes(Some(&attrs));
            f64::from(self.expr_label.layout().pixel_size().0) + 4.0
        };
        let mut px = base;
        let mut text_w = measure(px);
        for _ in 0..3 {
            if text_w <= width || px <= base * 0.45 {
                break;
            }
            px = (base * 0.45).max(px * width / text_w);
            text_w = measure(px);
        }
    }

    // ----------------------------------------------------------- modes
    fn on_mode_toggled(&self, scientific: bool) {
        if scientific != self.settings.bool("scientific") {
            let _ = self.settings.set("scientific", Value::Bool(scientific));
        }
    }

    fn set_mode(self: &Rc<Self>, scientific: bool) {
        let new_mode = if scientific {
            Mode::Scientific
        } else {
            Mode::Standard
        };
        if new_mode == self.mode.get() {
            return;
        }
        self.store_size();
        self.mode.set(new_mode);
        self.mode_changed_at.set(glib::monotonic_time());
        self.shrunk_for_switch.set(false);
        debug(&format!(
            "mode -> {}; window {}x{}",
            new_mode.key(),
            self.window.width(),
            self.window.height()
        ));
        self.calc.borrow_mut().scientific = scientific;
        if self.sci_toggle.is_active() != scientific {
            if scientific {
                &self.sci_toggle
            } else {
                &self.std_toggle
            }
            .set_active(true);
        }
        if scientific {
            self.unpin_standard();
            self.apply_size_for(new_mode, false); // grow first, then slide in
            self.sci_revealer.set_hexpand(true);
            self.sci_revealer.set_reveal_child(true);
            self.stagger_in();
        } else {
            self.pin_standard(); // the keys keep their size while the panel slides out
            self.stagger_out();
            // Shrink after the slide. Don't rely only on the revealer's signal:
            // shrink after the slide's duration even if the signal never arrives.
            self.sci_revealer.set_reveal_child(false);
            let delay = u64::from(self.sci_revealer.transition_duration()) + 60;
            let weak = Rc::downgrade(self);
            glib::timeout_add_local_once(Duration::from_millis(delay), move || {
                if let Some(win) = weak.upgrade() {
                    win.shrink_after_slide();
                }
            });
        }
        self.update_chip();
        self.refresh(None, false);
    }

    fn stagger_in(self: &Rc<Self>) {
        if !motion::enabled() {
            return;
        }
        for (i, anim) in self.sci_anims.iter().enumerate() {
            anim.scale().set(0.85);
            anim.opacity().set(0.0);
            let anim = anim.clone();
            let weak = Rc::downgrade(self);
            let delay = 60 + (i % 4) * 18 + (i / 4) * 24;
            glib::timeout_add_local_once(Duration::from_millis(delay as u64), move || {
                if weak
                    .upgrade()
                    .is_some_and(|win| win.mode.get() == Mode::Scientific)
                {
                    anim.scale().animate_to(1.0, Kind::Default);
                    anim.opacity().animate_to(1.0, Kind::Effects);
                }
            });
        }
    }

    /// The opening's stagger in reverse: the scientific keys shrink and fade,
    /// last key first, while the panel slides out.
    fn stagger_out(self: &Rc<Self>) {
        if !motion::enabled() {
            return;
        }
        let count = self.sci_anims.len();
        for (i, anim) in self.sci_anims.iter().enumerate() {
            let j = count - 1 - i;
            let anim = anim.clone();
            let weak = Rc::downgrade(self);
            let delay = (j % 4) * 12 + (j / 4) * 16;
            glib::timeout_add_local_once(Duration::from_millis(delay as u64), move || {
                if weak
                    .upgrade()
                    .is_some_and(|win| win.mode.get() == Mode::Standard)
                {
                    anim.scale().animate_to(0.85, Kind::Fast);
                    anim.opacity().animate_to(0.0, Kind::Effects);
                }
            });
        }
    }

    fn size_for(&self, mode: Mode) -> (i32, i32) {
        if self.settings.bool("remember_size") {
            self.state.borrow().size_for(mode)
        } else {
            mode.default_size()
        }
    }

    /// While the scientific panel slides out and the window shrinks, keep the
    /// standard keys at the right edge, growing smoothly to the size they will
    /// have in standard mode, instead of stretching into the space the panel
    /// leaves. Released once the window has shrunk.
    fn pin_standard(self: &Rc<Self>) {
        let grid = &self.standard_grid;
        let start_w = grid.width();
        if start_w <= 1 || !self.window.is_mapped() {
            return;
        }
        let shrink_by = self.window.default_size().0 - self.size_for(Mode::Standard).0;
        let min_w = grid.measure(gtk::Orientation::Horizontal, -1).0;
        let end_w = (start_w + self.sci_revealer.width() - shrink_by).max(min_w);
        self.pinned.set(true);
        let pinned_window_w = self.window.width();
        grid.set_hexpand(false);
        grid.set_halign(gtk::Align::End);
        grid.set_size_request(start_w, -1);
        // The scientific keys keep their size too: the sliding panel would
        // otherwise stretch them across the space it is given.
        if let Some(sci) = self.sci_revealer.child() {
            sci.set_size_request(sci.width(), -1);
            sci.set_halign(gtk::Align::End);
        }
        let duration = f64::from(self.sci_revealer.transition_duration()) / 1000.0;

        let start = Cell::new(None::<f64>);
        let weak = Rc::downgrade(self);
        grid.add_tick_callback(move |grid, clock| {
            if !weak.upgrade().is_some_and(|win| win.pinned.get()) {
                return glib::ControlFlow::Break;
            }
            let now = clock.frame_time() as f64 / 1e6;
            let t0 = *start.get().get_or_insert(now);
            start.set(Some(t0));
            let t = if duration <= 0.0 {
                1.0
            } else {
                ((now - t0) / duration).min(1.0)
            };
            let eased = 1.0 - (1.0 - t).powi(3); // ease-out, like the panel's slide
            let width = f64::from(start_w) + f64::from(end_w - start_w) * eased;
            grid.set_size_request(width.round_ties_even() as i32, -1);
            if t < 1.0 {
                glib::ControlFlow::Continue
            } else {
                glib::ControlFlow::Break
            }
        });

        let weak = Rc::downgrade(self);
        self.window.add_tick_callback(move |window, _| {
            let Some(win) = weak.upgrade().filter(|win| win.pinned.get()) else {
                return glib::ControlFlow::Break;
            };
            if f64::from(window.width()) <= f64::from(pinned_window_w) - f64::from(shrink_by) / 2.0
            {
                win.unpin_standard(); // the window has shrunk
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        // Release anyway if the window can't shrink (maximized, or the
        // compositor refused and the window isn't reopened).
        let weak = Rc::downgrade(self);
        let release = Duration::from_millis((duration * 1000.0) as u64 + 2000);
        glib::timeout_add_local_once(release, move || {
            if let Some(win) = weak.upgrade() {
                win.unpin_standard();
            }
        });
    }

    fn unpin_standard(&self) {
        if !self.pinned.replace(false) {
            return;
        }
        let grid = &self.standard_grid;
        grid.set_size_request(-1, -1);
        grid.set_halign(gtk::Align::Fill);
        grid.set_hexpand(true);
        if let Some(sci) = self.sci_revealer.child() {
            sci.set_size_request(SCI_PANEL_WIDTH, -1);
            sci.set_halign(gtk::Align::Fill);
        }
        if self.mode.get() == Mode::Standard {
            self.sci_revealer.set_hexpand(false);
        }
    }

    fn shrink_after_slide(self: &Rc<Self>) {
        if self.mode.get() != Mode::Standard || self.shrunk_for_switch.get() {
            return;
        }
        self.shrunk_for_switch.set(true);
        if !self.pinned.get() {
            // While pinned the empty panel fills the space; released with the pin.
            self.sci_revealer.set_hexpand(false); // expand flags propagate from the keys
        }
        self.shrink_to_mode(Mode::Standard);
    }

    /// Ask for the smaller size. Some compositors (Hyprland, for one) keep a
    /// floating window's size no matter what the app asks; if the window is
    /// still too wide shortly after, it is unmapped and shown again at the new
    /// size, which every compositor accepts.
    ///
    /// The compositor's "tiled" flag is deliberately not consulted: Hyprland
    /// sets it on floating windows too, so it can't tell the two apart.
    fn shrink_to_mode(self: &Rc<Self>, mode: Mode) {
        if self.fills_screen() {
            debug("shrink skipped: window fills the screen");
            return;
        }
        self.apply_size_for(mode, false);
        let (width, height) = self.window.default_size();
        debug(&format!(
            "shrink: asked for {width}x{height}, now {}x{}",
            self.window.width(),
            self.window.height()
        ));
        let weak = Rc::downgrade(self);
        let delay = if compositor::is_hyprland() { 600 } else { 250 };
        glib::timeout_add_local_once(Duration::from_millis(delay), move || {
            let Some(win) = weak.upgrade() else { return };
            if win.mode.get() != mode || !win.window.is_mapped() {
                return;
            }
            debug(&format!(
                "shrink check: wanted {width}, got {}",
                win.window.width()
            ));
            if win.window.width() > width + 24 {
                debug("compositor kept the old size; reopening the window");
                win.remap(width, height);
            }
        });
    }

    fn remap(self: &Rc<Self>, width: i32, height: i32) {
        self.remapping.set(true);
        self.mode_changed_at.set(glib::monotonic_time());
        self.window.set_visible(false);
        self.window.set_default_size(width, height);
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(win) = weak.upgrade() {
                win.window.present();
                win.remapping.set(false);
            }
        });
    }

    fn on_inverse(&self, inverse: bool) {
        self.inverse.set(inverse);
        for variant in &self.sci_variants {
            let (label, action, tip) = if inverse {
                variant.inverse
            } else {
                variant.normal
            };
            variant.key.button.set_label(label);
            variant.key.button.set_tooltip_text(Some(tip));
            *variant.key.action.borrow_mut() = action.to_string();
        }
    }

    fn update_chip(&self) {
        let rad = self.settings.str("angle_unit") == "rad";
        let label = if rad { "RAD" } else { "DEG" };
        self.angle_chip.set_label(label);
        self.angle_chip.set_visible(self.calc.borrow().scientific);
        self.angle_button.set_label(label);
        if self.angle_button.is_active() != rad {
            self.angle_button.set_active(rad);
        }
    }

    // ----------------------------------------------------------- history
    fn rebuild_history(self: &Rc<Self>) {
        while let Some(child) = self.history_list.first_child() {
            self.history_list.remove(&child);
        }
        let grouping = self.settings.bool("digit_grouping");
        let decimal = self.calc.borrow().decimal;
        let items = self.history.items();
        for item in &items {
            let row = gtk::ListBoxRow::new();
            let row_box = gtk::Box::builder()
                .spacing(8)
                .css_classes(["history-row"])
                .build();
            let texts = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .hexpand(true)
                .build();
            texts.append(
                &gtk::Label::builder()
                    .label(format!(
                        "{} =",
                        localize(&pretty(&item.expr, grouping), decimal)
                    ))
                    .xalign(1.0)
                    .ellipsize(pango::EllipsizeMode::Start)
                    .css_classes(["h-expr"])
                    .build(),
            );
            let result = format_number(
                item.value,
                &Format {
                    grouping,
                    ..Format::default()
                },
            );
            texts.append(
                &gtk::Label::builder()
                    .label(localize(&result, decimal))
                    .xalign(1.0)
                    .ellipsize(pango::EllipsizeMode::End)
                    .css_classes(["h-result"])
                    .selectable(false)
                    .build(),
            );
            row_box.append(&texts);
            let copy = icon_button("content_copy", "Copy result");
            copy.set_valign(gtk::Align::Center);
            let value = item.value;
            copy.connect_clicked(on!(self, win, |_| win.copy_value(value)));
            row_box.append(&copy);
            row.set_child(Some(&row_box));
            row.set_tooltip_text(Some("Use this result"));
            self.history_list.append(&row);
        }
        *self.history_values.borrow_mut() = items.iter().map(|item| item.value).collect();
        let has_items = !items.is_empty();
        self.history_stack
            .set_visible_child_name(if has_items { "list" } else { "empty" });
        self.clear_history_btn.set_sensitive(has_items);
        self.history_empty_hint
            .set_label(if self.settings.bool("save_history") {
                "Results appear here after you press ="
            } else {
                "History is kept only until you close the app. You can change this in Preferences."
            });
    }

    /// For development: where a key is drawn, as (x, width) in the window.
    pub fn dev_key_bounds(&self, action: &str) -> Option<(f32, f32)> {
        let key = self
            .key_motion
            .get(action)
            .filter(|key| key.anim.is_mapped())?;
        let bounds = key.anim.compute_bounds(&self.window)?;
        Some((bounds.x(), bounds.width()))
    }

    /// Interface actions for the development screenshot mode.
    pub fn dev_action(&self, action: &str) {
        let toggle = |button: &gtk::ToggleButton| button.set_active(!button.is_active());
        match action {
            "mode" => {
                if self.sci_toggle.is_active() {
                    self.std_toggle.set_active(true);
                } else {
                    self.sci_toggle.set_active(true);
                }
            }
            "inv" => toggle(&self.inv_button),
            "angle" => toggle(&self.angle_button),
            "history" => toggle(&self.history_btn),
            _ => eprintln!("calculator: unknown ui action {action}"),
        }
    }

    fn set_history_open(self: &Rc<Self>, opening: bool) {
        let height = self.window.height().max(300);
        let sheet_h = f64::from(height) * 0.74;
        self.history_sheet.set_size_request(-1, sheet_h as i32);
        self.sheet_open.set(opening);
        if opening {
            self.sheet_anim.set_visible(true);
            self.scrim.set_visible(true);
            self.scrim_anim.set_can_target(true);
            self.sheet_anim.y().set(sheet_h.trunc());
            self.sheet_anim.y().animate_to(0.0, Kind::Default);
            self.scrim_anim.opacity().set(0.0);
            self.scrim_anim.opacity().animate_to(1.0, Kind::Effects);
        } else {
            self.scrim_anim.set_can_target(false);
            let weak = Rc::downgrade(self);
            let hide = move || {
                if let Some(win) = weak.upgrade() {
                    if !win.sheet_open.get() {
                        win.sheet_anim.set_visible(false);
                        win.scrim.set_visible(false);
                    }
                }
            };
            let anim = Anim {
                kind: Some(Kind::Fast),
                done: Some(Box::new(hide)),
                ..Anim::default()
            };
            self.sheet_anim.y().animate(sheet_h.trunc() + 40.0, anim);
            self.scrim_anim.opacity().animate_to(0.0, Kind::Effects);
        }
    }

    // ----------------------------------------------------------- clipboard
    fn copy_result(self: &Rc<Self>) {
        let text = self.calc.borrow().copy_text(self.decimals());
        self.window.clipboard().set_text(&text);
        self.show_snackbar(&format!("Copied {text}"));
    }

    fn copy_value(self: &Rc<Self>, value: f64) {
        let format = Format {
            grouping: false,
            pretty_minus: false,
            ..Format::default()
        };
        let text = localize(&format_number(value, &format), self.calc.borrow().decimal);
        self.window.clipboard().set_text(&text);
        self.show_snackbar(&format!("Copied {text}"));
    }

    fn paste(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.window
            .clipboard()
            .read_text_async(gio::Cancellable::NONE, move |result| {
                let (Some(win), Ok(Some(text))) = (weak.upgrade(), result) else {
                    return;
                };
                let status = win.calc.borrow_mut().paste(&text);
                if status == Status::Ok {
                    win.refresh(None, false);
                }
            });
    }

    pub fn show_snackbar(self: &Rc<Self>, text: &str) {
        self.snack_label.set_label(text);
        self.snack_anim.set_visible(true);
        self.snack_anim.enter_from(60.0, 0.92);
        if let Some(id) = self.snack_timeout.borrow_mut().take() {
            id.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(2400), move || {
            let Some(win) = weak.upgrade() else { return };
            win.snack_timeout.borrow_mut().take();
            let snack = win.snack_anim.clone();
            let done = move || snack.set_visible(false);
            let anim = Anim {
                kind: Some(Kind::Effects),
                done: Some(Box::new(done)),
                ..Anim::default()
            };
            win.snack_anim.opacity().animate(0.0, anim);
            win.snack_anim.y().animate_to(30.0, Kind::Default);
        });
        *self.snack_timeout.borrow_mut() = Some(id);
    }

    fn show_preferences(&self) {
        let callback = self.on_preferences.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
    }

    pub fn set_preferences_handler(&self, handler: impl Fn() + 'static) {
        *self.on_preferences.borrow_mut() = Some(Rc::new(handler));
    }

    /// Forget the remembered sizes and go back to the default for this mode.
    pub fn reset_size(&self) {
        let mut state = self.state.borrow_mut();
        state.reset();
        state.save();
        drop(state);
        self.apply_size_for(self.mode.get(), false);
    }

    // ----------------------------------------------------------- settings
    fn apply_settings(&self) {
        let s = &self.settings;
        for size in ["small", "medium", "large"] {
            self.window.remove_css_class(&format!("text-{size}"));
        }
        self.window
            .add_css_class(&format!("text-{}", s.str("text_size")));
        for size in ["compact", "standard", "large"] {
            self.window.remove_css_class(&format!("keys-{size}"));
        }
        self.window
            .add_css_class(&format!("keys-{}", s.str("button_size")));
        {
            let mut calc = self.calc.borrow_mut();
            calc.degrees = s.str("angle_unit") == "deg";
            calc.decimal = if s.str("decimal_separator") == "comma" {
                ','
            } else {
                '.'
            };
            self.dot_button.set_label(&calc.decimal.to_string());
        }
        self.update_chip();
        self.apply_motion();
    }

    fn apply_motion(&self) {
        let scheme = match self.settings.str("motion").as_str() {
            "system" => (!self.theme.reduced_motion()).then_some(Scheme::Expressive),
            "standard" => Some(Scheme::Standard),
            "expressive" => Some(Scheme::Expressive),
            _ => None,
        };
        motion::set_scheme(scheme);
        if scheme.is_none() {
            self.window.add_css_class("no-motion");
        } else {
            self.window.remove_css_class("no-motion");
        }
        self.sci_revealer
            .set_transition_duration(if scheme.is_some() { 320 } else { 0 });
    }

    fn on_setting(self: &Rc<Self>, key: &str) {
        match key {
            "scientific" => self.set_mode(self.settings.bool("scientific")),
            "decimal_separator" => {
                self.apply_settings();
                self.rebuild_history();
                self.refresh(None, false);
            }
            "digit_grouping" | "save_history" => {
                self.rebuild_history();
                self.refresh(None, false);
            }
            "decimals" | "live_preview" => self.refresh(None, false),
            _ => {
                self.apply_settings();
                self.refresh(None, false);
            }
        }
    }

    // ----------------------------------------------------------- size
    fn apply_size_for(&self, mode: Mode, initial: bool) {
        let (width, height) = self.size_for(mode);
        if initial {
            self.window.set_default_size(width, height);
            return;
        }
        if self.fills_screen() {
            return;
        }
        self.window.set_default_size(width, height);
        if compositor::is_hyprland() && self.window.is_mapped() {
            // Hyprland ignores the app's own request for floating windows.
            debug(&format!("asking Hyprland for {width}x{height}"));
            compositor::hyprland_resize(width, height, Rc::new(debug), |ok| {
                debug(&format!(
                    "Hyprland resize {}",
                    if ok { "done" } else { "failed" }
                ));
            });
        }
    }

    /// True only when the window really is maximized or fullscreen.
    ///
    /// Some compositors (Hyprland) report floating windows as maximized, so
    /// the flag alone can't be trusted: a window that is reported maximized
    /// but covers well under the monitor is treated as a normal window.
    fn fills_screen(&self) -> bool {
        if self.window.is_fullscreen() {
            return true;
        }
        if !self.window.is_maximized() {
            return false;
        }
        let monitor = self
            .window
            .surface()
            .and_then(|surface| WidgetExt::display(&self.window).monitor_at_surface(&surface));
        let Some(monitor) = monitor else { return true };
        let geo = monitor.geometry();
        let (width, height) = (self.window.width(), self.window.height());
        let fills = f64::from(width) >= f64::from(geo.width()) * 0.85
            && f64::from(height) >= f64::from(geo.height()) * 0.85;
        if !fills {
            debug(&format!(
                "reported maximized but {width}x{height} on a {}x{} monitor; treating as floating",
                geo.width(),
                geo.height()
            ));
        }
        fills
    }

    fn store_size(&self) {
        if self.remapping.get() {
            return;
        }
        // Right after a mode switch the size belongs to the transition, and a
        // compositor that refused the resize would otherwise get its wide size
        // saved as the standard size.
        if glib::monotonic_time() - self.mode_changed_at.get() < 1_500_000 {
            return;
        }
        if self.fills_screen() || !self.window.is_mapped() {
            return;
        }
        if self.sci_revealer.reveals_child() != self.sci_revealer.is_child_revealed() {
            return; // mid-transition; the size is not the user's
        }
        // default-size tracks the real size in GTK 4 and uses the same units
        // as set_default_size(), so restoring never drifts.
        let (width, height) = self.window.default_size();
        if width > 0 && height > 0 {
            self.state
                .borrow_mut()
                .remember(self.mode.get(), width, height);
        }
    }

    fn on_resize(self: &Rc<Self>) {
        if let Some(id) = self.resize_save.borrow_mut().take() {
            id.remove();
        }
        let weak = Rc::downgrade(self);
        let id = glib::timeout_add_local_once(Duration::from_millis(400), move || {
            let Some(win) = weak.upgrade() else { return };
            win.resize_save.borrow_mut().take();
            win.store_size();
            if win.settings.bool("remember_size") {
                win.state.borrow().save();
            }
            win.refresh(None, false);
        });
        *self.resize_save.borrow_mut() = Some(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_names_map_to_actions() {
        assert_eq!(keymap("7").as_deref(), Some("digit:7"));
        assert_eq!(keymap("KP_3").as_deref(), Some("digit:3"));
        assert_eq!(keymap("comma").as_deref(), Some("dot:"));
        assert_eq!(keymap("KP_Separator").as_deref(), Some("dot:"));
        assert_eq!(keymap("minus").as_deref(), Some("op:−"));
        assert_eq!(keymap("KP_Enter").as_deref(), Some("eq:"));
        assert_eq!(keymap("asciicircum").as_deref(), Some("op:^"));
        assert_eq!(keymap("a"), None);
        assert_eq!(keymap("KP_Home"), None);
        assert_eq!(sci_letter("s"), Some("func:sin("));
        assert_eq!(sci_letter("E"), Some("exp:"));
        assert_eq!(sci_letter("x"), None);
    }
}
