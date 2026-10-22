#!/bin/sh
# Install Calculator for the current user (~/.local) or system-wide.
#
#   ./install.sh                 install to ~/.local
#   sudo ./install.sh --system   install to /usr/local
#   ./install.sh --prefix DIR    install to DIR
#
# With cargo installed, the program is built from source first. Without it, the
# ready-built program from a release download (bin/calculator) is used.
# Uninstall with ./uninstall.sh and the same options.
set -eu

APP_ID="io.github.HypestHyper.Calculator"
PREFIX="${HOME}/.local"
while [ $# -gt 0 ]; do
    case "$1" in
        --system) PREFIX="/usr/local" ;;
        --prefix) PREFIX="$2"; shift ;;
        --rust) ;;  # accepted for older instructions; there is only one version now
        -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

SRC="$(cd "$(dirname "$0")" && pwd)"
APPDIR="$PREFIX/share/$APP_ID"
BINDIR="$PREFIX/bin"

say() { printf '\033[1m%s\033[0m\n' "$*"; }

# --- the program: build first, so a failed build changes nothing ------------
BUILT="$SRC/target/release/calculator"
READY="$SRC/bin/calculator"
newer_source() {
    [ -n "$(find "$SRC/src" "$SRC/vendor" "$SRC/Cargo.toml" -newer "$1" -print 2>/dev/null | head -n 1)" ]
}
if command -v cargo >/dev/null 2>&1; then
    say "Building Calculator"
    cargo build --release --manifest-path "$SRC/Cargo.toml"
    PROGRAM="$BUILT"
elif [ -x "$BUILT" ] && ! newer_source "$BUILT"; then
    say "Using the existing build"
    PROGRAM="$BUILT"
elif [ -x "$READY" ]; then
    say "Using the ready-built program"
    PROGRAM="$READY"
else
    cat >&2 <<'EOF'
cargo was not found, and there is no ready-built program here. Either download
the release archive (it includes one), or install Rust 1.88 or newer and the
GTK 4 development files:
  Arch / CachyOS / Manjaro:  sudo pacman -S rust gtk4
  Debian / Ubuntu:           sudo apt install cargo libgtk-4-dev
  Fedora:                    sudo dnf install cargo gtk4-devel
With sudo, build first as yourself (cargo build --release), then rerun.
EOF
    exit 1
fi

# --- files ------------------------------------------------------------------
say "Installing to $PREFIX"
# Left over from the Python version (1.x).
rm -rf "$APPDIR/calculator" "$APPDIR/vendor"
mkdir -p "$APPDIR/fonts" "$BINDIR" \
    "$PREFIX/share/applications" "$PREFIX/share/metainfo" \
    "$PREFIX/share/icons/hicolor/scalable/apps" "$PREFIX/share/icons/hicolor/symbolic/apps"

cp "$SRC/data/fonts/"* "$APPDIR/fonts/"
# Launchers often run without ~/.local/bin in PATH, so name the program by its full path.
case "$BINDIR" in
    *" "*) EXEC="\"$BINDIR/$APP_ID\"" ;;
    *) EXEC="$BINDIR/$APP_ID" ;;
esac
sed "s|^Exec=.*|Exec=$EXEC|" "$SRC/data/$APP_ID.desktop" > "$PREFIX/share/applications/$APP_ID.desktop"
cp "$SRC/data/$APP_ID.metainfo.xml" "$PREFIX/share/metainfo/"
cp "$SRC/data/icons/hicolor/scalable/apps/$APP_ID.svg" "$PREFIX/share/icons/hicolor/scalable/apps/"
cp "$SRC/data/icons/hicolor/symbolic/apps/$APP_ID-symbolic.svg" "$PREFIX/share/icons/hicolor/symbolic/apps/"

# --- program ----------------------------------------------------------------
# Remove first: a running copy can't be overwritten in place.
rm -f "$BINDIR/$APP_ID"
# It finds its fonts in $PREFIX/share/$APP_ID.
cp "$PROGRAM" "$BINDIR/$APP_ID"
chmod 755 "$BINDIR/$APP_ID"

# Short command, only if nothing else already owns the name.
if [ -L "$BINDIR/calculator" ] && [ "$(readlink "$BINDIR/calculator")" = "$APP_ID" ]; then
    SHORT="calculator"  # ours, from an earlier install
elif [ ! -e "$BINDIR/calculator" ] && ! command -v calculator >/dev/null 2>&1; then
    ln -s "$APP_ID" "$BINDIR/calculator"
    SHORT="calculator"
else
    SHORT="$APP_ID"
fi

# --- caches -----------------------------------------------------------------
command -v gtk4-update-icon-cache >/dev/null 2>&1 && \
    gtk4-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && \
    gtk-update-icon-cache -q -t -f "$PREFIX/share/icons/hicolor" 2>/dev/null || true
command -v update-desktop-database >/dev/null 2>&1 && \
    update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true

say "Done. Start it from your launcher or run: $SHORT"
case ":$PATH:" in
    *":$BINDIR:"*) ;;
    *) echo "Note: $BINDIR is not in your PATH." ;;
esac
echo "To open it floating on a tiling compositor, see Preferences > Window, or run:"
echo "  $SHORT --print-window-rules hyprland   (or niri)"
