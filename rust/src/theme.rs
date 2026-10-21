//! Desktop theme detection and CSS loading. Port of `calculator/theme.py`.
//!
//! Accent and dark/light are read from standard sources only, in this order:
//!   1. XDG Desktop Portal  (org.freedesktop.appearance)
//!   2. GSettings           (org.gnome.desktop.interface), when the schema exists
//!   3. GTK settings        (settings.ini / XSettings via Gtk.Settings)
//!   4. Named colours of the active GTK theme
//!   5. Built-in fallback
//!
//! User choices in Preferences or palette.toml take priority over all of these.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use calculator::colors::{self, Roles, Seed, FALLBACK_SEED};
use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::config::{config_dir, Settings};

/// The Python app's stylesheet, shared as is.
const STYLE: &str = include_str!("../../calculator/style.css");

pub fn palette_file() -> std::path::PathBuf {
    config_dir().join("palette.toml")
}

// --------------------------------------------------------------------------
// Desktop settings
// --------------------------------------------------------------------------

/// Watches the desktop's colour scheme, accent, contrast and motion.
struct Desktop {
    portal: RefCell<HashMap<String, glib::Variant>>,
    portal_proxy: RefCell<Option<gio::DBusProxy>>,
    gsettings: Option<gio::Settings>,
    gsettings_keys: Vec<&'static str>,
    initial_prefer_dark: bool,
    theme_bg: Cell<Option<(f64, f64, f64)>>,
    theme_accent: Cell<Option<(f64, f64, f64)>>,
}

/// Unwrap a value that a portal may have boxed in an extra variant.
fn unbox(value: glib::Variant) -> glib::Variant {
    if value.type_().as_str() == "v" {
        value.as_variant().unwrap_or(value)
    } else {
        value
    }
}

impl Desktop {
    fn new(on_change: Rc<dyn Fn()>) -> Rc<Self> {
        let gtk_settings = gtk::Settings::default();
        let source = gio::SettingsSchemaSource::default();
        let schema = source.and_then(|s| s.lookup("org.gnome.desktop.interface", true));
        let (gsettings, gsettings_keys) = match &schema {
            Some(schema) => {
                let keys = [
                    "color-scheme",
                    "accent-color",
                    "gtk-theme",
                    "enable-animations",
                ]
                .into_iter()
                .filter(|k| schema.has_key(k))
                .collect();
                (
                    Some(gio::Settings::new("org.gnome.desktop.interface")),
                    keys,
                )
            }
            None => (None, Vec::new()),
        };
        let desktop = Rc::new(Desktop {
            portal: RefCell::default(),
            portal_proxy: RefCell::default(),
            gsettings,
            gsettings_keys,
            // Read before we start setting prefer-dark ourselves.
            initial_prefer_dark: gtk_settings
                .as_ref()
                .is_some_and(|s| s.is_gtk_application_prefer_dark_theme()),
            theme_bg: Cell::new(None),
            theme_accent: Cell::new(None),
        });
        desktop.read_theme_colors();
        desktop.setup_portal(on_change.clone());
        if let Some(gsettings) = &desktop.gsettings {
            let weak = Rc::downgrade(&desktop);
            let changed = on_change.clone();
            gsettings.connect_changed(None, move |_, key| {
                if weak
                    .upgrade()
                    .is_some_and(|d| d.gsettings_keys.contains(&key))
                {
                    changed();
                }
            });
        }
        if let Some(settings) = gtk_settings {
            let weak = Rc::downgrade(&desktop);
            let changed = on_change.clone();
            settings.connect_gtk_theme_name_notify(move |_| {
                if let Some(desktop) = weak.upgrade() {
                    desktop.read_theme_colors();
                    changed();
                }
            });
            settings.connect_gtk_enable_animations_notify(move |_| on_change());
        }
        desktop
    }

    fn read_theme_colors(&self) {
        // Cached: once we set prefer-dark ourselves the theme's colours follow us.
        self.theme_bg
            .set(theme_color(&["window_bg_color", "theme_bg_color"]));
        self.theme_accent.set(theme_color(&[
            "accent_bg_color",
            "accent_color",
            "theme_selected_bg_color",
        ]));
    }

