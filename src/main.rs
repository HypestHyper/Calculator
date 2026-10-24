//! Application entry point.

mod app_icon;
mod compositor;
mod config;
mod icons;
mod motion;
mod placement;
mod preferences;
mod scale_box;
mod theme;
mod timing;
mod window;
mod window_rules;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use config::{History, Settings, APP_ID, APP_NAME, VERSION};
use preferences::Prefs;
use theme::Theme;
use window::Win;

/// Everything the running app holds on to.
struct App {
    settings: Rc<Settings>,
    history: Rc<History>,
    theme: Rc<Theme>,
    window: Rc<Win>,
    prefs: RefCell<Option<Rc<Prefs>>>,
}

fn main() -> glib::ExitCode {
    timing::start();
    // GTK's GL renderer starts noticeably faster than its Vulkan one, the
    // default in newer GTK versions; GSK_RENDERER still picks another.
    if std::env::var_os("GSK_RENDERER").is_none() {
        std::env::set_var("GSK_RENDERER", "ngl");
    }
    glib::set_prgname(Some(APP_ID)); // X11 WM_CLASS; Wayland uses the application ID
    glib::set_application_name(APP_NAME);

    // One window per user. Development builds and debug
    // runs start a separate process, so they can run next to an installed
    // version (which shares the application ID) and print their own output.
    let separate = cfg!(debug_assertions) || std::env::var_os("CALCULATOR_DEBUG").is_some();
    let flags = if separate {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
    };
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();
    let rules = window_rules::ORDER.join("|");
    app.add_main_option(
        "print-window-rules",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::String,
        "Print the floating-window rule for a compositor and exit",
        Some(&rules),
    );
    app.add_main_option(
        "print-palette",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Print the active palette as palette.toml and exit",
        None,
    );
    app.add_main_option(
        "version",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Show the version and exit",
        None,
    );
    let print_palette = Rc::new(Cell::new(false));
    let flag = print_palette.clone();
    app.connect_handle_local_options(move |_, options| {
        if options.contains("version") {
            println!("{APP_NAME} {VERSION}");
            return 0;
        }
        if let Ok(Some(key)) = options.lookup::<String>("print-window-rules") {
            return match window_rules::text_for(&key.to_lowercase()) {
                Some(text) => {
                    print!("{text}");
                    0
                }
                None => {
                    eprintln!(
                        "Unknown compositor “{key}”. Choose one of: {}",
                        window_rules::ORDER.join(", ")
                    );
                    2
                }
            };
        }
        flag.set(options.contains("print-palette"));
        -1
    });

    let state: Rc<RefCell<Option<Rc<App>>>> = Rc::default();
    app.connect_startup(|_| {
        timing::mark("GTK started, app registered");
        icons::load_font();
        timing::mark("icon font loaded");
    });
    app.connect_activate(move |app| {
        if print_palette.get() {
            let settings = Settings::load_default();
            println!("{}", Theme::new(settings).export_template());
            app.quit();
            return;
        }
        let existing = state.borrow().clone();
        let running = existing.unwrap_or_else(|| {
            let running = start(app);
            *state.borrow_mut() = Some(running.clone());
            running
        });
        running.window.window.present();
        timing::mark("window shown");
        // For development: quit once the first frame is drawn, to time start-up.
        let quit = std::env::var_os("CALCULATOR_QUIT_ON_FIRST_FRAME").is_some();
        if quit || timing::enabled() {
            let app = app.clone();
            running.window.window.add_tick_callback(move |_, _| {
                timing::mark("first frame");
                if quit {
                    app.quit();
                }
                glib::ControlFlow::Break
            });
        }
        if let Ok(path) = std::env::var("CALCULATOR_SCREENSHOT") {
            screenshot_and_quit(app, running.clone(), path);
        }
    });
    let code = app.run();
    timing::mark("closed");
    code
}

