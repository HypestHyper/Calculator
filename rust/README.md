# Calculator in Rust

A complete Rust port of Calculator. It looks and behaves like the Python app,
reads and writes the same settings, palette and history files, and starts
nearly twice as fast. The Python version is still the default; the Rust one is
opt-in.

## Install

You need Rust 1.88 or newer and GTK 4 (4.10 or newer) with its development
files:

| Distribution | Command |
| --- | --- |
| Arch, CachyOS, Manjaro | `sudo pacman -S rust gtk4` |
| Debian, Ubuntu | `sudo apt install cargo libgtk-4-dev` |
| Fedora | `sudo dnf install cargo gtk4-devel` |

Then, from the repository folder:

```sh
./install.sh --rust          # builds and installs to ~/.local
```

It replaces the Python version's launcher, so your app launcher starts the Rust
version; settings and history are kept. `./install.sh` (without `--rust`) goes
back to Python, and `./uninstall.sh` removes either. For a system-wide install,
build first as yourself (`cargo build --release` in this folder), then run
`sudo ./install.sh --rust --system`.

To run it without installing: `cargo run --release` in this folder.

Every push also builds the program on GitHub: open the latest run of the
**Rust** workflow under Actions and download `calculator-rust-linux-x86_64`.
It is built on GitHub's Ubuntu runner, so it runs on systems at least that new,
such as current Arch. Make it executable (`chmod +x calculator`) after
unzipping. It finds its icon font through an installed copy of Calculator.

## How it matches the Python app

The port is checked against the Python app, not just by its own tests:

- **Calculations.** `src/engine.rs` and `src/logic.rs` are line-by-line ports
  of `engine.py` and `logic.py`. `scripts/parity.py` runs random expressions,
  number formats and key sequences through both and compares results bit for
  bit, display text and error messages included.
- **Colours.** Palettes come from the material-colors crate with the 2025
  colour specification, vendored in `vendor/material-colors` with two small
  fixes (see its README) so they equal what the Python app gets from
  materialyoucolor. `scripts/palette_parity.py` compares every colour role for
  many seeds, all nine styles, light and dark and each contrast level, plus the
  presets, the palette.toml template and palette.toml parsing.
- **Looks.** The window and Preferences use the Python app's own
  `calculator/style.css`. With the same settings, screenshots of both versions
  were compared pixel for pixel: standard and scientific mode, dark and light,
  desktop accent, presets, custom colours with contrast, palette.toml (also
  edited while running), settings changed live, the history sheet, errors and
  every part of Preferences. The colours from the XDG desktop portal, and
  changes to them while running, were tested with a stand-in portal.

Known differences:

- Preferences → Active colours says the palette comes from "material-colors"
  instead of "materialyoucolor".
- A pure black custom colour (`#000000`) with the Fidelity or Content style
  gives white instead of grey text on the primary and tertiary containers.
- Very deep expressions: the Python app stops at about 160 nested brackets
  with "Expression too deep", the Rust version at 500.
- Development builds (`cargo run` without `--release`) start as a separate
  app even when Calculator is already open, so they can run next to it.

## Development

```sh
cargo test                       # unit tests, including the ported Python tests
cargo clippy --all-targets
python3 scripts/parity.py        # calculations vs the Python app
python3 scripts/palette_parity.py   # colours vs the Python app (needs PyGObject and materialyoucolor)
```

Run the parity scripts with a larger count for a deeper check, for example
`python3 scripts/parity.py 100000 42`.

The app has development switches for screenshots and timing:

- `CALCULATOR_SCREENSHOT=out.png CALCULATOR_KEYS="digit:7 op:× digit:6 eq:"`
  runs the steps, saves the window as a PNG and quits. Steps are key actions
  or `ui:mode`, `ui:inv`, `ui:angle`, `ui:history`, `ui:prefs` (the screenshot
  is then of Preferences), `ui:scroll:N` (scroll Preferences), `ui:set:key=value`
  (change a setting) and `ui:wait` (one second).
- `CALCULATOR_QUIT_ON_FIRST_FRAME=1` quits as soon as the window is drawn, to
  time start-up. On a test machine the Rust version reached its first frame in
  about 220 ms, the Python version in about 400 ms.
- `CALCULATOR_DEBUG=1` prints mode switches and window sizes, as in the Python
  app.

| File | Purpose | Python original |
| --- | --- | --- |
| `src/engine.rs` | Expression parser and number formatting | `engine.py` |
| `src/logic.rs` | Key handling | `logic.py` |
| `src/colors.rs` | Palette generation, presets, palette.toml | `theme.py`, `presets.py` |
| `src/main.rs` | Start-up, command-line options, development switches | `app.py` |
| `src/window.rs` | The calculator window | `window.py` |
| `src/preferences.rs` | Preferences | `preferences.py` |
| `src/theme.rs` | Desktop colours (portal, GSettings, GTK) and CSS | `theme.py` |
| `src/motion.rs` | Spring animations, key shapes and press feedback | `motion.py` |
| `src/icons.rs` | Material Symbols icons | `icons.py` |
| `src/config.rs` | Settings, window sizes and history files | `config.py` |
| `src/compositor.rs` | Hyprland window resizing | `compositor.py` |
| `src/window_rules.rs` | Floating-window rules | `window_rules.py` |
| `src/app_icon.rs` | Palette-coloured launcher icon | `app_icon.py` |
| `vendor/material-colors` | Material colour library (MIT or Apache 2.0) | materialyoucolor |
| `tests/` | Ported unit tests | `tests/` |
| `examples/parity.rs`, `scripts/` | Comparison with the Python app | |
