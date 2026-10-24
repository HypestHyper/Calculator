//! Main calculator window.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::Duration;

use calculator::engine::{format_number, localize, pretty, Format};
use calculator::logic::{Calculator, Status};
use calculator::scale::{choose_scale, scaled_css, MIN_SCALE, STEP};
use gtk::prelude::*;
use gtk::{gdk, gio, glib, pango};

use crate::compositor;
use crate::config::{History, Mode, Settings, Value, WindowState, APP_NAME};
use crate::icons::{icon, icon_button};
use crate::motion::{self, Anim, Animated, KeyMotion, Kind, Scheme};
use crate::placement;
use crate::scale_box::ScaleBox;
use crate::theme::{Theme, STYLE};

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
/// Space between keys.
const KEY_SPACING: i32 = 10;
/// How long the window takes to shrink on Hyprland, in seconds.
const SHRINK_SECONDS: f64 = 0.3;

/// Signal numbers, the same on every Linux architecture.
mod libc_signal {
    pub const SIGHUP: i32 = 1;
    pub const SIGINT: i32 = 2;
    pub const SIGTERM: i32 = 15;
}

fn window_controls_box() -> gtk::Box {
    gtk::Box::builder()
        .spacing(10)
        .valign(gtk::Align::Center)
        .css_classes(["window-controls"])
        .build()
}

/// Minimize, maximize and close buttons where the desktop's
/// gtk-decoration-layout puts them, like GTK's own; kept up to date with it.
fn fill_window_controls(window: &gtk::ApplicationWindow, start: &gtk::Box, end: &gtk::Box) {
    let fill = {
        let (window, start, end) = (window.downgrade(), start.downgrade(), end.downgrade());
        move || {
            let (Some(window), Some(start), Some(end)) =
                (window.upgrade(), start.upgrade(), end.upgrade())
            else {
                return;
            };
            let layout = gtk::Settings::default()
                .and_then(|s| s.gtk_decoration_layout())
                .map(|l| l.to_string())
                .unwrap_or_else(|| ":close".to_string());
            let (left, right) = layout.split_once(':').unwrap_or(("", &layout));
            for (side, names) in [(&start, left), (&end, right)] {
                while let Some(child) = side.first_child() {
                    side.remove(&child);
                }
                for name in names.split(',').map(str::trim) {
                    if let Some(button) = window_control(&window, name) {
                        side.append(&button);
                    }
                }
                side.set_visible(side.first_child().is_some());
            }
        }
    };
    fill();
    if let Some(settings) = gtk::Settings::default() {
        let fill = fill.clone();
        settings.connect_gtk_decoration_layout_notify(move |_| fill());
    }
}

fn window_control(window: &gtk::ApplicationWindow, name: &str) -> Option<gtk::Button> {
    let (icon_name, tooltip) = match name {
        "minimize" => ("minimize", "Minimize"),
        "maximize" if window.is_resizable() => ("crop_square", "Maximize"),
        "close" => ("close", "Close"),
        _ => return None, // "icon", "menu" and unknown names: GTK shows none here either
    };
    let button = gtk::Button::builder()
        .child(&icon(icon_name, 16))
        .tooltip_text(tooltip)
        .focus_on_click(false)
        .css_classes(["window-control", name])
        .build();
    let weak = window.downgrade();
    match name {
        "minimize" => button.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.minimize();
            }
        }),
        "maximize" => {
            // The restore icon while maximized, as GTK shows.
            let update = {
                let button = button.downgrade();
                move |window: &gtk::ApplicationWindow| {
                    if let Some(button) = button.upgrade() {
                        let (name, tooltip) = if window.is_maximized() {
                            ("filter_none", "Restore")
                        } else {
                            ("crop_square", "Maximize")
                        };
                        button.set_child(Some(&icon(name, 16)));
                        button.set_tooltip_text(Some(tooltip));
                    }
                }
            };
            update(window);
            window.connect_maximized_notify(update);
            button.connect_clicked(move |_| {
                if let Some(window) = weak.upgrade() {
                    if window.is_maximized() {
                        window.unmaximize();
                    } else {
                        window.maximize();
                    }
                }
            })
        }
        _ => button.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.close();
            }
        }),
    };
    Some(button)
}

