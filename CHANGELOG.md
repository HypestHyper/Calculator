# Changelog

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
