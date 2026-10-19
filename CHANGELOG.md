# Changelog

## 1.0.2 — 2026-10-19

### Fixed

- The app now opens from app launchers (such as Noctalia, Rofi or Fuzzel) that do not have `~/.local/bin` in their `PATH`. Before, pressing Enter on Calculator did nothing. The installer now writes the full path of the program into the menu entry; run `./install.sh` again to get the fix.

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