    // Portal --------------------------------------------------------------
    fn setup_portal(self: &Rc<Self>, on_change: Rc<dyn Fn()>) {
        let proxy = gio::DBusProxy::for_bus_sync(
            gio::BusType::Session,
            gio::DBusProxyFlags::DO_NOT_LOAD_PROPERTIES,
            None,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            gio::Cancellable::NONE,
        );
        let Ok(proxy) = proxy else { return };
        let args = (vec!["org.freedesktop.appearance"],).to_variant();
        let reply = proxy.call_sync(
            "ReadAll",
            Some(&args),
            gio::DBusCallFlags::NONE,
            1500,
            gio::Cancellable::NONE,
        );
        let Ok(reply) = reply else { return };
        // (a{sa{sv}})
        let namespaces = reply.child_value(0);
        let mut portal = self.portal.borrow_mut();
        for i in 0..namespaces.n_children() {
            let entry = namespaces.child_value(i);
            if entry.child_value(0).str() != Some("org.freedesktop.appearance") {
                continue;
            }
            let values = entry.child_value(1);
            for j in 0..values.n_children() {
                let pair = values.child_value(j);
                if let (Some(key), Some(value)) =
                    (pair.child_value(0).str(), pair.child_value(1).as_variant())
                {
                    portal.insert(key.to_string(), unbox(value));
                }
            }
        }
        drop(portal);
        let weak = Rc::downgrade(self);
        // connect_local: the handler holds Rc values, so it must stay on this thread.
        proxy.connect_local("g-signal", false, move |values| {
            let signal = values.get(2).and_then(|v| v.get::<String>().ok());
            let params = values.get(3).and_then(|v| v.get::<glib::Variant>().ok());
            let (Some(signal), Some(params)) = (signal, params) else {
                return None;
            };
            if signal != "SettingChanged" || params.n_children() < 3 {
                return None;
            }
            let desktop = weak.upgrade()?;
            if params.child_value(0).str() == Some("org.freedesktop.appearance") {
                if let (Some(key), Some(value)) = (
                    params.child_value(1).str(),
                    params.child_value(2).as_variant(),
                ) {
                    desktop
                        .portal
                        .borrow_mut()
                        .insert(key.to_string(), unbox(value));
                    on_change();
                }
            }
            None
        });
        *self.portal_proxy.borrow_mut() = Some(proxy);
    }

    fn portal_u32(&self, key: &str) -> Option<u32> {
        self.portal.borrow().get(key).and_then(|v| v.get::<u32>())
    }

    // GSettings -----------------------------------------------------------
    fn gs(&self, key: &str) -> Option<glib::Variant> {
        let settings = self.gsettings.as_ref()?;
        self.gsettings_keys
            .contains(&key)
            .then(|| settings.value(key))
    }

    // Queries -------------------------------------------------------------
    /// (is_dark, source)
    fn dark(&self) -> (bool, &'static str) {
        if let Some(scheme @ (1 | 2)) = self.portal_u32("color-scheme") {
            return (scheme == 1, "portal");
        }
        match self
            .gs("color-scheme")
            .and_then(|v| v.get::<String>())
            .as_deref()
        {
            Some("prefer-dark") => return (true, "GSettings"),
            Some("prefer-light") => return (false, "GSettings"),
            _ => {}
        }
        if self.initial_prefer_dark {
            return (true, "GTK settings");
        }
        let theme_name = gtk::Settings::default()
            .and_then(|s| s.gtk_theme_name())
            .map(|n| n.to_lowercase())
            .unwrap_or_default();
        if theme_name.ends_with("-dark") || theme_name.contains(":dark") {
            return (true, "GTK theme");
        }
        if let Some(bg) = self.theme_bg.get() {
            return (colors::luminance(bg) < 0.2, "GTK theme");
        }
        (false, "default")
    }

