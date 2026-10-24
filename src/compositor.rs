//! Compositor-specific window resizing.
//!
//! Some compositors ignore a floating window's own resize requests. Hyprland is one of them, but it can be asked
//! through hyprctl. Everything here is asynchronous and best-effort: on any
//! failure the callback gets false and the window falls back to the generic
//! method.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::{gio, glib};

use crate::config::APP_ID;

pub fn is_hyprland() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some_and(|v| !v.is_empty())
        && gtk::glib::find_program_in_path("hyprctl").is_some()
}

/// Run a command; callback(ok, stdout).
pub(crate) fn run(argv: &[&str], callback: impl FnOnce(bool, String) + 'static) {
    let args: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
    let flags = gio::SubprocessFlags::STDOUT_PIPE | gio::SubprocessFlags::STDERR_MERGE;
    let proc = match gio::Subprocess::newv(&args, flags) {
        Ok(proc) => proc,
        Err(_) => return callback(false, String::new()),
    };
    let finished = proc.clone();
    proc.communicate_utf8_async(None, gio::Cancellable::NONE, move |result| match result {
        Ok((out, _)) => callback(
            finished.is_successful(),
            out.map(|s| s.to_string()).unwrap_or_default(),
        ),
        Err(_) => callback(false, String::new()),
    });
}

pub(crate) type Debug = Rc<dyn Fn(&str)>;

/// Try Hyprland's Lua dispatch (0.55+), then the classic syntax.
pub(crate) fn dispatch(
    lua_expr: String,
    classic_args: Vec<String>,
    debug: Debug,
    callback: impl FnOnce(bool) + 'static,
) {
    run(&["hyprctl", "dispatch", &lua_expr], move |ok, out| {
        debug(&format!("hyprctl (lua) -> {:?}", out.trim()));
        if ok && out.trim() == "ok" {
            return callback(true);
        }
        let mut argv = vec!["hyprctl", "dispatch"];
        argv.extend(classic_args.iter().map(String::as_str));
        run(&argv, move |ok2, out2| {
            debug(&format!("hyprctl (classic) -> {:?}", out2.trim()));
            callback(ok2 && out2.trim() == "ok");
        });
    });
}

/// Resize this app's window to width x height, keeping its centre.
pub fn hyprland_resize(
    width: i32,
    height: i32,
    debug: Debug,
    callback: impl FnOnce(bool) + 'static,
) {
    run(&["hyprctl", "clients", "-j"], move |ok, out| {
        let client = ok.then(|| find_client(&out, &debug)).flatten();
        let Some(Client {
            target,
            at: (x, y),
            size: (w, h),
        }) = client
        else {
            return callback(false);
        };
        let nx = (x + (w - f64::from(width)) / 2.0).round_ties_even() as i64;
        let ny = (y + (h - f64::from(height)) / 2.0).round_ties_even() as i64;

        let move_lua = format!(
            r#"hl.dsp.window.move({{ x = {nx}, y = {ny}, relative = false, window = "{target}" }})"#
        );
        let move_classic = vec![
            "movewindowpixel".to_string(),
            format!("exact {nx} {ny},{target}"),
        ];
        let debug2 = debug.clone();
        dispatch(
            format!(
                r#"hl.dsp.window.resize({{ x = {width}, y = {height}, relative = false, window = "{target}" }})"#
            ),
            vec![
                "resizewindowpixel".to_string(),
                format!("exact {width} {height},{target}"),
            ],
            debug,
            move |resized| {
                if !resized {
                    return callback(false);
                }
                dispatch(move_lua, move_classic, debug2, move |_| callback(true));
            },
        );
    });
}

/// Hide or show Hyprland's own animation for the window.
pub(crate) fn set_no_anim(
    target: &str,
    value: &str,
    debug: Debug,
    callback: impl FnOnce(bool) + 'static,
) {
    set_prop(target, "no_anim", "noanim", value, debug, callback);
}

/// Set a window property: `prop` in Lua, `classic` in the older syntax.
fn set_prop(
    target: &str,
    prop: &str,
    classic: &str,
    value: &str,
    debug: Debug,
    callback: impl FnOnce(bool) + 'static,
) {
    dispatch(
        format!(
            r#"hl.dsp.window.set_prop({{ prop = "{prop}", value = "{value}", window = "{target}" }})"#
        ),
        vec!["setprop".to_string(), format!("{target} {classic} {value}")],
        debug,
        callback,
    );
}

/// Make Hyprland keep this app's window's proportions while it is resized
/// with the mouse: those it has when the resize starts.
pub fn hyprland_keep_aspect(debug: Debug) {
    run(&["hyprctl", "clients", "-j"], move |ok, out| {
        let Some(client) = ok.then(|| find_client(&out, &debug)).flatten() else {
            return;
        };
        let debug2 = debug.clone();
        set_prop(
            &client.target,
            "keep_aspect_ratio",
            "keepaspectratio",
            "1",
            debug,
            move |ok| {
                debug2(&format!(
                    "keep aspect ratio: {}",
                    if ok { "on" } else { "failed" }
                ));
            },
        );
    });
}

