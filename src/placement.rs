//! Remembering where the window was.
//!
//! Wayland doesn't let an app place its own window, so this works only where
//! the desktop offers another way: Hyprland (hyprctl), Sway (swaymsg), niri
//! (niri msg, floating windows) and any desktop running on X11. GNOME and
//! KDE Plasma on Wayland place windows themselves.
//!
//! The window's centre is saved rather than its corner, so it lands in the
//! same place whichever mode it opens in. Positions are only restored by the
//! same method that saved them, since each uses its own coordinates.

use std::ffi::{c_char, c_int, c_uint, c_ulong, c_void, CString};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use gtk::{gdk, glib};

use crate::compositor::{self, Client, Debug};
use crate::config::APP_ID;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Hyprland,
    Sway,
    Niri,
    X11,
}

impl Method {
    pub fn key(self) -> &'static str {
        match self {
            Method::Hyprland => "hyprland",
            Method::Sway => "sway",
            Method::Niri => "niri",
            Method::X11 => "x11",
        }
    }
}

fn has_env(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|v| !v.is_empty())
}

/// How this session can place windows, if at all.
pub fn method(window: &impl IsA<gtk::Widget>) -> Option<Method> {
    let program = |name: &str| glib::find_program_in_path(name).is_some();
    // The compositor's own way first: under XWayland it is the one in charge.
    if compositor::is_hyprland() {
        Some(Method::Hyprland)
    } else if has_env("SWAYSOCK") && program("swaymsg") {
        Some(Method::Sway)
    } else if has_env("NIRI_SOCKET") && program("niri") {
        Some(Method::Niri)
    } else if WidgetExt::display(window).type_().name() == "GdkX11Display" {
        Some(Method::X11)
    } else {
        None
    }
}

/// Where the window's centre is now. Blocks briefly: used while the window
/// closes, when there is no time to wait for an answer.
pub fn center(window: &gtk::Window, method: Method, debug: &Debug) -> Option<(i32, i32)> {
    let ((x, y), (w, h)) = match method {
        Method::Hyprland => {
            let out = command_output("hyprctl", &["clients", "-j"])?;
            let client = compositor::find_client(&out, debug)?;
            (client.at, client.size)
        }
        Method::Sway => sway_rect(&command_output("swaymsg", &["-t", "get_tree", "-r"])?)?,
        Method::Niri => {
            let window = niri_window(&command_output("niri", &["msg", "-j", "windows"])?)?;
            (window.pos, window.size)
        }
        Method::X11 => x11::geometry(window)?,
    };
    // Rounded down: rounding the half pixel up drifts by one each time.
    Some(((x + w / 2.0).floor() as i32, (y + h / 2.0).floor() as i32))
}

/// Move the window so its centre is at `center`, if that point is still on
/// a monitor.
pub fn restore(window: &gtk::Window, method: Method, center: (i32, i32), debug: Debug) {
    debug(&format!(
        "moving the window back to centre {},{} ({})",
        center.0,
        center.1,
        method.key()
    ));
    match method {
        Method::Hyprland => restore_hyprland(center, debug),
        Method::Sway => restore_sway(center, debug),
        Method::Niri => restore_niri(center, debug),
        Method::X11 => {
            if !x11::on_screen(window, center) {
                debug("saved position is off every monitor; leaving the window where it opened");
                return;
            }
            x11::move_center(window, center);
        }
    }
}

/// Where the desktop can be told before the window opens (Hyprland, Sway),
/// tell it to open the window with its centre at `center`, so it appears in
/// place instead of being moved once it has appeared. `size` is the size the
/// window will ask for. Blocks briefly; call just before showing the window.
/// False where that isn't possible; then use `restore` once it is open.
pub fn prepare(method: Method, center: (i32, i32), size: (i32, i32), debug: &Debug) -> bool {
    match method {
        Method::Hyprland => prepare_hyprland(center, debug),
        Method::Sway => prepare_sway(center, size, debug),
        Method::Niri | Method::X11 => false,
    }
}

