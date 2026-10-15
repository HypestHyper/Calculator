#!/bin/sh
# Remove Calculator. Use the same --system / --prefix option as for install.
# Settings, palette and history are kept unless you pass --purge.
set -eu

APP_ID="io.github.HypestHyper.Calculator"
PREFIX="${HOME}/.local"
PURGE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --system) PREFIX="/usr/local" ;;
        --prefix) PREFIX="$2"; shift ;;
        --purge) PURGE=1 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
    shift
done

BINDIR="$PREFIX/bin"
if [ -L "$BINDIR/calculator" ] && [ "$(readlink "$BINDIR/calculator")" = "$APP_ID" ]; then
    rm -f "$BINDIR/calculator"
fi
rm -f "$BINDIR/$APP_ID" \
      "$PREFIX/share/applications/$APP_ID.desktop" \
      "$PREFIX/share/metainfo/$APP_ID.metainfo.xml" \
      "$PREFIX/share/icons/hicolor/scalable/apps/$APP_ID.svg" \
      "$PREFIX/share/icons/hicolor/symbolic/apps/$APP_ID-symbolic.svg"
rm -rf "$PREFIX/share/$APP_ID"

# The app keeps a palette-coloured copy of its icon in the user's icon folder.
USER_ICON="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps/$APP_ID.svg"
if [ -f "$USER_ICON" ] && grep -q "Material Design Icons" "$USER_ICON"; then
    rm -f "$USER_ICON"
fi

if [ "$PURGE" = 1 ]; then
    rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/$APP_ID" "${XDG_STATE_HOME:-$HOME/.local/state}/$APP_ID"
fi
command -v update-desktop-database >/dev/null 2>&1 && \
    update-desktop-database -q "$PREFIX/share/applications" 2>/dev/null || true
echo "Calculator removed from $PREFIX."