/// Shrink or grow this app's window to width x height in small steps over
/// `duration` seconds, keeping its centre. Hyprland's own animation would
/// move the window's box while the app already shows the final size; in
/// steps, the window and its contents change size together.
pub fn hyprland_animate_resize(
    width: i32,
    height: i32,
    duration: f64,
    debug: Debug,
    callback: impl FnOnce(bool) + 'static,
) {
    run(&["hyprctl", "clients", "-j"], move |ok, out| {
        let client = ok.then(|| find_client(&out, &debug)).flatten();
        let Some(client) = client else {
            return callback(false);
        };
        let target = client.target.clone();
        let debug2 = debug.clone();
        set_no_anim(&target, "1", debug.clone(), move |_| {
            let stepper = Rc::new(Stepper {
                client,
                to: (f64::from(width), f64::from(height)),
                duration,
                debug: debug2,
                start: glib::monotonic_time(),
                lua: Cell::new(true),
                busy: Cell::new(false),
                sent: Cell::new(None),
                callback: RefCell::new(Some(Box::new(callback))),
            });
            glib::timeout_add_local(Duration::from_millis(16), move || stepper.tick());
        });
    });
}

pub(crate) struct Client {
    pub target: String,
    pub at: (f64, f64),
    pub size: (f64, f64),
}

pub(crate) fn find_client(out: &str, debug: &Debug) -> Option<Client> {
    let clients = serde_json::from_str::<Vec<serde_json::Value>>(out).ok()?;
    let pid = u64::from(std::process::id());
    let client = clients.iter().find(|c| {
        c.get("pid").and_then(|p| p.as_u64()) == Some(pid)
            && c.get("class").and_then(|c| c.as_str()) == Some(APP_ID)
    });
    let Some(client) = client else {
        debug("hyprctl: window not found in clients");
        return None;
    };
    let pair = |key: &str| -> (f64, f64) {
        let v = client.get(key).and_then(|v| v.as_array());
        let n = |i: usize| {
            v.and_then(|a| a.get(i))
                .and_then(|x| x.as_f64())
                .unwrap_or(0.0)
        };
        (n(0), n(1))
    };
    let address = client.get("address").and_then(|a| a.as_str()).unwrap_or("");
    Some(Client {
        target: format!("address:{address}"),
        at: pair("at"),
        size: pair("size"),
    })
}

type Done = Box<dyn FnOnce(bool)>;

struct Stepper {
    client: Client,
    to: (f64, f64),
    duration: f64,
    debug: Debug,
    start: i64,
    /// Lua dispatch works; else the classic syntax.
    lua: Cell<bool>,
    /// A step is still on its way to Hyprland.
    busy: Cell<bool>,
    sent: Cell<Option<(i64, i64)>>,
    callback: RefCell<Option<Done>>,
}

impl Stepper {
    fn tick(self: &Rc<Self>) -> glib::ControlFlow {
        if self.busy.get() {
            return glib::ControlFlow::Continue;
        }
        let t = ((glib::monotonic_time() - self.start) as f64 / 1e6 / self.duration).min(1.0);
        let eased = 1.0 - (1.0 - t).powi(3); // ease-out, like the panel's slide
        let (w0, h0) = self.client.size;
        let size = (
            (w0 + (self.to.0 - w0) * eased).round() as i64,
            (h0 + (self.to.1 - h0) * eased).round() as i64,
        );
        if self.sent.get() == Some(size) {
            if t < 1.0 {
                return glib::ControlFlow::Continue;
            }
            self.finish();
            return glib::ControlFlow::Break;
        }
        self.send(size);
        glib::ControlFlow::Continue
    }

    fn send(self: &Rc<Self>, (w, h): (i64, i64)) {
        self.busy.set(true);
        self.sent.set(Some((w, h)));
        let (x, y) = self.client.at;
        let (w0, h0) = self.client.size;
        let nx = (x + (w0 - w as f64) / 2.0).round() as i64;
        let ny = (y + (h0 - h as f64) / 2.0).round() as i64;
        let target = &self.client.target;
        let this = self.clone();
        if self.lua.get() {
            let lua = format!(
                r#"hl.dispatch(hl.dsp.window.resize({{ x = {w}, y = {h}, relative = false, window = "{target}" }})) hl.dispatch(hl.dsp.window.move({{ x = {nx}, y = {ny}, relative = false, window = "{target}" }}))"#
            );
            run(&["hyprctl", "eval", &lua], move |ok, out| {
                let out_lc = out.to_lowercase();
                if ok
                    && !["error", "invalid", "unknown"]
                        .iter()
                        .any(|w| out_lc.contains(w))
                {
                    this.busy.set(false);
                    return;
                }
                (this.debug)(&format!(
                    "hyprctl eval -> {:?}; using the classic syntax",
                    out.trim()
                ));
                this.lua.set(false);
                this.sent.set(None);
                this.send((w, h));
            });
        } else {
            let batch = format!(
                "dispatch resizewindowpixel exact {w} {h},{target} ; dispatch movewindowpixel exact {nx} {ny},{target}"
            );
            run(&["hyprctl", "--batch", &batch], move |ok, out| {
                if !ok {
                    (this.debug)(&format!("hyprctl --batch -> {:?}", out.trim()));
                }
                this.busy.set(false);
            });
        }
    }

    fn finish(&self) {
        let target = self.client.target.clone();
        let debug = self.debug.clone();
        // A moment later, so the last step isn't animated after all.
        glib::timeout_add_local_once(Duration::from_millis(100), move || {
            set_no_anim(&target, "unset", debug, |_| {});
        });
        if let Some(callback) = self.callback.take() {
            callback(true);
        }
    }
}
