//! Preferences window. Port of `calculator/preferences.py`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::rc::{Rc, Weak};

use calculator::colors::{self, PRESETS, STYLES};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib, graphene, gsk};

use crate::config::{config_dir, History, Settings, Value, APP_NAME, VERSION};
use crate::icons::icon;
use crate::theme::{palette_file, Theme};
use crate::window::Win;
use crate::window_rules;

const WEBSITE: &str = "https://github.com/HypestHyper/Calculator";

const SHORTCUTS: [(&str, &str); 8] = [
    ("Enter", "Calculate"),
    ("Esc", "Clear, or close history"),
    ("Backspace", "Delete last"),
    ("Ctrl+C / Ctrl+V", "Copy result / paste"),
    ("Ctrl+H", "History"),
    ("Ctrl+S", "Scientific mode"),
    ("Ctrl+,", "Preferences"),
    ("s c t l g r p e", "sin cos tan ln log √ π e (scientific)"),
];

/// Called after a drop-down changes its setting.
type OnChange = Rc<dyn Fn(&Value)>;
/// A preset's swatch colours: surface, then three accents.
type SwatchColors = Rc<dyn Fn(&str) -> Option<[String; 4]>>;

const SOURCES: [(&str, &str); 4] = [
    ("desktop", "Desktop accent"),
    ("preset", "Preset palette"),
    ("custom", "Custom colour"),
    ("palette", "Palette file"),
];

pub struct Prefs {
    pub window: gtk::Window,
    settings: Rc<Settings>,
    theme: Rc<Theme>,
    main: Weak<Win>,
    source_dd: RefCell<Option<gtk::DropDown>>,
    preset_dd: RefCell<Option<gtk::DropDown>>,
    custom_row: RefCell<Option<gtk::Widget>>,
    style_row: RefCell<Option<gtk::Widget>>,
    status_subtitle: RefCell<Option<gtk::Label>>,
    palette_create: RefCell<Option<gtk::Button>>,
    palette_open: RefCell<Option<gtk::Button>>,
    rule_label: gtk::Label,
    rule_hint: gtk::Label,
    rule_key: Cell<&'static str>,
    swatch_cache: RefCell<HashMap<(&'static str, bool), [String; 4]>>,
}

impl Prefs {
    pub fn new(
        settings: Rc<Settings>,
        history: Rc<History>,
        theme: Rc<Theme>,
        main: &Rc<Win>,
    ) -> Rc<Self> {
        let window = gtk::Window::builder()
            .title("Preferences")
            .transient_for(&main.window)
            .modal(false)
            .default_width(560)
            .default_height(720)
            .build();
        window.add_css_class("calc-prefs");
        if main.window.has_css_class("no-motion") {
            window.add_css_class("no-motion");
        }
        let header = gtk::HeaderBar::new();
        header.set_title_widget(Some(
            &gtk::Label::builder()
                .label("Preferences")
                .css_classes(["title-text"])
                .build(),
        ));
        window.set_titlebar(Some(&header));

        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["pref-page"])
            .build();
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .child(&page)
            .build();
        window.set_child(Some(&scroll));

