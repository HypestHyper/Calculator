//! Material Symbols icons.
//!
//! Port of `calculator/icons.py`. The Material Symbols Rounded variable font
//! (Apache-2.0) ships with the app and is registered privately with
//! fontconfig, so nothing is installed system wide. If the font cannot be
//! loaded, icons fall back to symbolic theme icons.

use std::cell::Cell;
use std::ffi::{c_char, c_int, c_void, CString};

use gtk::pango::prelude::*;
use gtk::prelude::*;

use crate::config::data_dirs;

const FONT_FAMILY: &str = "Material Symbols Rounded";
const FONT_FILE: &str = "MaterialSymbolsRounded.ttf";

#[link(name = "fontconfig")]
extern "C" {
    fn FcConfigAppFontAddFile(config: *mut c_void, file: *const c_char) -> c_int;
}

/// name -> (codepoint, fallback symbolic icon)
fn lookup(name: &str) -> (u32, &'static str) {
    match name {
        "backspace" => (0xE14A, "edit-clear-symbolic"),
        "history" => (0xE8B3, "document-open-recent-symbolic"),
        "settings" => (0xE8B8, "emblem-system-symbolic"),
        "calculate" => (0xEA5F, "accessories-calculator-symbolic"),
        "function" => (0xF866, "view-more-symbolic"),
        "science" => (0xEA4B, "view-more-symbolic"),
        "content_copy" => (0xE14D, "edit-copy-symbolic"),
        "delete_sweep" => (0xE16C, "user-trash-symbolic"),
        "close" => (0xE5CD, "window-close-symbolic"),
        "check" => (0xE668, "object-select-symbolic"),
        "palette" => (0xE40A, "applications-graphics-symbolic"),
        "history_toggle_off" => (0xF17D, "document-open-recent-symbolic"),
        "dark_mode" => (0xE51C, "weather-clear-night-symbolic"),
        "light_mode" => (0xE518, "weather-clear-symbolic"),
        "contrast" => (0xEB37, "display-brightness-symbolic"),
        "animation" => (0xE71C, "media-playback-start-symbolic"),
        "tune" => (0xE429, "preferences-system-symbolic"),
        "info" => (0xE88E, "help-about-symbolic"),
        "desktop_windows" => (0xE30C, "video-display-symbolic"),
        "open_in_new" => (0xE89E, "document-open-symbolic"),
        "text_fields" => (0xE262, "format-text-bold-symbolic"),
        "grid_view" => (0xE9B0, "view-grid-symbolic"),
        "square_foot" => (0xEA49, "view-grid-symbolic"),
        "replay" => (0xE042, "view-refresh-symbolic"),
        "keyboard" => (0xE312, "input-keyboard-symbolic"),
        "drag_indicator" => (0xE945, "list-drag-handle-symbolic"),
        _ => {
            eprintln!("calculator: unknown icon {name}");
            (0xE88E, "image-missing-symbolic")
        }
    }
}

thread_local! {
    static FONT_OK: Cell<bool> = const { Cell::new(false) };
}

/// Register the bundled font with fontconfig. Call before any text is drawn.
pub fn load_font() -> bool {
    let path = data_dirs()
        .into_iter()
        .map(|d| d.join("fonts").join(FONT_FILE))
        .find(|p| p.is_file());
    if let Some(path) = path {
        if let Ok(c_path) = CString::new(path.to_string_lossy().as_bytes()) {
            // SAFETY: a null config means fontconfig's current configuration,
            // and the path is a valid NUL-terminated string for the call.
            unsafe {
                FcConfigAppFontAddFile(std::ptr::null_mut(), c_path.as_ptr());
            }
        }
    }
    let ok = family_available();
    FONT_OK.with(|f| f.set(ok));
    ok
}

fn family_available() -> bool {
    let context = gtk::Label::new(None).pango_context();
    context
        .font_map()
        .is_some_and(|map| map.list_families().iter().any(|f| f.name() == FONT_FAMILY))
}

/// A widget showing the named Material Symbol.
pub fn icon(name: &str, size: i32) -> gtk::Widget {
    let (codepoint, fallback) = lookup(name);
    let widget: gtk::Widget = if FONT_OK.with(Cell::get) {
        let label = gtk::Label::new(Some(&char::from_u32(codepoint).unwrap().to_string()));
        label.add_css_class("msym");
        label.upcast()
    } else {
        let image = gtk::Image::from_icon_name(fallback);
        image.set_pixel_size(size);
        image.upcast()
    };
    widget.set_can_target(false);
    widget
}

pub fn icon_button(name: &str, tooltip: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(&icon(name, 20)));
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("icon-btn");
    button
}