fn start(app: &gtk::Application) -> Rc<App> {
    if std::env::var_os("CALCULATOR_DEBUG").is_some() {
        println!(
            "calculator: {APP_NAME} {VERSION} (Rust) debug on, desktop {}",
            std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "?".into())
        );
    }
    let settings = Settings::load_default();
    let history = History::load_default(settings.bool("save_history"));
    timing::mark("settings and history read");
    let theme = Theme::new(settings.clone());
    timing::mark("colours ready");
    let window = Win::new(app, settings.clone(), history.clone(), theme.clone());
    timing::mark("window built");
    let running = Rc::new(App {
        settings,
        history,
        theme,
        window,
        prefs: RefCell::default(),
    });

    let update_icon = {
        let weak = Rc::downgrade(&running);
        move || {
            if let Some(app) = weak.upgrade() {
                app_icon::update(&app.theme.state().roles, app.settings.bool("themed_icon"));
            }
        }
    };
    update_icon();
    running.theme.connect(update_icon.clone());
    let weak = Rc::downgrade(&running);
    running.settings.connect(move |key| {
        let Some(app) = weak.upgrade() else { return };
        match key {
            "save_history" => app.history.set_enabled(app.settings.bool("save_history")),
            "themed_icon" => update_icon(),
            _ => {}
        }
    });

    let weak = Rc::downgrade(&running);
    running.window.set_preferences_handler(move || {
        if let Some(app) = weak.upgrade() {
            app.show_preferences();
        }
    });
    let weak = Rc::downgrade(&running);
    running.window.window.connect_close_request(move |_| {
        // The hidden preferences window would otherwise keep the app running.
        if let Some(prefs) = weak.upgrade().and_then(|app| app.prefs.borrow_mut().take()) {
            prefs.window.destroy();
        }
        glib::Propagation::Proceed
    });
    running
}

impl App {
    fn show_preferences(&self) {
        let existing = self.prefs.borrow().clone();
        let prefs = existing.unwrap_or_else(|| {
            let prefs = Prefs::new(
                self.settings.clone(),
                self.history.clone(),
                self.theme.clone(),
                &self.window,
            );
            prefs.window.set_hide_on_close(true);
            *self.prefs.borrow_mut() = Some(prefs.clone());
            prefs
        });
        prefs.window.present();
    }
}

