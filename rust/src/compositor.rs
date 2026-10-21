//! Compositor-specific window resizing.
//!
//! Port of `calculator/compositor.py`. Some compositors ignore a floating
//! window's own resize requests. Hyprland is one of them, but it can be asked
//! through hyprctl. Everything here is asynchronous and best-effort: on any
//! failure the callback gets false and the window falls back to the generic
//! method.

use std::rc::Rc;

use gtk::gio;

use crate::config::APP_ID;

pub fn is_hyprland() -> bool {
    std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some_and(|v| !v.is_empty())
        && gtk::glib::find_program_in_path("hyprctl").is_some()
}

/// Run a command; callback(ok, stdout).
fn run(argv: &[&str], callback: impl FnOnce(bool, String) + 'static) {
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

type Debug = Rc<dyn Fn(&str)>;

/// Try Hyprland's Lua dispatch (0.55+), then the classic syntax.
fn dispatch(
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
        if !ok {
            return callback(false);
        }
        let Ok(clients) = serde_json::from_str::<Vec<serde_json::Value>>(&out) else {
            return callback(false);
        };
        let pid = u64::from(std::process::id());
        let mine = clients.iter().find(|c| {
            c.get("pid").and_then(|p| p.as_u64()) == Some(pid)
                && c.get("class").and_then(|c| c.as_str()) == Some(APP_ID)
        });
        let Some(client) = mine else {
            debug("hyprctl: window not found in clients");
            return callback(false);
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
        let address = client
            .get("address")
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .to_string();
        let ((x, y), (w, h)) = (pair("at"), pair("size"));
        // Python's round() rounds halves to even.
        let nx = (x + (w - f64::from(width)) / 2.0).round_ties_even() as i64;
        let ny = (y + (h - f64::from(height)) / 2.0).round_ties_even() as i64;
        let target = format!("address:{address}");

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