        let prefs = Rc::new(Prefs {
            window,
            settings,
            theme: theme.clone(),
            main: Rc::downgrade(main),
            source_dd: RefCell::default(),
            preset_dd: RefCell::default(),
            custom_row: RefCell::default(),
            style_row: RefCell::default(),
            status_subtitle: RefCell::default(),
            palette_create: RefCell::default(),
            palette_open: RefCell::default(),
            rule_label: gtk::Label::builder()
                .xalign(0.0)
                .selectable(true)
                .wrap(false)
                .css_classes(["code"])
                .build(),
            rule_hint: gtk::Label::builder()
                .xalign(0.0)
                .wrap(true)
                .css_classes(["pref-subtitle"])
                .build(),
            rule_key: Cell::new(window_rules::ORDER[0]),
            swatch_cache: RefCell::default(),
        });
        prefs.build_appearance(&page);
        prefs.build_palette_file(&page);
        prefs.build_display(&page);
        prefs.build_behaviour(&page, history);
        prefs.build_window(&page);
        prefs.build_shortcuts(&page);
        prefs.build_about(&page);

        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(&prefs);
        keys.connect_key_pressed(move |_, keyval, _, mods| {
            let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
            if keyval == gdk::Key::Escape
                || (ctrl && (keyval == gdk::Key::w || keyval == gdk::Key::comma))
            {
                if let Some(prefs) = weak.upgrade() {
                    prefs.window.close();
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        prefs.window.add_controller(keys);

        let weak = Rc::downgrade(&prefs);
        theme.connect(move || {
            if let Some(prefs) = weak.upgrade() {
                prefs.on_theme();
            }
        });
        prefs.on_theme();
        prefs
    }

    // ------------------------------------------------------------ helpers
    fn section(&self, page: &gtk::Box, title: &str) -> gtk::Box {
        page.append(
            &gtk::Label::builder()
                .label(title)
                .xalign(0.0)
                .css_classes(["section-title"])
                .build(),
        );
        let card = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .css_classes(["pref-card"])
            .build();
        page.append(&card);
        card
    }

    /// A row with an icon, title, optional subtitle and a control. Returns the
    /// row (the outer box for a vertical row) and its subtitle label.
    fn row(
        &self,
        card: &gtk::Box,
        title: &str,
        subtitle: Option<&str>,
        widget: Option<&gtk::Widget>,
        icon_name: Option<&str>,
        vertical: bool,
    ) -> (gtk::Box, Option<gtk::Label>) {
        let row = gtk::Box::builder()
            .spacing(14)
            .css_classes(["pref-row"])
            .build();
        let ic: gtk::Widget = match icon_name {
            Some(name) => {
                let ic = icon(name, 20);
                ic.set_valign(gtk::Align::Center);
                ic
            }
            None => gtk::Box::new(gtk::Orientation::Horizontal, 0).upcast(), // keeps titles aligned
        };
        ic.set_size_request(22, -1);
        row.append(&ic);
        let texts = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .build();
        texts.append(
            &gtk::Label::builder()
                .label(title)
                .xalign(0.0)
                .wrap(true)
                .css_classes(["pref-title"])
                .build(),
        );
        let sub = subtitle.map(|text| {
            let label = gtk::Label::builder()
                .label(text)
                .xalign(0.0)
                .wrap(true)
                .css_classes(["pref-subtitle"])
                .build();
            texts.append(&label);
            label
        });
        if vertical {
            let outer = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(10)
                .css_classes(["pref-row"])
                .build();
            row.remove_css_class("pref-row");
            row.append(&texts);
            outer.append(&row);
            if let Some(widget) = widget {
                outer.append(widget);
            }
            card.append(&outer);
            return (outer, sub);
        }
        row.append(&texts);
        if let Some(widget) = widget {
            widget.set_valign(gtk::Align::Center);
            row.append(widget);
        }
        card.append(&row);
        (row, sub)
    }

    fn segmented(&self, options: &[(&str, &str)], key: &'static str) -> gtk::Widget {
        let group = gtk::Box::builder()
            .css_classes(["connected-group", "small"])
            .build();
        let mut first: Option<gtk::ToggleButton> = None;
        let current = self.settings.str(key);
        for &(value, label) in options {
            let button = gtk::ToggleButton::with_label(label);
            match &first {
                None => first = Some(button.clone()),
                Some(first) => button.set_group(Some(first)),
            }
            button.set_active(current == value);
            let settings = self.settings.clone();
            let value = value.to_string();
            button.connect_toggled(move |button| {
                if button.is_active() {
                    let _ = settings.set(key, Value::Str(value.clone()));
                }
            });
            group.append(&button);
        }
        group.upcast()
    }

    fn dropdown(
        &self,
        options: Vec<(Value, String)>,
        key: &'static str,
        on_change: Option<OnChange>,
    ) -> gtk::DropDown {
        let labels: Vec<&str> = options.iter().map(|(_, label)| label.as_str()).collect();
        let dd = gtk::DropDown::from_strings(&labels);
        let current = self.settings.get(key);
        let index = options
            .iter()
            .position(|(value, _)| *value == current)
            .unwrap_or(0);
        dd.set_selected(index as u32);
        let settings = self.settings.clone();
        let values: Vec<Value> = options.into_iter().map(|(value, _)| value).collect();
        dd.connect_selected_notify(move |dd| {
            let Some(value) = values.get(dd.selected() as usize) else {
                return;
            };
            let _ = settings.set(key, value.clone());
            if let Some(on_change) = &on_change {
                on_change(value);
            }
        });
        dd
    }

    fn switch(&self, key: &'static str) -> gtk::Widget {
        let sw = gtk::Switch::builder()
            .active(self.settings.bool(key))
            .build();
        let settings = self.settings.clone();
        sw.connect_active_notify(move |sw| {
            let _ = settings.set(key, Value::Bool(sw.is_active()));
        });
        sw.upcast()
    }

    fn strs(options: &[(&str, &str)]) -> Vec<(Value, String)> {
        options
            .iter()
            .map(|(v, l)| (Value::Str(v.to_string()), l.to_string()))
            .collect()
    }

    // ------------------------------------------------------------ sections
    fn build_appearance(self: &Rc<Self>, page: &gtk::Box) {
        let card = self.section(page, "Colours");
        let theme = self.segmented(
            &[("system", "System"), ("light", "Light"), ("dark", "Dark")],
            "theme",
        );
        self.row(&card, "Theme", None, Some(&theme), Some("contrast"), false);

        let weak = Rc::downgrade(self);
        let on_source: OnChange = Rc::new(move |value| {
            if let (Some(prefs), Value::Str(source)) = (weak.upgrade(), value) {
                prefs.update_source_rows(source);
            }
        });
        let source_dd = self.dropdown(Self::strs(&SOURCES), "color_source", Some(on_source));
        self.row(
            &card,
            "Colour source",
            Some("Where the colours come from"),
            Some(source_dd.upcast_ref()),
            Some("palette"),
            false,
        );
        *self.source_dd.borrow_mut() = Some(source_dd);

        let preset_dd = self.preset_dropdown();
        self.row(
            &card,
            "Preset",
            Some("Material palettes and popular themes"),
            Some(preset_dd.upcast_ref()),
            None,
            false,
        );
        *self.preset_dd.borrow_mut() = Some(preset_dd);

        let color_button = gtk::ColorDialogButton::new(Some(
            gtk::ColorDialog::builder().with_alpha(false).build(),
        ));
        if let Ok(rgba) = gdk::RGBA::parse(self.settings.str("custom_accent")) {
            color_button.set_rgba(&rgba);
        }
        let settings = self.settings.clone();
        color_button.connect_rgba_notify(move |button| {
            let rgba = button.rgba();
            let rgb = (
                f64::from(rgba.red()) * 255.0,
                f64::from(rgba.green()) * 255.0,
                f64::from(rgba.blue()) * 255.0,
            );
            let _ = settings.set("custom_accent", Value::Str(colors::to_hex(rgb)));
        });
        let (custom_row, _) = self.row(
            &card,
            "Custom colour",
            Some("Seed for the generated palette"),
            Some(color_button.upcast_ref()),
            None,
            false,
        );
        *self.custom_row.borrow_mut() = Some(custom_row.upcast());

        let styles = STYLES
            .iter()
            .map(|(k, n)| (Value::Str(k.to_string()), n.to_string()))
            .collect();
        let style_dd = self.dropdown(styles, "palette_style", None);
        let (style_row, _) = self.row(
            &card,
            "Palette style",
            Some("How colourful the generated palette is"),
            Some(style_dd.upcast_ref()),
            Some("tune"),
            false,
        );
        *self.style_row.borrow_mut() = Some(style_row.upcast());

        let contrast = self.dropdown(
            Self::strs(&[
                ("system", "Follow system"),
                ("standard", "Standard"),
                ("medium", "Medium"),
                ("high", "High"),
            ]),
            "contrast",
            None,
        );
        self.row(
            &card,
            "Contrast",
            None,
            Some(contrast.upcast_ref()),
            None,
            false,
        );
        let icon_switch = self.switch("themed_icon");
        self.row(
            &card,
            "Match app icon",
            Some("Colour the launcher icon with your palette"),
            Some(&icon_switch),
            Some("calculate"),
            false,
        );
        let (_, status) = self.row(&card, "Active colours", Some(""), None, Some("info"), false);
        *self.status_subtitle.borrow_mut() = status;
        self.update_source_rows(&self.settings.str("color_source"));
    }

    fn build_palette_file(self: &Rc<Self>, page: &gtk::Box) {
        let card = self.section(page, "Palette file");
        let subtitle = format!(
            "{}\nSet a seed, a style or any colour role by hand. Changes apply as soon as the file is saved.",
            home_relative(&palette_file())
        );
        let (row, _) = self.row(&card, "palette.toml", Some(&subtitle), None, None, true);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let create = gtk::Button::builder()
            .label("Create from current colours")
            .css_classes(["tonal"])
            .build();
        create.connect_clicked(on(self, |prefs| prefs.create_palette()));
        let open = gtk::Button::builder()
            .label("Open")
            .css_classes(["tonal"])
            .build();
        open.connect_clicked(on(self, |prefs| open_path(&prefs.window, &palette_file())));
        let folder = gtk::Button::builder()
            .label("Open folder")
            .css_classes(["text-btn"])
            .build();
        folder.connect_clicked(on(self, |prefs| open_path(&prefs.window, &config_dir())));
        for b in [&create, &open, &folder] {
            buttons.append(b);
        }
        row.append(&buttons);
        *self.palette_create.borrow_mut() = Some(create);
        *self.palette_open.borrow_mut() = Some(open);
        self.update_palette_buttons();
    }

    fn build_display(&self, page: &gtk::Box) {
        let card = self.section(page, "Display");
        let w = self.segmented(
            &[("small", "S"), ("medium", "M"), ("large", "L")],
            "text_size",
        );
        self.row(
            &card,
            "Text size",
            None,
            Some(&w),
            Some("text_fields"),
            false,
        );
        let w = self.segmented(
            &[
                ("compact", "Compact"),
                ("standard", "Standard"),
                ("large", "Large"),
            ],
            "button_size",
        );
        self.row(
            &card,
            "Button size",
            None,
            Some(&w),
            Some("grid_view"),
            false,
        );
        let w = self.switch("live_preview");
        self.row(
            &card,
            "Live preview",
            Some("Show the result while typing"),
            Some(&w),
            None,
            false,
        );
        let w = self.switch("digit_grouping");
        self.row(
            &card,
            "Digit grouping",
            Some("Separate thousands with a thin space"),
            Some(&w),
            None,
            false,
        );
        let w = self.segmented(
            &[("point", "Point  1.5"), ("comma", "Comma  1,5")],
            "decimal_separator",
        );
        self.row(
            &card,
            "Decimal separator",
            Some("Also decides how pasted numbers are read"),
            Some(&w),
            None,
            false,
        );
        let mut places = vec![(Value::Int(-1), "Auto".to_string())];
        places.extend((0..=10).map(|n| (Value::Int(n), n.to_string())));
        let w = self.dropdown(places, "decimals", None);
        self.row(
            &card,
            "Decimal places",
            None,
            Some(w.upcast_ref()),
            None,
            false,
        );
    }

    fn build_behaviour(&self, page: &gtk::Box, history: Rc<History>) {
        let card = self.section(page, "Behaviour");
        let w = self.segmented(&[("deg", "Degrees"), ("rad", "Radians")], "angle_unit");
        self.row(
            &card,
            "Angle unit",
            Some("Used by sin, cos, tan and their inverses"),
            Some(&w),
            Some("square_foot"),
            false,
        );
        let w = self.dropdown(
            Self::strs(&[
                ("system", "Follow system"),
                ("expressive", "Expressive"),
                ("standard", "Standard"),
                ("off", "Off"),
            ]),
            "motion",
            None,
        );
        self.row(
            &card,
            "Animations",
            Some("Expressive springs bounce; Standard does not"),
            Some(w.upcast_ref()),
            Some("animation"),
            false,
        );
        let w = self.switch("save_history");
        self.row(
            &card,
            "Save history",
            Some("Keep past calculations between sessions"),
            Some(&w),
            Some("history"),
            false,
        );
        let clear = gtk::Button::builder()
            .label("Clear")
            .css_classes(["danger"])
            .build();
        clear.connect_clicked(move |_| history.clear());
        self.row(
            &card,
            "Clear history",
            None,
            Some(clear.upcast_ref()),
            None,
            false,
        );
    }

    fn build_window(self: &Rc<Self>, page: &gtk::Box) {
        let card = self.section(page, "Window");
        let w = self.switch("remember_size");
        self.row(
            &card,
            "Remember size",
            Some("Separately for standard and scientific mode"),
            Some(&w),
            Some("desktop_windows"),
            false,
        );
        let reset = gtk::Button::builder()
            .label("Reset")
            .css_classes(["tonal"])
            .build();
        reset.connect_clicked(on(self, |prefs| {
            if let Some(main) = prefs.main.upgrade() {
                main.reset_size();
            }
        }));
        self.row(
            &card,
            "Reset window size",
            None,
            Some(reset.upcast_ref()),
            None,
            false,
        );

        let detected = window_rules::detect();
        let mut sub = "Tiling compositors decide whether a window floats. Add this rule once and Calculator always opens floating. Drag the top of the window to move it.".to_string();
        if let Some(rule) = detected.and_then(window_rules::rule) {
            sub = format!("Detected: {}. {sub}", rule.name);
        }
        let (float_row, _) = self.row(
            &card,
            "Open floating",
            Some(&sub),
            None,
            Some("open_in_new"),
            true,
        );

        let tabs = gtk::Box::builder()
            .css_classes(["connected-group", "small"])
            .build();
        let initial = detected.unwrap_or(window_rules::ORDER[0]);
        let mut first: Option<gtk::ToggleButton> = None;
        for key in window_rules::ORDER {
            let button = gtk::ToggleButton::with_label(window_rules::rule(key).unwrap().name);
            match &first {
                None => first = Some(button.clone()),
                Some(first) => button.set_group(Some(first)),
            }
            let weak = Rc::downgrade(self);
            button.connect_toggled(move |button| {
                if let (true, Some(prefs)) = (button.is_active(), weak.upgrade()) {
                    prefs.show_rule(key);
                }
            });
            tabs.append(&button);
            if key == initial {
                button.set_active(true);
            }
        }
        self.show_rule(initial);

        let copy = gtk::Button::builder()
            .css_classes(["filled"])
            .halign(gtk::Align::Start)
            .build();
        let copy_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        copy_box.append(&icon("content_copy", 18));
        copy_box.append(&gtk::Label::new(Some("Copy rule")));
        copy.set_child(Some(&copy_box));
        copy.connect_clicked(on(self, |prefs| prefs.copy_rule()));

        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        top.append(&tabs);
        float_row.append(&top);
        let code_scroll = gtk::ScrolledWindow::builder()
            .vscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .child(&self.rule_label)
            .build();
        float_row.append(&code_scroll);
        float_row.append(&self.rule_hint);
        float_row.append(&copy);
    }

    fn build_shortcuts(&self, page: &gtk::Box) {
        let card = self.section(page, "Keyboard");
        let grid = gtk::Grid::builder()
            .column_spacing(18)
            .row_spacing(6)
            .css_classes(["pref-row"])
            .build();
        for (i, (keys, what)) in SHORTCUTS.iter().enumerate() {
            grid.attach(
                &gtk::Label::builder()
                    .label(*keys)
                    .xalign(0.0)
                    .css_classes(["pref-title"])
                    .build(),
                0,
                i as i32,
                1,
                1,
            );
            grid.attach(
                &gtk::Label::builder()
                    .label(*what)
                    .xalign(0.0)
                    .wrap(true)
                    .css_classes(["pref-subtitle"])
                    .build(),
                1,
                i as i32,
                1,
                1,
            );
        }
        card.append(&grid);
    }

    fn build_about(self: &Rc<Self>, page: &gtk::Box) {
        let card = self.section(page, "About");
        let link = gtk::Button::builder()
            .label("Website")
            .css_classes(["text-btn"])
            .build();
        link.connect_clicked(on(self, |prefs| {
            gtk::UriLauncher::new(WEBSITE).launch(
                Some(&prefs.window),
                gio::Cancellable::NONE,
                |_| {},
            );
        }));
        self.row(
            &card,
            &format!("{APP_NAME} {VERSION}"),
            Some("MIT licence. Icons: Material Symbols and Material Design Icons, Apache-2.0."),
            Some(link.upcast_ref()),
            Some("calculate"),
            false,
        );
    }

    // ------------------------------------------------------------ actions
    fn update_source_rows(&self, source: &str) {
        if let Some(row) = self.custom_row.borrow().as_ref() {
            row.set_sensitive(source == "custom");
        }
        if let Some(row) = self.style_row.borrow().as_ref() {
            row.set_sensitive(source == "desktop" || source == "custom");
        }
    }

    /// Preset picker: a drop-down whose items show colour swatches.
    fn preset_dropdown(self: &Rc<Self>) -> gtk::DropDown {
        let keys: Vec<&str> = PRESETS.iter().map(|p| p.key).collect();
        let model = gtk::StringList::new(&keys);
        let factory = gtk::SignalListItemFactory::new();
        let weak = Rc::downgrade(self);
        factory.connect_setup(move |_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let weak = weak.clone();
            let colors: SwatchColors =
                Rc::new(move |key| weak.upgrade().map(|prefs| prefs.swatch_colors(key)));
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            row.append(&Swatch::new(colors));
            row.append(&gtk::Label::builder().xalign(0.0).build());
            item.set_child(Some(&row));
        });
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let key = item
                .item()
                .and_downcast::<gtk::StringObject>()
                .map(|s| s.string().to_string());
            let (Some(key), Some(row)) = (key, item.child()) else {
                return;
            };
            let Some(preset) = colors::preset(&key) else {
                return;
            };
            if let Some(swatch) = row.first_child().and_downcast::<Swatch>() {
                swatch.set_key(preset.key);
            }
            if let Some(label) = row.last_child().and_downcast::<gtk::Label>() {
                label.set_label(preset.name);
            }
        });
        let dd = gtk::DropDown::new(Some(model), gtk::Expression::NONE);
        dd.set_factory(Some(&factory));
        let current = self.settings.str("preset");
        dd.set_selected(PRESETS.iter().position(|p| p.key == current).unwrap_or(0) as u32);
        dd.connect_selected_notify(on1(self, |prefs, dd: &gtk::DropDown| {
            let Some(preset) = PRESETS.get(dd.selected() as usize) else {
                return;
            };
            let _ = prefs
                .settings
                .set("preset", Value::Str(preset.key.to_string()));
            if prefs.settings.str("color_source") != "preset" {
                if let Some(source_dd) = prefs.source_dd.borrow().as_ref() {
                    let index = SOURCES.iter().position(|(k, _)| *k == "preset").unwrap();
                    source_dd.set_selected(index as u32);
                }
            }
        }));
        dd
    }