    /// (seed or None, source)
    fn accent(&self) -> (Option<Seed>, &'static str) {
        let accent = self
            .portal
            .borrow()
            .get("accent-color")
            .and_then(|v| v.get::<(f64, f64, f64)>());
        if let Some((r, g, b)) =
            accent.filter(|&(r, g, b)| [r, g, b].iter().all(|c| (0.0..=1.0).contains(c)))
        {
            let c = |v: f64| (v * 255.0).round_ties_even() as u8;
            return (Some((c(r), c(g), c(b))), "portal");
        }
        if let Some(seed) = self
            .gs("accent-color")
            .and_then(|v| v.get::<String>())
            .and_then(|n| colors::gnome_accent(&n))
        {
            return (Some(seed), "GSettings");
        }
        if let Some((r, g, b)) = self.theme_accent.get() {
            let c = |v: f64| v.round_ties_even() as u8;
            return (Some((c(r), c(g), c(b))), "GTK theme");
        }
        (None, "default")
    }

    fn high_contrast(&self) -> bool {
        self.portal_u32("contrast") == Some(1)
    }

    fn reduced_motion(&self) -> bool {
        if self.portal_u32("reduced-motion") == Some(1) {
            return true;
        }
        if self.gs("enable-animations").and_then(|v| v.get::<bool>()) == Some(false) {
            return true;
        }
        !gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations())
    }
}

/// A named colour of the active GTK theme, as 0–255 floats.
#[allow(deprecated)] // GtkStyleContext is the only way to read theme colours
fn theme_color(names: &[&str]) -> Option<(f64, f64, f64)> {
    let context = gtk::Label::new(None).style_context();
    names.iter().find_map(|name| {
        let rgba = context.lookup_color(name)?;
        (rgba.alpha() > 0.5).then(|| {
            (
                f64::from(rgba.red()) * 255.0,
                f64::from(rgba.green()) * 255.0,
                f64::from(rgba.blue()) * 255.0,
            )
        })
    })
}

// --------------------------------------------------------------------------
// Theme manager
// --------------------------------------------------------------------------

/// Where the current colours came from, for Preferences.
#[derive(Debug, Clone, Default)]
pub struct Info {
    pub mode: String,
    pub accent: String,
    pub generator: String,
}

#[derive(Debug, Clone)]
pub struct State {
    pub roles: Roles,
    pub dark: bool,
    pub seed: Seed,
    pub style: String,
    pub info: Info,
}

pub struct Theme {
    settings: Rc<Settings>,
    desktop: RefCell<Option<Rc<Desktop>>>,
    provider: gtk::CssProvider,
    pending: Cell<bool>,
    listeners: RefCell<Vec<Rc<dyn Fn()>>>,
    state: RefCell<State>,
    palette_monitor: RefCell<Option<gio::FileMonitor>>,
}

