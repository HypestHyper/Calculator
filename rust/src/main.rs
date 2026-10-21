//! Application entry point. Port of `calculator/app.py`.

mod compositor;
mod config;
mod icons;
mod motion;
mod palette;
mod window;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gio, glib};

use config::{History, Settings, APP_ID, APP_NAME, VERSION};
use palette::Theme;
use window::Win;

fn main() -> glib::ExitCode {
    glib::set_prgname(Some(APP_ID)); // X11 WM_CLASS; Wayland uses the application ID
    glib::set_application_name(APP_NAME);

    // Non-unique while the Rust version runs alongside the Python one: with the
    // same application ID, a second launch would otherwise just raise the
    // other app's window.
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.add_main_option(
        "version",
        glib::Char::from(0),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Show the version and exit",
        None,
    );
    app.connect_handle_local_options(|_, options| {
        if options.contains("version") {
            println!("{APP_NAME} {VERSION} (Rust)");
            return 0;
        }
        -1
    });

    let main_window: Rc<RefCell<Option<Rc<Win>>>> = Rc::default();
    app.connect_startup(|_| {
        icons::load_font();
    });
    app.connect_activate(move |app| {
        let mut slot = main_window.borrow_mut();
        let win = slot.get_or_insert_with(|| {
            let settings = Settings::load_default();
            let history = History::load_default(settings.bool("save_history"));
            let theme = Rc::new(Theme::install());
            theme.apply(&settings.str("theme"));
            {
                let history = history.clone();
                let settings2 = settings.clone();
                settings.connect(move |key| {
                    if key == "save_history" {
                        history.set_enabled(settings2.bool("save_history"));
                    }
                });
            }
            Win::new(app, settings, history, theme)
        });
        win.window.present();
        // For development: quit once the first frame is drawn, to time start-up.
        if std::env::var_os("CALCULATOR_QUIT_ON_FIRST_FRAME").is_some() {
            let app = app.clone();
            win.window.add_tick_callback(move |_, _| {
                app.quit();
                glib::ControlFlow::Break
            });
        }
        if let Ok(path) = std::env::var("CALCULATOR_SCREENSHOT") {
            screenshot_and_quit(app, win.clone(), path);
        }
    });
    app.run()
}

/// For development: run the steps in CALCULATOR_KEYS (space-separated key
/// actions such as "digit:7 op:× eq:", or "ui:mode", "ui:inv", "ui:angle",
/// "ui:history", "ui:wait"), then save the window as a PNG and quit. Each
/// "ui:wait" pauses a second, for transitions.
fn screenshot_and_quit(app: &gtk::Application, win: Rc<Win>, path: String) {
    let keys = std::env::var("CALCULATOR_KEYS").unwrap_or_default();
    let steps: Vec<String> = keys
        .split(' ')
        .filter(|k| !k.is_empty())
        .map(str::to_string)
        .collect();
    let app = app.clone();
    glib::timeout_add_local_once(Duration::from_millis(800), move || {
        run_steps(app, win, steps, path)
    });
}

fn run_steps(app: gtk::Application, win: Rc<Win>, mut steps: Vec<String>, path: String) {
    while !steps.is_empty() {
        let step = steps.remove(0);
        match step.strip_prefix("ui:") {
            Some("wait") => {
                glib::timeout_add_local_once(Duration::from_millis(1000), move || {
                    run_steps(app, win, steps, path)
                });
                return;
            }
            Some(action) => win.dev_action(action),
            None => win.press(&step),
        }
    }
    {
        glib::timeout_add_local_once(Duration::from_millis(1500), move || {
            eprintln!(
                "calculator: window {}x{}",
                win.window.width(),
                win.window.height()
            );
            let window = &win.window;
            let (w, h) = (window.width(), window.height());
            let paintable = gtk::WidgetPaintable::new(Some(window));
            let snapshot = gtk::Snapshot::new();
            paintable.snapshot(&snapshot, f64::from(w), f64::from(h));
            let renderer = window.native().and_then(|n| n.renderer());
            match (snapshot.to_node(), renderer) {
                (Some(node), Some(renderer)) => {
                    let texture = renderer.render_texture(&node, None);
                    if let Err(err) = texture.save_to_png(&path) {
                        eprintln!("calculator: could not save {path}: {err}");
                    }
                }
                _ => eprintln!("calculator: nothing to save"),
            }
            app.quit();
        });
    }
}
