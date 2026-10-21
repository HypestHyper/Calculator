# material-colors (vendored)

A copy of [material-colors](https://crates.io/crates/material-colors) 0.5.0
by Aiving, a Rust port of Google's Material Color Utilities, licensed
MIT OR Apache-2.0 (see `LICENSE-MIT` and `LICENSE-APACHE`).

Calculator vendors it so the Rust app generates exactly the colours the
Python app gets from [materialyoucolor](https://pypi.org/project/materialyoucolor/)
with the 2025 colour specification. Changes from the original:

1. Removed: image quantization and scoring (`image`, `quantize`, `score`,
   `utils/random.rs`), which the app does not use. This also lowers the
   minimum Rust version from 1.97 to 1.88.
2. `src/dynamic_color/color_spec_2025.rs`, Expressive variant on phones:
   - `primary`: the tone's upper bound is 25 for yellow hues, 88 for cyan
     hues and 98 otherwise, in both light and dark (the original applied
     the yellow rule only in light and the cyan rule only in dark).
   - `primary_container` in dark: `t_max_c(30, 93)` instead of
     `t_min_c(30, 93)`.

   Both match materialyoucolor and Google's reference ColorSpec2025.

`rust/scripts/palette_parity.py` checks the result against materialyoucolor.
