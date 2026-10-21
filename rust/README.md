# Calculator in Rust

A Rust port of Calculator, built alongside the Python app. It does not replace
the Python version yet; the installer and releases still use Python.

## Status

| Stage | Contents | State |
| --- | --- | --- |
| 1. Core | Expression engine and key handling (`src/engine.rs`, `src/logic.rs`) | Done |
| 2. Usable app | The GTK 4 window: standard and scientific keys, live preview, history, keyboard, clipboard, animations, Hyprland resizing, settings, window sizes | Done |
| 3. Full match | Desktop accent colours and the XDG portal, Material 3 palette generation, presets, `palette.toml`, contrast, the Preferences window, themed app icon, installer and releases | Planned |

The core is a line-by-line port of `calculator/engine.py` and `calculator/logic.py`:
same results, same display text, same error messages. The window is a port of
`calculator/window.py` and looks the same: with the same settings, screenshots
of the two versions match pixel for pixel.

What stage 2 does not have yet:

- Colours are always the default palette (seed `#6750A4`), as if the colour
  source were the Lavender preset. Light or dark follows the Theme setting, or
  the desktop's GSettings and GTK theme; the desktop portal is not read yet.
- The Preferences button only explains that Preferences come later.

It reads and writes the same files as the Python app (`settings.toml`,
`window.json`, `history.json`), so both share settings and history. While both
versions exist, the Rust one starts as a separate app even when the Python one
is open.

## Try it

You need Rust 1.85 or newer and GTK 4 with its development files
(Arch: `sudo pacman -S rust gtk4`; Debian, Ubuntu: `sudo apt install cargo libgtk-4-dev`;
Fedora: `sudo dnf install cargo gtk4-devel`).

```sh
cd rust
cargo run --release
```

The icons come from the font in `data/fonts`, found through the source tree or
an installed Python version.

Every push also builds the program on GitHub: open the latest run of the
**Rust** workflow under Actions and download `calculator-rust-linux-x86_64`.
It is built on GitHub's Ubuntu runner, so it runs on systems at least that
new, such as current Arch. Make it executable (`chmod +x calculator`) after
unzipping.

## Development

```sh
cd rust
cargo test                       # unit tests, including the ported Python tests
cargo clippy --all-targets
python3 scripts/parity.py        # compare the core with the Python app on random input
```

`scripts/parity.py` generates random expressions, numbers to format and key
sequences, runs them through both versions and lists every difference. It
compares results bit for bit. Run it with a larger count or another seed for a
deeper check: `python3 scripts/parity.py 100000 42`.

For screenshots and timing, the app has two development switches:

- `CALCULATOR_SCREENSHOT=out.png CALCULATOR_KEYS="digit:7 op:× digit:6 eq:"`
  runs the steps, saves the window as a PNG and quits. Steps are key actions,
  or `ui:mode`, `ui:inv`, `ui:angle`, `ui:history` and `ui:wait` (one second).
- `CALCULATOR_QUIT_ON_FIRST_FRAME=1` quits as soon as the window is drawn, to
  time start-up. On a test machine the Rust version reached its first frame in
  about 170 ms, the Python version in about 400 ms.

`CALCULATOR_DEBUG=1` prints mode switches and window sizes, as in the Python app.

One known difference: the Python app stops at its recursion limit (about 160
nested brackets) with "Expression too deep"; the Rust port allows up to 500.

| File | Purpose |
| --- | --- |
| `src/engine.rs` | Expression parser and number formatting |
| `src/logic.rs` | Key handling |
| `src/main.rs` | Application start-up and development switches |
| `src/window.rs` | The calculator window |
| `src/motion.rs` | Spring animations, key shapes and press feedback |
| `src/palette.rs` | Colours and the stylesheet (`calculator/style.css`, shared) |
| `src/icons.rs` | Material Symbols icons |
| `src/config.rs` | Settings, window sizes and history files |
| `src/compositor.rs` | Hyprland window resizing |
| `tests/` | Ported unit tests |
| `examples/parity.rs`, `scripts/parity.py` | Comparison with the Python app |
