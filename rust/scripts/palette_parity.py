#!/usr/bin/env python3
"""Compare the Rust port's colours with the Python app's.

Needs what the Python app's colours need: PyGObject with GTK 4 (theme.py
imports it) and materialyoucolor. Checks generated palettes for many seeds,
every style, light and dark and each contrast level; the presets; the
palette.toml template; and palette.toml parsing.

    python3 rust/scripts/palette_parity.py [random seeds]
"""

import random
import subprocess
import sys
from pathlib import Path

RUST = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(RUST.parent))

from calculator.presets import ORDER, PRESETS  # noqa: E402
from calculator.theme import (ROLES, STYLE_NAMES, _generate_with_library, palette_template,  # noqa: E402
                              parse_hex, read_palette_file)

# Known difference: a pure black seed (#000000) with the fidelity or content
# style. Both versions give readable colours; they differ in the text on the
# primary and tertiary containers.
KNOWN = {("#000000", "fidelity"), ("#000000", "content")}

PALETTE_FILES = [
    'seed = "#2e7d32"\\nstyle = "vibrant"\\nmode = "dark"',
    '[roles]\\nonPrimary = "#fff"\\nsurface-container = "#123456"\\nbogus = "#000000"',
    '[dark]\\nsurface = "nope"\\n[light]\\nprimary = "#ABCDEF"',
    'seed = 12\\nstyle = "nope"\\nmode = "dim"',
    'seed = "#zzz"',
    'this is not toml',
    '',
]


def roles_line(roles):
    return " ".join(f"{r}={roles[r]}" for r in ROLES if r in roles)


def python_answer(case):
    kind = case[0]
    if kind == "palette":
        _, seed, style, dark, contrast = case
        return roles_line(_generate_with_library(parse_hex(seed), dark == "1", float(contrast), style))
    if kind == "preset":
        p = PRESETS[case[1]]
        own = p.get("dark" if case[2] == "1" else "light")
        return f"{p['name']}|{p['seed']}|{p['style']}|{roles_line(own) if own else '-'}"
    if kind == "template":
        _, seed, style, dark = case
        roles = _generate_with_library(parse_hex(seed), dark == "1", 0.0, style)
        return palette_template(roles, parse_hex(seed), style, dark == "1").replace("\n", "\\n")
    text = case[1].replace("\\n", "\n")
    path = Path("/tmp/calculator-palette-parity.toml")
    path.write_text(text, encoding="utf-8")
    p = read_palette_file(path)
    if p is None:
        return "none"
    seed = f'Some("{"#%02x%02x%02x" % p["seed"]}")' if p["seed"] else "None"
    def opt(v):
        return f'Some("{v}")' if v else "None"
    return "|".join([seed, opt(p["style"]), opt(p["mode"]), roles_line(p["roles"]),
                     roles_line(p["light"]), roles_line(p["dark"])])


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 60
    rng = random.Random(5)
    seeds = ["#6750a4", "#0061a4", "#386a20", "#006a6a", "#c2185b", "#e8590c", "#e0a100", "#9c27b0",
             "#5f6368", "#3584e4", "#ffffff", "#000000", "#ff0000", "#808080", "#ffff00", "#00ffff"]
    seeds += [f"#{rng.randrange(1 << 24):06x}" for _ in range(count)]
    cases = [("palette", s, st, d, c) for s in seeds for st in STYLE_NAMES for d in "01"
             for c in ("0.0", "0.5", "1.0")]
    cases += [("preset", key, d) for key in ORDER for d in "01"]
    cases += [("template", s, st, d) for s in seeds[:6] for st in ("tonal_spot", "vibrant") for d in "01"]
    cases += [("palfile", text) for text in PALETTE_FILES]

    subprocess.run(["cargo", "build", "-q", "--release", "--example", "parity"], cwd=RUST, check=True)
    binary = RUST / "target" / "release" / "examples" / "parity"
    stdin = "".join("\t".join(case) + "\n" for case in cases)
    rust = subprocess.run([binary], input=stdin, capture_output=True, text=True, check=True).stdout.splitlines()
    assert len(rust) == len(cases), (len(rust), len(cases))

    differences = known = 0
    for case, rust_answer in zip(cases, rust):
        if python_answer(case) == rust_answer:
            continue
        if case[0] == "palette" and (case[1], case[2]) in KNOWN:
            known += 1
            continue
        differences += 1
        if differences <= 10:
            print("case:  ", " ".join(case))
            print("python:", python_answer(case)[:400])
            print("rust:  ", rust_answer[:400])
            print()
    print(f"{len(cases)} cases: {differences} differ ({known} known differences skipped)")
    sys.exit(1 if differences else 0)


if __name__ == "__main__":
    main()