    fn swatch_colors(&self, key: &str) -> [String; 4] {
        let dark = self.theme.dark();
        let Some(preset) = colors::preset(key) else {
            return Default::default();
        };
        if let Some(cached) = self.swatch_cache.borrow().get(&(preset.key, dark)) {
            return cached.clone();
        }
        let roles = preset.roles(dark).unwrap_or_else(|| {
            colors::generate_palette(
                colors::parse_hex(preset.seed).unwrap(),
                dark,
                0.0,
                preset.style,
            )
        });
        let pick = |role: &str| roles.get(role).unwrap_or("#000000").to_string();
        let result = [
            pick("surface_container_high"),
            pick("primary"),
            pick("secondary"),
            pick("tertiary"),
        ];
        self.swatch_cache
            .borrow_mut()
            .insert((preset.key, dark), result.clone());
        result
    }

    fn on_theme(&self) {
        if let Some(dd) = self.preset_dd.borrow().as_ref() {
            dd.queue_draw();
            for_descendants(dd.upcast_ref(), &|child| {
                if child.is::<Swatch>() {
                    child.queue_draw();
                }
            });
        }
        let state = self.theme.state();
        let mode = if state.dark { "Dark" } else { "Light" };
        let text = format!(
            "{mode} mode from {} · accent {} from {} · palette by {}",
            state.info.mode,
            colors::seed_hex(state.seed),
            state.info.accent,
            state.info.generator
        );
        if let Some(label) = self.status_subtitle.borrow().as_ref() {
            label.set_label(&text);
        }
        self.update_palette_buttons();
    }

