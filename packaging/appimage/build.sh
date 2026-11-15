#!/bin/sh
# Build Calculator-<version>-x86_64.AppImage from target/release/calculator
# (run `cargo build --release` first). Downloads linuxdeploy and its GTK
# plugin into build/appimage/tools if they aren't there yet.
#
#   packaging/appimage/build.sh        writes build/appimage/Calculator-<version>-x86_64.AppImage
set -eu

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP_ID="io.github.HypestHyper.Calculator"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
WORK="$ROOT/build/appimage"
TOOLS="$WORK/tools"
APPDIR="$WORK/AppDir"
OUTPUT="$WORK/Calculator-$VERSION-x86_64.AppImage"

[ -x "$ROOT/target/release/calculator" ] || {
    echo "Build the program first: cargo build --release" >&2
    exit 1
}

mkdir -p "$TOOLS"
fetch() {
    [ -x "$TOOLS/$1" ] && return
    curl -fsSL -o "$TOOLS/$1" "$2"
    chmod +x "$TOOLS/$1"
}
fetch linuxdeploy-x86_64.AppImage \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
fetch linuxdeploy-plugin-gtk.sh \
    https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh

# The program finds its icon font in ../share/$APP_ID next to it.
rm -rf "$APPDIR"
install -Dm755 "$ROOT/target/release/calculator" "$APPDIR/usr/bin/$APP_ID"
install -Dm644 "$ROOT/data/fonts/MaterialSymbolsRounded.ttf" \
    "$APPDIR/usr/share/$APP_ID/fonts/MaterialSymbolsRounded.ttf"
install -Dm644 "$ROOT/data/$APP_ID.metainfo.xml" "$APPDIR/usr/share/metainfo/$APP_ID.metainfo.xml"
install -Dm644 "$ROOT/data/icons/hicolor/symbolic/apps/$APP_ID-symbolic.svg" \
    "$APPDIR/usr/share/icons/hicolor/symbolic/apps/$APP_ID-symbolic.svg"

# No FUSE on build machines: run the tools extracted.
export APPIMAGE_EXTRACT_AND_RUN=1
export PATH="$TOOLS:$PATH"
export DEPLOY_GTK_VERSION=4

# Bundle GTK 4 and the libraries the program needs.
linuxdeploy-x86_64.AppImage --appdir "$APPDIR" \
    --executable "$APPDIR/usr/bin/$APP_ID" \
    --desktop-file "$ROOT/data/$APP_ID.desktop" \
    --icon-file "$ROOT/data/icons/hicolor/scalable/apps/$APP_ID.svg" \
    --plugin gtk

# The GTK plugin forces the X11 backend, for old GTK 3 crashes on Wayland.
# GTK 4 is fine there, and on Wayland the calculator needs it for its window
# sizing and placement, so let GTK choose.
sed -i '/export GDK_BACKEND=x11/d' "$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"

rm -f "$OUTPUT"
LINUXDEPLOY_OUTPUT_VERSION="$VERSION" OUTPUT="$OUTPUT" \
    linuxdeploy-x86_64.AppImage --appdir "$APPDIR" --output appimage
echo "Built $OUTPUT"