/// After a window `prepare`d for opened: tidy up, and move it after all if
/// the desktop put it somewhere else.
pub fn after_open(window: &gtk::Window, method: Method, center: (i32, i32), debug: Debug) {
    if method == Method::Hyprland {
        release_hyprland(debug.clone());
    }
    let window = window.clone();
    check(method, center, debug.clone(), move |placed| {
        if !placed {
            restore(&window, method, center, debug);
        }
    });
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn json_list(text: &str) -> Vec<serde_json::Value> {
    serde_json::from_str(text).unwrap_or_default()
}

fn number(value: &serde_json::Value, key: &str) -> f64 {
    value.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn pair(value: Option<&serde_json::Value>) -> Option<(f64, f64)> {
    let list = value?.as_array()?;
    Some((list.first()?.as_f64()?, list.get(1)?.as_f64()?))
}

fn inside(point: (i32, i32), (x, y, w, h): (f64, f64, f64, f64)) -> bool {
    let (px, py) = (f64::from(point.0), f64::from(point.1));
    px >= x && px < x + w && py >= y && py < y + h
}

fn own_pid() -> u64 {
    u64::from(std::process::id())
}

// ----------------------------------------------------------- Hyprland

/// The Hyprland monitor that shows `point`: its name and top-left corner.
fn hyprland_monitor_at(monitors: &str, point: (i32, i32)) -> Option<(String, f64, f64)> {
    json_list(monitors).into_iter().find_map(|m| {
        let scale = Some(number(&m, "scale"))
            .filter(|s| *s > 0.0)
            .unwrap_or(1.0);
        // Rotated monitors (odd transforms) swap width and height.
        let (mut w, mut h) = (number(&m, "width") / scale, number(&m, "height") / scale);
        if m.get("transform").and_then(|v| v.as_i64()).unwrap_or(0) % 2 == 1 {
            std::mem::swap(&mut w, &mut h);
        }
        let (x, y) = (number(&m, "x"), number(&m, "y"));
        inside(point, (x, y, w, h))
            .then(|| Some((m.get("name")?.as_str()?.to_string(), x, y)))
            .flatten()
    })
}

/// The Lua global holding this launch's position rule.
fn hyprland_rule_global() -> String {
    format!("calculator_position_{}", own_pid())
}

/// Ask Hyprland, before the window opens, to open it with its centre at
/// `center`: a window rule added at run time, newer than the user's rules,
/// so its `move` wins (Hyprland lets a later `move` cancel an earlier
/// `center = true`, and the other way round; so no `center` here at all). The window then opens in place,
/// with Hyprland's own open animation, instead of being moved once it has
/// appeared. Blocks briefly; call just before showing the window. False if
/// Hyprland doesn't take it (a config without Lua, before 0.55).
fn prepare_hyprland(center: (i32, i32), debug: &Debug) -> bool {
    let Some(monitors) = command_output("hyprctl", &["monitors", "-j"]) else {
        return false;
    };
    let Some((monitor, mx, my)) = hyprland_monitor_at(&monitors, center) else {
        debug("saved position is off every monitor; leaving the window where it opens");
        return true; // nothing to do, and nothing to move later either
    };
    let (x, y) = (f64::from(center.0) - mx, f64::from(center.1) - my);
    let class = APP_ID.replace('.', "\\\\.");
    let lua = format!(
        r#"{global} = hl.window_rule({{ name = "{global}", match = {{ class = "^{class}$", title = "^Calculator$" }}, monitor = "{monitor}", move = {{ "{x}-(window_w*0.5)", "{y}-(window_h*0.5)" }} }})"#,
        global = hyprland_rule_global(),
    );
    let reply = command_output("hyprctl", &["eval", &lua]).unwrap_or_default();
    debug(&format!(
        "hyprctl eval (position rule) -> {:?}",
        reply.trim()
    ));
    reply.trim() == "ok"
}

/// Whether the window's centre is within a few pixels of `center`.
fn check(method: Method, center: (i32, i32), debug: Debug, done: impl FnOnce(bool) + 'static) {
    let found = move |rect: Option<((f64, f64), (f64, f64))>| {
        let Some(((x, y), (w, h))) = rect else {
            return true; // can't tell; leave it
        };
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let close =
            (cx - f64::from(center.0)).abs() <= 4.0 && (cy - f64::from(center.1)).abs() <= 4.0;
        if !close {
            debug(&format!(
                "opened with its centre at {cx},{cy}, not {},{}",
                center.0, center.1
            ));
        }
        close
    };
    match method {
        Method::Hyprland => compositor::run(&["hyprctl", "clients", "-j"], move |ok, out| {
            let debug: Debug = Rc::new(|_: &str| {});
            let client = ok.then(|| compositor::find_client(&out, &debug)).flatten();
            done(found(client.map(|c| (c.at, c.size))));
        }),
        Method::Sway => compositor::run(&["swaymsg", "-t", "get_tree", "-r"], move |ok, out| {
            done(found(ok.then(|| sway_rect(&out)).flatten()));
        }),
        Method::Niri | Method::X11 => done(true),
    }
}

/// Switch this launch's position rule off once the window has opened, so it
/// doesn't place other windows (Preferences) or a later launch.
fn release_hyprland(debug: Debug) {
    let lua = format!(
        "if {g} then {g}:set_enabled(false) end {g} = nil",
        g = hyprland_rule_global()
    );
    compositor::run(&["hyprctl", "eval", &lua], move |_, out| {
        debug(&format!(
            "hyprctl eval (position rule off) -> {:?}",
            out.trim()
        ));
    });
}

fn restore_hyprland(center: (i32, i32), debug: Debug) {
    compositor::run(&["hyprctl", "monitors", "-j"], move |ok, out| {
        let on_screen = ok && hyprland_monitor_at(&out, center).is_some();
        if !on_screen {
            debug("saved position is off every monitor; leaving the window where it opened");
            return;
        }
        compositor::run(&["hyprctl", "clients", "-j"], move |ok, out| {
            let client = ok.then(|| compositor::find_client(&out, &debug)).flatten();
            let Some(Client {
                target,
                size: (w, h),
                ..
            }) = client
            else {
                return;
            };
            let nx = (f64::from(center.0) - w / 2.0).round() as i64;
            let ny = (f64::from(center.1) - h / 2.0).round() as i64;
            let debug2 = debug.clone();
            let target2 = target.clone();
            // Without Hyprland's animation, so it doesn't fly across.
            compositor::set_no_anim(&target, "1", debug.clone(), move |_| {
                compositor::dispatch(
                    format!(
                        r#"hl.dsp.window.move({{ x = {nx}, y = {ny}, relative = false, window = "{target2}" }})"#
                    ),
                    vec![
                        "movewindowpixel".to_string(),
                        format!("exact {nx} {ny},{target2}"),
                    ],
                    debug2.clone(),
                    move |_| {
                        glib::timeout_add_local_once(Duration::from_millis(100), move || {
                            compositor::set_no_anim(&target2, "unset", debug2, |_| {});
                        });
                    },
                );
            });
        });
    });
}

// ----------------------------------------------------------- Sway

/// This app's window in Sway's tree, as (position, size), in layout
/// coordinates across all outputs.
fn sway_rect(tree: &str) -> Option<((f64, f64), (f64, f64))> {
    fn find(node: &serde_json::Value) -> Option<&serde_json::Value> {
        let pid = node.get("pid").and_then(|p| p.as_u64());
        let app_id = node.get("app_id").and_then(|a| a.as_str());
        if pid == Some(own_pid()) && app_id == Some(APP_ID) {
            return Some(node);
        }
        ["nodes", "floating_nodes"]
            .iter()
            .filter_map(|key| node.get(*key).and_then(|n| n.as_array()))
            .flatten()
            .find_map(find)
    }
    let tree: serde_json::Value = serde_json::from_str(tree).ok()?;
    let rect = find(&tree)?.get("rect")?;
    Some((
        (number(rect, "x"), number(rect, "y")),
        (number(rect, "width"), number(rect, "height")),
    ))
}

/// The output rectangles, in Sway's layout coordinates.
fn sway_outputs(outputs: &str) -> Vec<(f64, f64, f64, f64)> {
    json_list(outputs)
        .iter()
        .filter_map(|o| o.get("rect"))
        .map(|r| {
            (
                number(r, "x"),
                number(r, "y"),
                number(r, "width"),
                number(r, "height"),
            )
        })
        .collect()
}

/// A run-time `for_window` rule for this process's calculator window: Sway
/// runs it as the window opens, before it is drawn. Sway can't remove such a
/// rule, so it names this process, which no later launch will be.
fn prepare_sway(center: (i32, i32), size: (i32, i32), debug: &Debug) -> bool {
    let Some(outputs) = command_output("swaymsg", &["-t", "get_outputs", "-r"]) else {
        return false;
    };
    if !sway_outputs(&outputs)
        .into_iter()
        .any(|r| inside(center, r))
    {
        debug("saved position is off every output; leaving the window where it opens");
        return true;
    }
    let x = center.0 - size.0 / 2;
    let y = center.1 - size.1 / 2;
    let class = APP_ID.replace('.', "\\.");
    let command = format!(
        r#"for_window [app_id="^{class}$" pid={} title="^Calculator$"] move absolute position {x} {y}"#,
        own_pid()
    );
    let ok = Command::new("swaymsg")
        .arg(&command)
        .output()
        .is_ok_and(|out| out.status.success());
    debug(&format!("swaymsg for_window (position) -> {ok}"));
    ok
}

fn restore_sway(center: (i32, i32), debug: Debug) {
    compositor::run(&["swaymsg", "-t", "get_outputs", "-r"], move |ok, out| {
        let on_screen = ok && sway_outputs(&out).into_iter().any(|r| inside(center, r));
        if !on_screen {
            debug("saved position is off every output; leaving the window where it opened");
            return;
        }
        compositor::run(&["swaymsg", "-t", "get_tree", "-r"], move |ok, out| {
            let Some((_, (w, h))) = ok.then(|| sway_rect(&out)).flatten() else {
                return;
            };
            let nx = (f64::from(center.0) - w / 2.0).round() as i64;
            let ny = (f64::from(center.1) - h / 2.0).round() as i64;
            let command = format!("[pid={}] move absolute position {nx} {ny}", own_pid());
            compositor::run(&["swaymsg", &command], move |ok, out| {
                if !ok {
                    debug(&format!("swaymsg -> {:?}", out.trim()));
                }
            });
        });
    });
}

// ----------------------------------------------------------- niri

struct NiriWindow {
    id: u64,
    /// Position in the workspace view, and size, of the window's tile.
    pos: (f64, f64),
    size: (f64, f64),
}

/// This app's window, if it floats and niri reports where it is (niri 25.08
/// or newer).
fn niri_window(windows: &str) -> Option<NiriWindow> {
    let windows = json_list(windows);
    let window = windows.iter().find(|w| {
        w.get("pid").and_then(|p| p.as_u64()) == Some(own_pid())
            && w.get("app_id").and_then(|a| a.as_str()) == Some(APP_ID)
            && w.get("is_floating").and_then(|f| f.as_bool()) == Some(true)
    })?;
    let layout = window.get("layout")?;
    Some(NiriWindow {
        id: window.get("id")?.as_u64()?,
        pos: pair(layout.get("tile_pos_in_workspace_view"))?,
        size: pair(layout.get("tile_size"))?,
    })
}

fn restore_niri(center: (i32, i32), debug: Debug) {
    compositor::run(&["niri", "msg", "-j", "windows"], move |ok, out| {
        let Some(window) = ok.then(|| niri_window(&out)).flatten() else {
            debug("niri: no floating window with a known position");
            return;
        };
        // Moved by the difference, so it doesn't matter what niri measures
        // absolute positions from. Both points are in the workspace view,
        // which is always on screen.
        let now = (
            window.pos.0 + window.size.0 / 2.0,
            window.pos.1 + window.size.1 / 2.0,
        );
        let dx = (f64::from(center.0) - now.0).round() as i64;
        let dy = (f64::from(center.1) - now.1).round() as i64;
        if dx == 0 && dy == 0 {
            return;
        }
        let id = window.id.to_string();
        let (dx, dy) = (format!("{dx:+}"), format!("{dy:+}"));
        let argv = [
            "niri",
            "msg",
            "action",
            "move-floating-window",
            "--id",
            &id,
            "-x",
            &dx,
            "-y",
            &dy,
        ];
        compositor::run(&argv, move |ok, out| {
            if !ok {
                debug(&format!("niri msg -> {:?}", out.trim()));
            }
        });
    });
}

// ----------------------------------------------------------- X11

/// Xlib, looked up in the running process: it is loaded whenever GTK runs on
/// X11, and needn't be linked otherwise.
mod x11 {
    use super::*;

    type XDisplay = c_void;
    type XWindow = c_ulong;

    extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    /// RTLD_DEFAULT: search every loaded library.
    const RTLD_DEFAULT: *mut c_void = std::ptr::null_mut();

    fn symbol(name: &str) -> Option<*mut c_void> {
        let name = CString::new(name).ok()?;
        // SAFETY: dlsym only reads the NUL-terminated name.
        let found = unsafe { dlsym(RTLD_DEFAULT, name.as_ptr()) };
        (!found.is_null()).then_some(found)
    }

    macro_rules! xfn {
        ($name:literal: fn($($arg:ty),*) -> $ret:ty) => {
            // SAFETY: the symbol has this C signature in libgtk-4 or libX11.
            symbol($name).map(|f| unsafe {
                std::mem::transmute::<*mut c_void, unsafe extern "C" fn($($arg),*) -> $ret>(f)
            })
        };
    }

    struct Handles {
        display: *mut XDisplay,
        window: XWindow,
    }

    fn handles(window: &gtk::Window) -> Option<Handles> {
        let surface = window.surface()?;
        let gdk_display = WidgetExt::display(window);
        let get_xid = xfn!("gdk_x11_surface_get_xid": fn(*mut gdk::ffi::GdkSurface) -> XWindow)?;
        let get_xdisplay =
            xfn!("gdk_x11_display_get_xdisplay": fn(*mut gdk::ffi::GdkDisplay) -> *mut XDisplay)?;
        // SAFETY: both are live GDK X11 objects, borrowed for the calls.
        let (window, display) = unsafe {
            (
                get_xid(surface.to_glib_none().0),
                get_xdisplay(gdk_display.to_glib_none().0),
            )
        };
        (window != 0 && !display.is_null()).then_some(Handles { display, window })
    }

    /// The window's position on the root window, and size.
    pub fn geometry(window: &gtk::Window) -> Option<((f64, f64), (f64, f64))> {
        let h = handles(window)?;
        let root_of = xfn!("XDefaultRootWindow": fn(*mut XDisplay) -> XWindow)?;
        let translate = xfn!("XTranslateCoordinates": fn(*mut XDisplay, XWindow, XWindow, c_int, c_int, *mut c_int, *mut c_int, *mut XWindow) -> c_int)?;
        let get_geometry = xfn!("XGetGeometry": fn(*mut XDisplay, XWindow, *mut XWindow, *mut c_int, *mut c_int, *mut c_uint, *mut c_uint, *mut c_uint, *mut c_uint) -> c_int)?;
        let (mut x, mut y, mut child) = (0, 0, 0);
        let (mut root, mut gx, mut gy) = (0, 0, 0);
        let (mut w, mut hgt, mut border, mut depth) = (0, 0, 0, 0);
        // SAFETY: valid display and window; every out-pointer is a local.
        let ok = unsafe {
            let root_window = root_of(h.display);
            translate(
                h.display,
                h.window,
                root_window,
                0,
                0,
                &mut x,
                &mut y,
                &mut child,
            ) != 0
                && get_geometry(
                    h.display,
                    h.window,
                    &mut root,
                    &mut gx,
                    &mut gy,
                    &mut w,
                    &mut hgt,
                    &mut border,
                    &mut depth,
                ) != 0
        };
        ok.then(|| ((f64::from(x), f64::from(y)), (f64::from(w), f64::from(hgt))))
    }

    pub fn on_screen(window: &gtk::Window, center: (i32, i32)) -> bool {
        let monitors = WidgetExt::display(window).monitors();
        (0..monitors.n_items())
            .filter_map(|i| monitors.item(i).and_downcast::<gdk::Monitor>())
            .any(|m| {
                let g = m.geometry();
                inside(
                    center,
                    (
                        f64::from(g.x()),
                        f64::from(g.y()),
                        f64::from(g.width()),
                        f64::from(g.height()),
                    ),
                )
            })
    }

    pub fn move_center(window: &gtk::Window, center: (i32, i32)) {
        let Some(((_, _), (w, h))) = geometry(window) else {
            return;
        };
        let Some(handles) = handles(window) else {
            return;
        };
        let (Some(move_window), Some(flush)) = (
            xfn!("XMoveWindow": fn(*mut XDisplay, XWindow, c_int, c_int) -> c_int),
            xfn!("XFlush": fn(*mut XDisplay) -> c_int),
        ) else {
            return;
        };
        let x = (f64::from(center.0) - w / 2.0).round() as c_int;
        let y = (f64::from(center.1) - h / 2.0).round() as c_int;
        // SAFETY: valid display and window.
        unsafe {
            move_window(handles.display, handles.window, x, y);
            flush(handles.display);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_window_in_a_sway_tree() {
        let tree = format!(
            r#"{{"nodes": [{{"nodes": [], "floating_nodes": [
                {{"pid": {}, "app_id": "{APP_ID}", "rect": {{"x": 100, "y": 50, "width": 220, "height": 330}}}}
            ]}}]}}"#,
            own_pid()
        );
        assert_eq!(sway_rect(&tree), Some(((100.0, 50.0), (220.0, 330.0))));
        assert_eq!(sway_rect(r#"{"nodes": []}"#), None);
    }

    #[test]
    fn finds_a_floating_niri_window() {
        let windows = format!(
            r#"[{{"id": 7, "pid": {}, "app_id": "{APP_ID}", "is_floating": true,
                 "layout": {{"tile_pos_in_workspace_view": [40.0, 60.0], "tile_size": [220.0, 330.0]}}}}]"#,
            own_pid()
        );
        let window = niri_window(&windows).unwrap();
        assert_eq!(
            (window.id, window.pos, window.size),
            (7, (40.0, 60.0), (220.0, 330.0))
        );
        // Tiled, or an older niri without positions: nothing to restore.
        assert!(niri_window(&windows.replace("true", "false")).is_none());
        assert!(niri_window(&format!(
            r#"[{{"id": 7, "pid": {}, "app_id": "{APP_ID}", "is_floating": true}}]"#,
            own_pid()
        ))
        .is_none());
    }
}
