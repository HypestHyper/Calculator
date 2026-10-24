# Changelog

## Unreleased

### Fixed

- No focus outline around the display, and along the window's edges, after typing or after opening the app again while it is already running (for example with Enter in a launcher).

### Changed

- Faster start with large icon themes such as Papirus: the window no longer waits for the icon theme to load. Its minimize, maximize and close buttons are drawn with the app's own icon font, like its other icons, and follow the desktop's button layout as before. On a test machine with Papirus-Dark this saved about 300 ms.
- Faster start: GTK's GL renderer is used instead of the Vulkan one that newer GTK versions default to. On a test with Hyprland it started about 115 ms sooner. Set `GSK_RENDERER` to choose another.
- Faster start with a long history: its rows are built after the window is shown, instead of before. With a full history (100 entries) this saved 30 to 70 ms.
- The window opens smaller by default: 220×330 in standard mode, 396×330 in scientific mode.

### Added

- The window opens where it was when last closed, on Hyprland, Sway, niri (floating windows; niri 25.08 or newer) and any desktop running on X11. On Hyprland with a Lua config (0.55 or newer) it opens straight there, with Hyprland's own open animation, instead of opening in the middle and then moving. A position that is no longer on any monitor is ignored. On GNOME and KDE Plasma under Wayland the desktop decides where windows open, so the app can't.

## 2.0.2 — 2026-10-24

### Fixed

- On Hyprland, the window size you set is remembered, between modes and after a restart. GTK stops following a window's size while the compositor reports it as tiled, and Hyprland reports floating windows as tiled, so the app kept saving its first size. It now reads the window's real size there.

## 2.0.1 — 2026-10-24

### Fixed

- Switching modes keeps the size you set. Opening scientific mode from a small window gives a scientific window of the same height, wider in the same proportion as the defaults; closing it goes back to the matching standard width. Before, each mode had its own remembered size, so a size set in one mode was lost in the other.
- A small window no longer jumps back to full size after closing scientific mode.
- The scientific keys are scaled down with the rest when the panel opens in a small window.
- The window size is also saved when the app is stopped by logging out or shutting down, not only when its window is closed.

## 2.0.0 — 2026-10-22

Calculator is now written in Rust only. The Python version is gone; the Rust one looks and works the same, keeps your settings and history, and starts nearly twice as fast.

To update a clone, run `git pull` and `./install.sh` in its folder. The installer needs Rust 1.88 or newer and the GTK 4 development files (`sudo pacman -S rust gtk4` on Arch). Without Rust, use the release download: it includes a ready-built program. `--rust` is still accepted and no longer needed.

### Added

- The window can be made much smaller: below its normal size, the keys, text and spacing scale down to fit, to about half size, and back up when the window is made larger again.

### Changed

- On Hyprland, leaving scientific mode shrinks the window in small steps, so the window and its contents shrink together, as smoothly as they grow. The keys stay in place until the window has reached its new size.
- Release downloads include a ready-built program for x86-64 Linux.

### Removed

- The Python version, its tests and the tools that compared the two versions.

## 1.0.5 — 2026-10-22

### Fixed

- Closing scientific mode: the scientific keys no longer stretch wider and wider as the panel slides out. They keep their size and fade away.
- The window shrinks with Hyprland's animation again, as smoothly as it grows. 1.0.4 had turned that animation off, which made the shrink abrupt.

## 1.0.4 — 2026-10-22

### Fixed

- On Hyprland, closing scientific mode no longer looks stretched. Hyprland animates a window's size change by drawing its contents scaled, so the app now turns that animation off for its own window while it shrinks, and back on right after.
- The minimize, maximize and close buttons show a single round shape. Before, many GTK themes' own circle and outline appeared inside a second pill, and a focus ring could appear after typing.

## 1.0.3 — 2026-10-21

### Added

- A Rust version of the app, as an option: `./install.sh --rust` builds and installs it in place of the Python version (needs Rust 1.88 or newer and the GTK 4 development files). It looks and works the same, keeps your settings and history, and starts nearly twice as fast. Run `./install.sh` again to go back. See [rust/README.md](rust/README.md).

### Fixed

- Closing scientific mode is now as smooth as opening it. The standard keys no longer stretch into the space the scientific panel leaves; they keep their size, and the scientific keys fade out as the panel slides away.

## 1.0.2 — 2026-10-19

### Fixed

- The app now opens from app launchers (such as Noctalia, Rofi or Fuzzel) that do not have `~/.local/bin` in their `PATH`. Before, pressing Enter on Calculator did nothing. The installer now writes the full path of the program into the menu entry; to get the fix, run `git pull` and then `./install.sh` again in your Calculator folder, or install from the new download.

## 1.0.1 — 2026-10-16

### Added

- Decimal separator setting (Preferences → Display): point or comma. It is used for the display, history, the decimal key, copying and pasting.

### Fixed

- Pressing `=` again after a number in E notation, such as `2E−5`, no longer subtracts the exponent.
- Pressing `=` again after multiplying by a negative number keeps the multiplication (`5×−3 = =` gives 45).
- A leading minus (`−5 =`) is no longer repeated as a subtraction.
- Results reused in a new calculation keep their full precision, so `1÷3 =` then `×3 =` gives exactly 1.
- Pasting keeps `x` inside `exp`, accepts `Ans` and `mod`, and reads `1,000` and `1,234.5` with thousands separators.
- `tan` of an odd multiple of π/2 is reported as undefined in radians, as it already was in degrees.
- Fixed decimal places no longer show `−0.00` for small negative numbers.
- Backspace removes `exp(` in one step.

## 1.0.0 — 2026-10-15

- First release.
