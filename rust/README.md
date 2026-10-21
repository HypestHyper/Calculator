# Calculator in Rust

A Rust port of Calculator, built alongside the Python app. It does not replace
the Python version yet; the installer and releases still use Python.

## Status

| Stage | Contents | State |
| --- | --- | --- |
| 1. Core | Expression engine and key handling (`src/engine.rs`, `src/logic.rs`) | Done |
| 2. Usable app | GTK 4 window: keys, display, live preview, history, keyboard, clipboard, settings file | Planned |
| 3. Full match | Desktop colours, Material 3 palettes and presets, Preferences, animations, Hyprland resizing, themed icon, installer | Planned |

The core is a line-by-line port of `calculator/engine.py` and `calculator/logic.py`:
same results, same display text, same error messages.

## Development

```sh
cd rust
cargo test                       # the Python tests, ported
cargo clippy --all-targets
python3 scripts/parity.py        # compare with the Python app on random input
```

`scripts/parity.py` generates random expressions, numbers to format and key
sequences, runs them through both versions and lists every difference. It
compares results bit for bit. Run it with a larger count or another seed for a
deeper check: `python3 scripts/parity.py 100000 42`.

One known difference: the Python app stops at its recursion limit (about 160
nested brackets) with "Expression too deep"; the Rust port allows up to 500.

| File | Purpose |
| --- | --- |
| `src/engine.rs` | Expression parser and number formatting |
| `src/logic.rs` | Key handling |
| `tests/` | Ported unit tests |
| `examples/parity.rs`, `scripts/parity.py` | Comparison with the Python app |