/// A size in px at the given scale.
fn scaled(px: i32, scale: f64) -> i32 {
    (f64::from(px) * scale).round() as i32
}

const TEXT_SIZES: [(&str, f64); 3] = [("small", 46.0), ("medium", 58.0), ("large", 70.0)];

/// Content minimum = fixed + scale * per_scale, per axis, and the minimum
/// at full scale once known.
#[derive(Clone, Copy, Default)]
struct SizeModel {
    fixed: [f64; 2],
    per_scale: [f64; 2],
    full: [Option<f64>; 2],
}

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
    /// Hyprland was told where to open the window (placement.rs).
    placed_by_rule: Cell<bool>,
    /// The history rows need rebuilding before the sheet is shown.
    history_stale: Cell<bool>,
    standard_grid: gtk::Grid,
    sci_grid: gtk::Grid,
    /// The standard keys are pinned while the scientific panel closes.
    pinned: Cell<bool>,

    // Contents scale down in smaller windows; see update_scale.
    header_box: ScaleBox,
    body_box: ScaleBox,
    scale_provider: gtk::CssProvider,
    scale: Cell<f64>,
    scale_pending: Cell<bool>,
    /// How the contents' minimum size follows the scale, per mode.
    size_models: Cell<[SizeModel; 2]>,
    grow_tried: Cell<Option<[i32; 2]>>,
    last_size: Cell<(i32, i32)>,
    shrunk_at: Cell<i64>,
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
        .row_spacing(KEY_SPACING)
        .column_spacing(KEY_SPACING)
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
        // Its own minimize, maximize and close buttons, drawn with the icon
        // font: GTK's look their icons up in the icon theme, and the first
        // lookup waits for the whole theme to load (a third of a second with
        // Papirus), which would hold up the window's first frame.
        header.set_show_title_buttons(false);
        let controls_start = window_controls_box();
        let controls_end = window_controls_box();
        header.pack_start(&controls_start);
        header.pack_end(&controls_end);
        fill_window_controls(&window, &controls_start, &controls_end);
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
        let header_box = ScaleBox::new(&header);
        header_box.add_css_class("scale-box");
        window.set_titlebar(Some(&header_box));

        // Body --------------------------------------------------------------
        let overlay = gtk::Overlay::new();
        let body_box = ScaleBox::new(&overlay);
        window.set_child(Some(&body_box));
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
            // Keys go to the window, so the display never needs focus; with
            // it, typing would draw a focus outline around it.
            .focusable(false)
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
        let sci_grid = build_sci_grid(&mut parts);
        sci_revealer.set_child(Some(&sci_grid));
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
            placed_by_rule: Cell::new(false),
            history_stale: Cell::new(false),
            standard_grid,
            sci_grid,
            pinned: Cell::new(false),
            header_box,
            body_box,
            scale_provider: gtk::CssProvider::new(),
            scale: Cell::new(1.0),
            scale_pending: Cell::new(false),
            size_models: Cell::default(),
            grow_tried: Cell::new(None),
            last_size: Cell::new((0, 0)),
            shrunk_at: Cell::new(0),
        });
        gtk::style_context_add_provider_for_display(
            &WidgetExt::display(&win.window),
            &win.scale_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
        );
        debug(&format!(
            "starting in {} mode at {:?}",
            mode.key(),
            win.size_for(mode)
        ));
        win.apply_size_for(mode, true);
        win.prepare_position();
        win.connect_signals();
        win.rebuild_history();
        win.apply_settings();
        win.refresh(None, false);
        win
    }

    fn connect_signals(self: &Rc<Self>) {
        for scale_box in [&self.header_box, &self.body_box] {
            let weak = Rc::downgrade(self);
            scale_box.connect_allocate(move || {
                if let Some(win) = weak.upgrade() {
                    win.queue_scale();
                }
            });
        }
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
                win.save_size();
            }
            glib::Propagation::Proceed
        });
        // Logging out or shutting down stops the app without closing the
        // window: save the size first.
        for signal in [
            libc_signal::SIGTERM,
            libc_signal::SIGHUP,
            libc_signal::SIGINT,
        ] {
            let weak = Rc::downgrade(self);
            glib::unix_signal_add_local(signal, move || {
                if let Some(win) = weak.upgrade() {
                    win.save_size();
                    if let Some(app) = win.window.application() {
                        app.quit();
                    }
                }
                glib::ControlFlow::Break
            });
        }
        self.window
            .connect_default_width_notify(on!(self, win, |_| win.on_resize()));
        self.window
            .connect_default_height_notify(on!(self, win, |_| win.on_resize()));
        // Back where it was last time, where the desktop allows it.
        let weak = Rc::downgrade(self);
        let placed = Cell::new(false);
        self.window.connect_map(move |window| {
            let Some(win) = weak.upgrade() else { return };
            if placed.replace(true) || !win.settings.bool("remember_size") {
                return;
            }
            let Some(method) = placement::method(window) else {
                return;
            };
            let saved = {
                let state = win.state.borrow();
                state
                    .center
                    .filter(|_| state.placed_by.as_deref() == Some(method.key()))
            };
            if win.placed_by_rule.get() {
                // Opened in place; the rule's work is done.
                glib::timeout_add_local_once(Duration::from_millis(500), || {
                    placement::release_hyprland(Rc::new(debug));
                });
            } else if let Some(center) = saved {
                let window = window.clone().upcast::<gtk::Window>();
                // Once the window manager has placed it, or the move is undone.
                glib::timeout_add_local_once(Duration::from_millis(50), move || {
                    placement::restore(&window, method, center, Rc::new(debug));
                });
            }
        });
        // The surface's size changes even where the default size doesn't
        // follow it (see current_size).
        let weak = Rc::downgrade(self);
        self.window.connect_realize(move |window| {
            let Some(surface) = window.surface() else {
                return;
            };
            for property in ["width", "height"] {
                let weak = weak.clone();
                surface.connect_notify_local(Some(property), move |_, _| {
                    if let Some(win) = weak.upgrade() {
                        win.on_resize();
                    }
                });
            }
        });
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
            .map_or(58.0, |(_, px)| *px)
            * self.scale.get();
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
        // Rescale once the switch is over.
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(1600), move || {
            if let Some(win) = weak.upgrade() {
                win.queue_scale();
            }
        });
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
            // A hidden panel keeps the styles it had when hidden; restyle it
            // for the current scale.
            self.sci_grid.add_css_class("restyle");
            self.sci_grid.remove_css_class("restyle");
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
        let shrink_by = self.current_size().0 - self.size_for(Mode::Standard).0;
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
            if window.width() <= pinned_window_w - shrink_by + 2 {
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

    fn unpin_standard(self: &Rc<Self>) {
        if !self.pinned.replace(false) {
            return;
        }
        let grid = &self.standard_grid;
        grid.set_size_request(-1, -1);
        grid.set_halign(gtk::Align::Fill);
        grid.set_hexpand(true);
        if let Some(sci) = self.sci_revealer.child() {
            sci.set_size_request(scaled(SCI_PANEL_WIDTH, self.scale.get()), -1);
            sci.set_halign(gtk::Align::Fill);
        }
        if self.mode.get() == Mode::Standard {
            self.sci_revealer.set_hexpand(false);
        }
        self.queue_scale();
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
        if compositor::is_hyprland() && self.window.is_mapped() && motion::enabled() {
            // Step by step, so the window and its contents shrink together.
            let (width, height) = self.size_for(mode);
            debug(&format!(
                "shrink: animating to {width}x{height} through Hyprland"
            ));
            let weak = Rc::downgrade(self);
            compositor::hyprland_animate_resize(
                width,
                height,
                SHRINK_SECONDS,
                Rc::new(debug),
                move |ok| {
                    debug(&format!(
                        "Hyprland shrink {}",
                        if ok { "done" } else { "failed" }
                    ));
                    if let Some(win) = weak.upgrade() {
                        win.window.set_default_size(width, height);
                        win.check_shrunk(mode, width, height, 600);
                    }
                },
            );
            return;
        }
        self.apply_size_for(mode, false);
        let (width, height) = self.window.default_size();
        debug(&format!(
            "shrink: asked for {width}x{height}, now {}x{}",
            self.window.width(),
            self.window.height()
        ));
        self.check_shrunk(
            mode,
            width,
            height,
            if compositor::is_hyprland() { 600 } else { 250 },
        );
    }

    /// Reopen the window at the new size if the compositor kept the old one.
    fn check_shrunk(self: &Rc<Self>, mode: Mode, width: i32, height: i32, delay: u64) {
        let weak = Rc::downgrade(self);
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
    /// Rebuild the history sheet's rows: right away while the sheet is open,
    /// else once the app is idle (or when the sheet opens, if sooner). A full
    /// history takes longer to build than the rest of the window, so start-up
    /// doesn't wait for it.
    fn rebuild_history(self: &Rc<Self>) {
        if self.sheet_open.get() {
            self.fill_history();
            return;
        }
        if self.history_stale.replace(true) {
            return; // already scheduled
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_full(glib::Priority::LOW, move || {
            if let Some(win) = weak.upgrade().filter(|win| win.history_stale.get()) {
                win.fill_history();
            }
            glib::ControlFlow::Break
        });
    }

    fn fill_history(self: &Rc<Self>) {
        self.history_stale.set(false);
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
            _ if action.starts_with("size:") => {
                // size:WxH, as if the user resized the window
                let size = action["size:".len()..].split_once('x');
                if let Some((Ok(w), Ok(h))) = size.map(|(w, h)| (w.parse(), h.parse())) {
                    // Reopened: without a window manager, a mapped window keeps its size.
                    self.window.set_visible(false);
                    self.window.set_default_size(w, h);
                    self.window.present();
                }
            }
            _ => eprintln!("calculator: unknown ui action {action}"),
        }
    }

    fn set_history_open(self: &Rc<Self>, opening: bool) {
        let height = self.window.height().max(300);
        let sheet_h = f64::from(height) * 0.74;
        self.history_sheet.set_size_request(-1, sheet_h as i32);
        self.sheet_open.set(opening);
        if opening && self.history_stale.get() {
            self.fill_history();
        }
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
                if key == "text_size" || key == "button_size" {
                    self.size_models.take(); // the sizes at full scale changed
                }
                self.apply_settings();
                self.refresh(None, false);
                self.queue_scale();
            }
        }
    }

    // ----------------------------------------------------------- scale
    fn queue_scale(self: &Rc<Self>) {
        if self.scale_pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            if let Some(win) = weak.upgrade() {
                win.update_scale();
            }
        });
    }

    /// The smallest size the header bar and body need, together.
    fn content_min(&self) -> [i32; 2] {
        let (header, body) = (self.header_box.child(), self.body_box.child());
        let h = gtk::Orientation::Horizontal;
        let v = gtk::Orientation::Vertical;
        [
            header.measure(h, -1).0.max(body.measure(h, -1).0),
            header.measure(v, -1).0 + body.measure(v, -1).0,
        ]
    }

    fn size_model(&self) -> SizeModel {
        self.size_models.get()[self.mode.get() as usize]
    }

    fn set_size_model(&self, model: SizeModel) {
        let mut models = self.size_models.get();
        models[self.mode.get() as usize] = model;
        self.size_models.set(models);
    }

    /// Fit content minimum = fixed + scale * per_scale on each axis, from the
    /// size at full scale and the size now.
    fn learn_sizes(&self) -> [i32; 2] {
        let need = self.content_min();
        let scale = self.scale.get();
        let model = self.size_model();
        let (mut fixed, mut per_scale, mut full) = (model.fixed, model.per_scale, model.full);
        for i in 0..2 {
            let n = f64::from(need[i]);
            if scale >= 1.0 {
                full[i] = Some(n);
            } else if let Some(f) = full[i] {
                fixed[i] = ((n - scale * f) / (1.0 - scale)).max(0.0);
            }
            per_scale[i] = (n - fixed[i]) / scale;
            full[i] = Some(fixed[i] + per_scale[i]);
        }
        self.set_size_model(SizeModel {
            fixed,
            per_scale,
            full,
        });
        need
    }

    /// Pick the largest scale whose sizes fit the window: full size in a
    /// normal window, down to MIN_SCALE in a small one.
    fn update_scale(self: &Rc<Self>) {
        self.scale_pending.set(false);
        let switching = glib::monotonic_time() - self.mode_changed_at.get() < 1_500_000;
        if switching || self.pinned.get() || !self.window.is_mapped() {
            return; // sizes during a mode switch belong to the transition
        }
        let available = [
            self.body_box.width(),
            self.header_box.height() + self.body_box.height(),
        ];
        if available.iter().any(|&a| a <= 1) {
            return;
        }
        let need = self.learn_sizes();
        let model = self.size_model();
        let scale = self.scale.get();
        let short = need.iter().zip(available).any(|(&n, a)| n > a);
        if short && scale >= 1.0 && self.try_grow(need, available) {
            return;
        }
        let avail = available.map(f64::from);
        let target = if short {
            // Doesn't fit: as far down as the sizes say, at least one step.
            let target = choose_scale(avail, model.fixed, model.per_scale);
            target
                .min(((scale - STEP) * 100.0).round() / 100.0)
                .max(MIN_SCALE)
        } else if model
            .full
            .iter()
            .zip(avail)
            .all(|(f, a)| f.is_some_and(|f| f <= a))
        {
            1.0 // full size fits
        } else {
            // Grow only with a little room to spare, so it never flips back and forth.
            let room = [avail[0] - 4.0, avail[1] - 4.0];
            scale.max(choose_scale(room, model.fixed, model.per_scale))
        };
        if target != scale {
            self.apply_scale(target);
        }
    }

    /// At full scale, make room by growing the window when it is a few
    /// pixels short of its contents, as GTK does for a window smaller than
    /// them: unless the user has just made it smaller, or growing was already
    /// tried for this size and didn't work (a tiled or maximized window).
    /// Further short, the window is small on purpose and the contents scale
    /// down instead. True if it grows.
    fn try_grow(self: &Rc<Self>, need: [i32; 2], available: [i32; 2]) -> bool {
        if need.iter().zip(available).any(|(&n, a)| n - a > 16) {
            return false;
        }
        if glib::monotonic_time() - self.shrunk_at.get() < 1_000_000 || self.fills_screen() {
            return false;
        }
        if self.grow_tried.replace(Some(need)) == Some(need) {
            return false;
        }
        let (width, height) = self.current_size();
        let width = width + (need[0] - available[0]).max(0);
        let height = height + (need[1] - available[1]).max(0);
        debug(&format!("growing to {width}x{height} to fit the contents"));
        self.window.set_default_size(width, height);
        if compositor::is_hyprland() {
            compositor::hyprland_resize(width, height, Rc::new(debug), |_| {});
        }
        // Scale down instead if it didn't grow.
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(500), move || {
            if let Some(win) = weak.upgrade() {
                win.queue_scale();
            }
        });
        true
    }

    fn apply_scale(&self, scale: f64) {
        debug(&format!("scale {} -> {scale}", self.scale.get()));
        self.scale.set(scale);
        self.scale_provider
            .load_from_data(&scaled_css(STYLE, scale));
        let spacing = scaled(KEY_SPACING, scale);
        for grid in [&self.standard_grid, &self.sci_grid] {
            grid.set_row_spacing(spacing as u32);
            grid.set_column_spacing(spacing as u32);
        }
        if !self.pinned.get() {
            self.sci_grid
                .set_size_request(scaled(SCI_PANEL_WIDTH, scale), -1);
        }
        // The boxes report the contents' minimum at MIN_SCALE. The new styles
        // take effect at the next layout, so a factor that raises the reported
        // minimum waits until then: with the old sizes it would grow the window.
        let shrink = MIN_SCALE / scale;
        let set_shrink = |shrink: f64| {
            for scale_box in [&self.header_box, &self.body_box] {
                scale_box.set_shrink(shrink);
            }
        };
        if shrink < self.header_box.shrink() {
            set_shrink(shrink);
        } else {
            let boxes = [self.header_box.clone(), self.body_box.clone()];
            let frames = Cell::new(0);
            self.window.add_tick_callback(move |_, _| {
                frames.set(frames.get() + 1);
                if frames.get() < 2 {
                    return glib::ControlFlow::Continue;
                }
                for scale_box in &boxes {
                    scale_box.set_shrink(shrink);
                }
                glib::ControlFlow::Break
            });
        }
        self.fit_text(&self.expr_label.label());
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
        let (width, height) = self.current_size();
        if width > 0 && height > 0 {
            self.state
                .borrow_mut()
                .remember(self.mode.get(), width, height);
        }
    }

    /// Where the desktop can be told before the window opens (Hyprland), tell
    /// it where the window was last time.
    fn prepare_position(&self) {
        if !self.settings.bool("remember_size")
            || placement::method(&self.window) != Some(placement::Method::Hyprland)
        {
            return;
        }
        let saved = {
            let state = self.state.borrow();
            state
                .center
                .filter(|_| state.placed_by.as_deref() == Some(placement::Method::Hyprland.key()))
        };
        if let Some(center) = saved {
            let placed = placement::prepare_hyprland(center, &(Rc::new(debug) as _));
            self.placed_by_rule.set(placed);
        }
    }

    /// Remember the window's size now and write it to disk.
    fn save_size(&self) {
        self.store_size();
        if self.settings.bool("remember_size") {
            if let Some(method) =
                placement::method(&self.window).filter(|_| self.window.is_mapped())
            {
                let window = self.window.clone().upcast::<gtk::Window>();
                if let Some(center) = placement::center(&window, method, &(Rc::new(debug) as _)) {
                    let mut state = self.state.borrow_mut();
                    state.center = Some(center);
                    state.placed_by = Some(method.key().to_string());
                }
            }
            let state = self.state.borrow();
            debug(&format!(
                "saving sizes: standard {:?}, scientific {:?}, centre {:?}",
                state.size_for(Mode::Standard),
                state.size_for(Mode::Scientific),
                state.center
            ));
            state.save();
        }
    }

    /// The window's size, in the units of set_default_size().
    ///
    /// GTK keeps the default size up to date with the real size, so
    /// restoring never drifts, but not while the compositor calls the window
    /// tiled or maximized. Hyprland calls floating windows tiled, so there the
    /// default size stays at what the app last asked for; the real size is
    /// used instead.
    fn current_size(&self) -> (i32, i32) {
        let untracked = gdk::ToplevelState::TILED
            | gdk::ToplevelState::TOP_TILED
            | gdk::ToplevelState::RIGHT_TILED
            | gdk::ToplevelState::BOTTOM_TILED
            | gdk::ToplevelState::LEFT_TILED
            | gdk::ToplevelState::MAXIMIZED;
        let tracked = self
            .window
            .surface()
            .and_downcast::<gdk::Toplevel>()
            .is_none_or(|toplevel| !toplevel.state().intersects(untracked));
        let (width, height) = (self.window.width(), self.window.height());
        if tracked || width <= 1 || height <= 1 {
            self.window.default_size()
        } else {
            (width, height)
        }
    }

    fn on_resize(self: &Rc<Self>) {
        let size = self.current_size();
        let last = self.last_size.replace(size);
        if size.0 < last.0 || size.1 < last.1 {
            self.shrunk_at.set(glib::monotonic_time()); // made smaller: scale, don't grow back
        }
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