    fn update_palette_buttons(&self) {
        let exists = palette_file().exists();
        if let Some(open) = self.palette_open.borrow().as_ref() {
            open.set_sensitive(exists);
        }
        if let Some(create) = self.palette_create.borrow().as_ref() {
            create.set_label(if exists {
                "Overwrite from current colours"
            } else {
                "Create from current colours"
            });
        }
    }

    fn create_palette(&self) {
        let path = palette_file();
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&path, self.theme.export_template()));
        if let Err(err) = written {
            eprintln!("calculator: could not write palette file: {err}");
            return;
        }
        self.update_palette_buttons();
        open_path(&self.window, &path);
    }

    fn show_rule(&self, key: &'static str) {
        let Some(rule) = window_rules::rule(key) else {
            return;
        };
        self.rule_key.set(rule.key);
        self.rule_label.set_label(&rule.snippet);
        self.rule_hint
            .set_label(&format!("Add to {}. {}", rule.file, rule.note));
    }

    fn copy_rule(&self) {
        let Some(rule) = window_rules::rule(self.rule_key.get()) else {
            return;
        };
        self.window
            .clipboard()
            .set_text(&format!("{}\n", rule.snippet));
        if let Some(main) = self.main.upgrade() {
            main.show_snackbar("Window rule copied");
        }
    }
}