impl Theme {
    pub fn new(settings: Rc<Settings>) -> Rc<Self> {
        let theme = Rc::new(Theme {
            settings: settings.clone(),
            desktop: RefCell::default(),
            provider: gtk::CssProvider::new(),
            pending: Cell::new(false),
            listeners: RefCell::default(),
            state: RefCell::new(State {
                roles: Roles::default(),
                dark: false,
                seed: FALLBACK_SEED,
                style: "tonal_spot".into(),
                info: Info::default(),
            }),
            palette_monitor: RefCell::default(),
        });
        let weak = Rc::downgrade(&theme);
        let queue: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(theme) = weak.upgrade() {
                theme.queue_update();
            }
        });
        *theme.desktop.borrow_mut() = Some(Desktop::new(queue.clone()));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &theme.provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        theme.watch_palette(queue.clone());
        settings.connect(move |key| {
            if matches!(
                key,
                "theme"
                    | "color_source"
                    | "preset"
                    | "custom_accent"
                    | "palette_style"
                    | "contrast"
            ) {
                queue();
            }
        });
        theme.update();
        theme
    }

    fn desktop(&self) -> Rc<Desktop> {
        self.desktop.borrow().clone().unwrap()
    }

    pub fn connect(&self, callback: impl Fn() + 'static) {
        self.listeners.borrow_mut().push(Rc::new(callback));
    }

    pub fn state(&self) -> State {
        self.state.borrow().clone()
    }

    pub fn dark(&self) -> bool {
        self.state.borrow().dark
    }

    pub fn reduced_motion(&self) -> bool {
        self.desktop().reduced_motion()
    }

    fn watch_palette(&self, queue: Rc<dyn Fn()>) {
        let path = palette_file();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let monitor = gio::File::for_path(&path)
            .monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE);
        if let Ok(monitor) = monitor {
            monitor.connect_changed(move |_, _, _, _| queue());
            *self.palette_monitor.borrow_mut() = Some(monitor);
        }
    }

    fn queue_update(self: &Rc<Self>) {
        if self.pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(60), move || {
            if let Some(theme) = weak.upgrade() {
                theme.pending.set(false);
                theme.update();
            }
        });
    }

    pub fn compute(&self) -> State {
        let s = &self.settings;
        let desktop = self.desktop();
        let source = s.str("color_source");
        let palette = if source == "palette" {
            colors::read_palette_file(&palette_file())
        } else {
            None
        };
        let mut info = Info::default();

        // Light or dark
        let dark = if let Some(mode) = palette.as_ref().and_then(|p| p.mode.clone()) {
            info.mode = "palette file".into();
            mode == "dark"
        } else if s.str("theme") != "system" {
            info.mode = "Preferences".into();
            s.str("theme") == "dark"
        } else {
            let (dark, from) = desktop.dark();
            info.mode = from.into();
            dark
        };

        let preset = if source == "preset" {
            colors::preset(&s.str("preset"))
        } else {
            None
        };

        // Seed colour
        let mut seed = None;
        if let Some(preset) = preset {
            seed = colors::parse_hex(preset.seed);
            info.accent = format!("preset “{}”", preset.name);
        } else if source == "custom" {
            seed = colors::parse_hex(&s.str("custom_accent"));
            info.accent = "Preferences".into();
        } else if let Some(file_seed) = palette.as_ref().and_then(|p| p.seed) {
            seed = Some(file_seed);
            info.accent = "palette file".into();
        }
        if seed.is_none() {
            let (desktop_seed, from) = desktop.accent();
            seed = desktop_seed;
            info.accent = from.into();
        }
        let seed = seed.unwrap_or_else(|| {
            info.accent = "default".into();
            FALLBACK_SEED
        });

        let style = palette
            .as_ref()
            .and_then(|p| p.style.clone())
            .or_else(|| preset.map(|p| p.style.to_string()))
            .unwrap_or_else(|| s.str("palette_style"));
        let contrast = match s.str("contrast").as_str() {
            "system" => {
                if desktop.high_contrast() {
                    1.0
                } else {
                    0.0
                }
            }
            level => colors::contrast_level(level).unwrap_or(0.0),
        };

        let mut roles = colors::generate_palette(seed, dark, contrast, &style);
        info.generator = "material-colors".into();
        if let Some(own) = preset.and_then(|p| p.roles(dark)) {
            roles.update(&own);
            info.generator = format!("the {} theme", preset.unwrap().name);
        }
        if let Some(palette) = &palette {
            roles.update(&palette.roles);
            roles.update(if dark { &palette.dark } else { &palette.light });
        }
        State {
            roles,
            dark,
            seed,
            style,
            info,
        }
    }

    pub fn update(&self) {
        let state = self.compute();
        let mut css: String = state
            .roles
            .iter()
            .map(|(role, hex)| format!("@define-color {role} {hex};\n"))
            .collect();
        css.push_str(STYLE);
        #[allow(deprecated)] // load_from_string needs GTK 4.12
        self.provider.load_from_data(&css);
        if let Some(gtk_settings) = gtk::Settings::default() {
            if gtk_settings.is_gtk_application_prefer_dark_theme() != state.dark {
                gtk_settings.set_gtk_application_prefer_dark_theme(state.dark);
            }
        }
        *self.state.borrow_mut() = state;
        let listeners: Vec<_> = self.listeners.borrow().clone();
        for listener in listeners {
            listener();
        }
    }

    pub fn export_template(&self) -> String {
        let state = self.state.borrow();
        colors::palette_template(&state.roles, state.seed, &state.style, state.dark)
    }
}