/// For development: run the steps in CALCULATOR_KEYS (space-separated key
/// actions such as "digit:7 op:× eq:", or "ui:mode", "ui:inv", "ui:angle",
/// "ui:history", "ui:size:WxH", "ui:wait"), then save the window as a PNG and quit. Each
/// "ui:wait" pauses a second, for transitions.
fn screenshot_and_quit(app: &gtk::Application, running: Rc<App>, path: String) {
    let keys = std::env::var("CALCULATOR_KEYS").unwrap_or_default();
    let steps: Vec<String> = keys
        .split(' ')
        .filter(|k| !k.is_empty())
        .map(str::to_string)
        .collect();
    // No shadow, so the corners around the rounded window stay transparent.
    let provider = gtk::CssProvider::new();
    provider.load_from_data("window.csd { box-shadow: none; }");
    gtk::style_context_add_provider_for_display(
        &gtk::prelude::WidgetExt::display(&running.window.window),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
    let app = app.clone();
    glib::timeout_add_local_once(Duration::from_millis(800), move || {
        run_steps(app, running, steps, path)
    });
}

fn run_steps(app: gtk::Application, running: Rc<App>, mut steps: Vec<String>, path: String) {
    let win = running.window.clone();
    while !steps.is_empty() {
        let step = steps.remove(0);
        match step.strip_prefix("ui:") {
            Some("wait") => {
                glib::timeout_add_local_once(Duration::from_millis(1000), move || {
                    run_steps(app, running, steps, path)
                });
                return;
            }
            Some("prefs") => running.show_preferences(),
            Some(action) if action.starts_with("set:") => {
                // set:key=value, as Preferences would; ints and booleans are parsed.
                let (key, raw) = action["set:".len()..].split_once('=').unwrap_or_default();
                let value = match raw {
                    "true" => config::Value::Bool(true),
                    "false" => config::Value::Bool(false),
                    _ => raw
                        .parse()
                        .map_or_else(|_| config::Value::Str(raw.to_string()), config::Value::Int),
                };
                if let Err(err) = running.settings.set(key, value) {
                    eprintln!("calculator: {err}");
                }
            }
            Some(action) if action.starts_with("scroll:") => {
                let offset: f64 = action["scroll:".len()..].parse().unwrap_or(0.0);
                let prefs = running.prefs.borrow().clone();
                if let Some(scroll) = prefs
                    .and_then(|p| p.window.child())
                    .and_downcast::<gtk::ScrolledWindow>()
                {
                    scroll.vadjustment().set_value(offset);
                }
            }
            Some(action) => win.dev_action(action),
            None => win.press(&step),
        }
    }
    if let Ok(dir) = std::env::var("CALCULATOR_FRAMES") {
        record_frames(app, win, dir, 0);
        return;
    }
    glib::timeout_add_local_once(Duration::from_millis(1500), move || {
        // The preferences window when it is open, else the calculator.
        let prefs = running
            .prefs
            .borrow()
            .clone()
            .filter(|p| p.window.is_visible());
        let window: gtk::Window = match &prefs {
            Some(prefs) => prefs.window.clone(),
            None => win.window.clone().upcast(),
        };
        eprintln!("calculator: window {}x{}", window.width(), window.height());
        let (w, h) = (window.width(), window.height());
        let paintable = gtk::WidgetPaintable::new(Some(&window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(&snapshot, f64::from(w), f64::from(h));
        let renderer = window.native().and_then(|n| n.renderer());
        match (snapshot.to_node(), renderer) {
            (Some(node), Some(renderer)) => {
                // Just the window: its shadow, outside, is left out.
                let bounds = gtk::graphene::Rect::new(0.0, 0.0, w as f32, h as f32);
                let texture = renderer.render_texture(&node, Some(&bounds));
                if let Err(err) = texture.save_to_png(&path) {
                    eprintln!("calculator: could not save {path}: {err}");
                }
            }
            _ => eprintln!("calculator: nothing to save"),
        }
        app.quit();
    });
}

/// For development: 40 frames, 40 ms apart, after the steps; prints the window
/// width and where the "=" key is, and saves every other frame as a PNG.
fn record_frames(app: gtk::Application, win: Rc<Win>, dir: String, i: u32) {
    let window = &win.window;
    if i.is_multiple_of(2) {
        let paintable = gtk::WidgetPaintable::new(Some(window));
        let snapshot = gtk::Snapshot::new();
        paintable.snapshot(
            &snapshot,
            f64::from(window.width()),
            f64::from(window.height()),
        );
        if let (Some(node), Some(renderer)) = (
            snapshot.to_node(),
            window.native().and_then(|n| n.renderer()),
        ) {
            let _ = renderer
                .render_texture(&node, None)
                .save_to_png(format!("{dir}/f{i:03}.png"));
        }
    }
    let (x, w) = win.dev_key_bounds("eq:").unwrap_or((-1.0, -1.0));
    let (mx, mw) = win.dev_key_bounds("op: mod ").unwrap_or((-1.0, -1.0));
    println!(
        "t={:4} ms  window {:4}  '=' key width {:3} at x={:3}   'mod' key width {:3} at x={:4}",
        i * 40,
        window.width(),
        w.round(),
        x.round(),
        mw.round(),
        mx.round()
    );
    if i >= 40 {
        app.quit();
        return;
    }
    glib::timeout_add_local_once(Duration::from_millis(40), move || {
        record_frames(app, win, dir, i + 1)
    });
}
