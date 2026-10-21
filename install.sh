#!/bin/sh
# Install Calculator for the current user (~/.local) or system-wide.
#
#   ./install.sh             install to ~/.local
#   sudo ./install.sh --system   install to /usr/local
#   ./install.sh --prefix DIR    install to DIR
#   ./install.sh --no-vendor     skip downloading materialyoucolor
#   ./install.sh --rust          install the Rust version instead (needs cargo)
#
# Uninstall with ./uninstall.sh and the same options.
set -eu

APP_ID="io.github.HypestHyper.Calculator"
PREFIX="${HOME}/.local"
VENDOR=1
RUST=0
while [ $# -gt 0 ]; do
    case "$1" in
        --system) PREFIX="/usr/local" ;;
        --prefix) PREFIX="$2"; shift ;;
        --no-vendor) VENDOR=0 ;;
        --rust) RUST=1 ;;
        -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

SRC="$(cd "$(dirname "$0")" && pwd)"
APPDIR="$PREFIX/share/$APP_ID"
BINDIR="$PREFIX/bin"

say() { printf '\033[1m%s\033[0m\n' "$*"; }

# --- Rust version: build first, so a failed build changes nothing ------------
RUST_BIN="$SRC/rust/target/release/calculator"
if [ "$RUST" = 1 ]; then
    if command -v cargo >/dev/null 2>&1; then
        say "Building the Rust version"
        cargo build --release --manifest-path "$SRC/rust/Cargo.toml"
    elif [ ! -x "$RUST_BIN" ]; then
        cat >&2 <<'EOF'
cargo was not found. Install Rust 1.88 or newer and the GTK 4 development files:
  Arch / CachyOS / Manjaro:  sudo pacman -S rust gtk4
  Debian / Ubuntu:           sudo apt install cargo libgtk-4-dev
  Fedora:                    sudo dnf install cargo gtk4-devel
With sudo, build first as yourself (cargo build --release in rust/), then rerun.
EOF
        exit 1
    elif [ -n "$(find "$SRC/rust/src" "$SRC/rust/vendor" "$SRC/rust/Cargo.toml" "$SRC/calculator/style.css" \
                 -newer "$RUST_BIN" -print 2>/dev/null | head -n 1)" ]; then
        echo "The Rust build in rust/target/release is older than the source and cargo was not found." >&2
        echo "Rebuild it as yourself with: cargo build --release --manifest-path rust/Cargo.toml" >&2
        exit 1
    else
        say "Using the existing Rust build"
    fi
fi

# --- dependencies -----------------------------------------------------------
if [ "$RUST" = 1 ]; then
    :  # the build above needed GTK 4 already
elif ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required." >&2; exit 1
elif ! python3 -c 'import sys; sys.exit(sys.version_info < (3, 11))'; then
    echo "Python 3.11 or newer is required." >&2; exit 1
elif ! python3 -c 'import gi; gi.require_version("Gtk", "4.0"); from gi.repository import Gtk' 2>/dev/null; then
    cat >&2 <<'EOF'
GTK 4 for Python is missing. Install it with your package manager:
  Arch / CachyOS / Manjaro:  sudo pacman -S python-gobject gtk4
  Debian / Ubuntu:           sudo apt install python3-gi gir1.2-gtk-4.0
  Fedora:                    sudo dnf install python3-gobject gtk4
  openSUSE:                  sudo zypper install python3-gobject typelib-1_0-Gtk-4_0
EOF
    exit 1
fi

# --- files ------------------------------------------------------------------
say "Installing to $PREFIX"
rm -rf "$APPDIR/calculator"
mkdir -p "$APPDIR/fonts" "$BINDIR" \
    "$PREFIX/share/applications" "$PREFIX/share/metainfo" \
    "$PREFIX/share/icons/hicolor/scalable/apps" "$PREFIX/share/icons/hicolor/symbolic/apps"

if [ "$RUST" = 0 ]; then
    cp -r "$SRC/calculator" "$APPDIR/calculator"
    find "$APPDIR/calculator" -name '__pycache__' -type d -prune -exec rm -rf {} +
fi
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

# --- optional: Material colour library --------------------------------------
# Generates the exact Material 3 palettes. Without it a built-in approximation is used.
# The Rust version has its own copy built in.
if [ "$RUST" = 1 ]; then
    rm -rf "$APPDIR/vendor"
elif python3 -c 'import materialyoucolor' 2>/dev/null; then
    say "Using the installed materialyoucolor"
elif [ "$VENDOR" = 1 ]; then
    say "Fetching materialyoucolor into $APPDIR/vendor"
    rm -rf "$APPDIR/vendor"
    if ! python3 -m pip install --quiet --disable-pip-version-check --no-deps \
            --target "$APPDIR/vendor" materialyoucolor 2>/dev/null; then
        echo "  Could not fetch it (pip missing or offline). The built-in palette generator will be used."
        rm -rf "$APPDIR/vendor"
    fi
fi

# --- launcher ---------------------------------------------------------------
# Remove first: a running Rust build can't be overwritten in place.
rm -f "$BINDIR/$APP_ID"
if [ "$RUST" = 1 ]; then
    # The program itself; it finds its fonts in $PREFIX/share/$APP_ID.
    cp "$RUST_BIN" "$BINDIR/$APP_ID"
else
    cat > "$BINDIR/$APP_ID" <<EOF
#!/bin/sh
APPDIR="$APPDIR"
PYTHONPATH="\$APPDIR:\$APPDIR/vendor\${PYTHONPATH:+:\$PYTHONPATH}" exec python3 -m calculator "\$@"
EOF
fi
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
