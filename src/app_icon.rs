//! App icon that follows the user's colours.
//!
//! Launchers can't restyle an app icon, so the app writes its own icon,
//! coloured from the current palette, to the user's icon folder and rewrites
//! it when the colours change. The shipped icon is the fallback until the app
//! first runs.
//!
//! Icon: "calculator" from Material Design Icons by Pictogrammers, Apache 2.0.

use std::fs;
use std::path::{Path, PathBuf};

use calculator::colors::{mix, Roles};
use gtk::gio;

use crate::config::{data_dirs, APP_ID};

const PATH_D: &str = concat!(
    "M7,2H17A2,2 0 0,1 19,4V20A2,2 0 0,1 17,22H7A2,2 0 0,1 5,20V4A2,2 0 0,1 7,2",
    "M7,4V8H17V4H7M7,10V12H9V10H7M11,10V12H13V10H11M15,10V12H17V10H15",
    "M7,14V16H9V14H7M11,14V16H13V14H11M15,14V16H17V14H15",
    "M7,18V20H9V18H7M11,18V20H13V18H11M15,18V20H17V18H15Z",
);

const DEFAULT_COLORS: (&str, &str) = ("#8f7ae0", "#5a3fa8");

pub fn render(top: &str, bottom: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"128\" height=\"128\" viewBox=\"0 0 24 24\">\n  \
         <!-- \"calculator\" from Material Design Icons (Pictogrammers), Apache License 2.0 -->\n  \
         <defs><linearGradient id=\"g\" x1=\"0\" y1=\"2\" x2=\"0\" y2=\"22\" gradientUnits=\"userSpaceOnUse\">\
         <stop offset=\"0\" stop-color=\"{top}\"/><stop offset=\"1\" stop-color=\"{bottom}\"/>\
         </linearGradient></defs>\n  <path fill=\"url(#g)\" d=\"{PATH_D}\"/>\n</svg>\n"
    )
}

/// A gentle top-to-bottom gradient from the palette's primary colour.
pub fn colors_for(roles: &Roles) -> (String, String) {
    let primary = roles.get("primary").unwrap_or(DEFAULT_COLORS.1);
    (mix(primary, "#ffffff", 0.18), mix(primary, "#000000", 0.12))
}

fn icons_dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute());
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    data.unwrap_or_else(|| home.join(".local/share"))
        .join("icons/hicolor")
}

/// Only manage the icon for an installed app, not a source checkout.
fn is_installed() -> bool {
    data_dirs().iter().skip(1).any(|d| {
        d.parent().is_some_and(|p| {
            p.join("applications")
                .join(format!("{APP_ID}.desktop"))
                .is_file()
        })
    })
}

/// Write the icon if it changed. Safe to call on every theme change.
pub fn update(roles: &Roles, themed: bool) {
    if !is_installed() {
        return;
    }
    let (top, bottom) = if themed {
        colors_for(roles)
    } else {
        (DEFAULT_COLORS.0.to_string(), DEFAULT_COLORS.1.to_string())
    };
    let target = icons_dir()
        .join("scalable/apps")
        .join(format!("{APP_ID}.svg"));
    let svg = render(&top, &bottom);
    if fs::read_to_string(&target).is_ok_and(|current| current == svg) {
        return;
    }
    let written = (|| {
        fs::create_dir_all(target.parent().unwrap())?;
        let tmp = target.with_extension("tmp");
        fs::write(&tmp, &svg)?;
        fs::rename(&tmp, &target)
    })();
    if let Err(err) = written {
        eprintln!("calculator: could not update the app icon: {err}");
        return;
    }
    refresh_cache(&icons_dir());
}

fn refresh_cache(theme_dir: &Path) {
    if !theme_dir.join("icon-theme.cache").exists() {
        return; // no cache: icon lookups read the folder directly
    }
    let tool = ["gtk4-update-icon-cache", "gtk-update-icon-cache"]
        .into_iter()
        .find_map(gtk::glib::find_program_in_path);
    if let Some(tool) = tool {
        let args = [
            tool.as_os_str(),
            "-q".as_ref(),
            "-t".as_ref(),
            "-f".as_ref(),
            theme_dir.as_os_str(),
        ];
        let _ = gio::Subprocess::newv(&args, gio::SubprocessFlags::NONE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_icon_matches_the_shipped_svg() {
        let shipped = include_str!(
            "../data/icons/hicolor/scalable/apps/io.github.HypestHyper.Calculator.svg"
        );
        assert_eq!(render(DEFAULT_COLORS.0, DEFAULT_COLORS.1), shipped);
    }
}