/// A button handler that holds the preferences weakly.
fn on(prefs: &Rc<Prefs>, f: impl Fn(&Rc<Prefs>) + 'static) -> impl Fn(&gtk::Button) + 'static {
    let weak = Rc::downgrade(prefs);
    move |_| {
        if let Some(prefs) = weak.upgrade() {
            f(&prefs);
        }
    }
}

fn on1<W>(prefs: &Rc<Prefs>, f: impl Fn(&Rc<Prefs>, &W) + 'static) -> impl Fn(&W) + 'static {
    let weak = Rc::downgrade(prefs);
    move |w| {
        if let Some(prefs) = weak.upgrade() {
            f(&prefs, w);
        }
    }
}

fn for_descendants(widget: &gtk::Widget, f: &dyn Fn(&gtk::Widget)) {
    let mut child = widget.first_child();
    while let Some(c) = child {
        f(&c);
        for_descendants(&c, f);
        child = c.next_sibling();
    }
}

fn home_relative(path: &Path) -> String {
    let text = path.to_string_lossy().to_string();
    let home = glib::home_dir().to_string_lossy().to_string();
    match text.strip_prefix(&home) {
        Some(rest) => format!("~{rest}"),
        None => text,
    }
}

fn open_path(window: &gtk::Window, path: &Path) {
    let dir = if path.extension().is_some() {
        path.parent()
    } else {
        Some(path)
    };
    if let Some(dir) = dir {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = gio::File::for_path(path);
    gtk::FileLauncher::new(Some(&file)).launch(Some(window), gio::Cancellable::NONE, |result| {
        if let Err(err) = result {
            eprintln!("calculator: could not open the file: {err}");
        }
    });
}

// --------------------------------------------------------------------------
// Swatch
// --------------------------------------------------------------------------

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Swatch {
        pub key: Cell<Option<&'static str>>,
        pub colors: RefCell<Option<SwatchColors>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Swatch {
        const NAME: &'static str = "CalcSwatch";
        type Type = super::Swatch;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Swatch {}

    impl WidgetImpl for Swatch {
        fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let size = if orientation == gtk::Orientation::Horizontal {
                52
            } else {
                24
            };
            (size, size, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let Some(key) = self.key.get() else { return };
            let Some(colors) = self.colors.borrow().as_ref().and_then(|f| f(key)) else {
                return;
            };
            let widget = self.obj();
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let [bg, dots @ ..] = &colors;
            fill_round(snapshot, bg, 0.0, 0.0, w, h, h / 2.0);
            let r = h / 2.0 - 5.0;
            let step = (w - h) / 2.0;
            for (i, color) in dots.iter().enumerate() {
                let cx = h / 2.0 + i as f32 * step;
                fill_round(snapshot, color, cx - r, h / 2.0 - r, 2.0 * r, 2.0 * r, r);
            }
        }
    }
}

glib::wrapper! {
    /// A pill showing a preset's surface with three accent dots.
    pub struct Swatch(ObjectSubclass<imp::Swatch>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Swatch {
    fn new(colors: SwatchColors) -> Self {
        let swatch: Swatch = glib::Object::builder()
            .property("valign", gtk::Align::Center)
            .build();
        *swatch.imp().colors.borrow_mut() = Some(colors);
        swatch
    }

    fn set_key(&self, key: &'static str) {
        self.imp().key.set(Some(key));
        self.queue_draw();
    }
}

fn fill_round(snapshot: &gtk::Snapshot, hex: &str, x: f32, y: f32, w: f32, h: f32, radius: f32) {
    let Ok(rgba) = gdk::RGBA::parse(hex) else {
        return;
    };
    let rect = graphene::Rect::new(x, y, w, h);
    snapshot.push_rounded_clip(&gsk::RoundedRect::from_rect(rect, radius));
    snapshot.append_color(&rgba, &rect);
    snapshot.pop();
}
