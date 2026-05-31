#!/bin/bash
# Build RustConn for macOS and create/update the .app bundle.
#
# Usage:
#   ./scripts/macos-build.sh              # debug build + bundle + launch
#   ./scripts/macos-build.sh --release    # release build + bundle + launch
#   ./scripts/macos-build.sh --no-launch  # build + bundle, don't launch
#   ./scripts/macos-build.sh --clean      # remove existing bundle first

set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# Config
# ──────────────────────────────────────────────────────────────────────────────
BUNDLE_NAME="dist/RustConn.app"
BUNDLE_ID="io.github.totoshko88.RustConn"
FEATURES="tray-macos,vnc-embedded,rdp-embedded,rdp-audio,spice-embedded,adw-1-8"
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')
ICON_SVG="rustconn/assets/icons/hicolor/scalable/apps/io.github.totoshko88.RustConn.svg"

# ──────────────────────────────────────────────────────────────────────────────
# Args
# ──────────────────────────────────────────────────────────────────────────────
RELEASE=false
LAUNCH=true
CLEAN=false

for arg in "$@"; do
    case "$arg" in
        --release) RELEASE=true ;;
        --no-launch) LAUNCH=false ;;
        --clean) CLEAN=true ;;
        -h|--help)
            echo "Usage: $0 [--release] [--no-launch] [--clean]"
            exit 0
            ;;
        *) echo "Unknown option: $arg"; exit 2 ;;
    esac
done

if $RELEASE; then
    PROFILE="release"
    CARGO_FLAGS="--release"
    TARGET_DIR="target/release"
else
    PROFILE="debug"
    CARGO_FLAGS=""
    TARGET_DIR="target/debug"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Helpers
# ──────────────────────────────────────────────────────────────────────────────
info() { printf '\033[34m[info]\033[0m %s\n' "$*"; }
ok()   { printf '\033[32m[ ok ]\033[0m %s\n' "$*"; }
fail() { printf '\033[31m[fail]\033[0m %s\n' "$*" >&2; exit 1; }

# ──────────────────────────────────────────────────────────────────────────────
# 1. Build
# ──────────────────────────────────────────────────────────────────────────────
info "Building rustconn ($PROFILE) with features: $FEATURES"
cargo build -p rustconn --no-default-features --features "$FEATURES" $CARGO_FLAGS \
    || fail "cargo build failed"
cargo build -p rustconn-cli $CARGO_FLAGS || fail "cargo build rustconn-cli failed"
ok "Build complete: $TARGET_DIR/rustconn"

# ──────────────────────────────────────────────────────────────────────────────
# 2. Create bundle structure
# ──────────────────────────────────────────────────────────────────────────────
if $CLEAN && [ -d "$BUNDLE_NAME" ]; then
    info "Removing existing $BUNDLE_NAME"
    rm -rf "$BUNDLE_NAME"
fi

mkdir -p "$BUNDLE_NAME/Contents/MacOS"
mkdir -p "$BUNDLE_NAME/Contents/Resources/share/icons"

# ──────────────────────────────────────────────────────────────────────────────
# 3. Copy binaries
# ──────────────────────────────────────────────────────────────────────────────
cp "$TARGET_DIR/rustconn" "$BUNDLE_NAME/Contents/MacOS/"
cp "$TARGET_DIR/rustconn-cli" "$BUNDLE_NAME/Contents/MacOS/"
ok "Binaries copied"

# ──────────────────────────────────────────────────────────────────────────────
# 4. Generate icon (.icns)
# ──────────────────────────────────────────────────────────────────────────────
if command -v rsvg-convert &>/dev/null && [ -f "$ICON_SVG" ]; then
    ICONSET=$(mktemp -d)/RustConn.iconset
    mkdir -p "$ICONSET"
    for size in 16 32 64 128 256 512 1024; do
        rsvg-convert -w "$size" -h "$size" "$ICON_SVG" -o "$ICONSET/icon_${size}.png"
    done
    cp "$ICONSET/icon_16.png"   "$ICONSET/icon_16x16.png"
    cp "$ICONSET/icon_32.png"   "$ICONSET/icon_16x16@2x.png"
    cp "$ICONSET/icon_32.png"   "$ICONSET/icon_32x32.png"
    cp "$ICONSET/icon_64.png"   "$ICONSET/icon_32x32@2x.png"
    cp "$ICONSET/icon_128.png"  "$ICONSET/icon_128x128.png"
    cp "$ICONSET/icon_256.png"  "$ICONSET/icon_128x128@2x.png"
    cp "$ICONSET/icon_256.png"  "$ICONSET/icon_256x256.png"
    cp "$ICONSET/icon_512.png"  "$ICONSET/icon_256x256@2x.png"
    cp "$ICONSET/icon_512.png"  "$ICONSET/icon_512x512.png"
    cp "$ICONSET/icon_1024.png" "$ICONSET/icon_512x512@2x.png"
    iconutil -c icns "$ICONSET" -o "$BUNDLE_NAME/Contents/Resources/RustConn.icns" 2>/dev/null
    rm -rf "$(dirname "$ICONSET")"
    ok "Icon generated"
else
    info "Skipping icon (rsvg-convert not found or SVG missing)"
fi

# ──────────────────────────────────────────────────────────────────────────────
# 5. Compile locales
# ──────────────────────────────────────────────────────────────────────────────
if command -v msgfmt &>/dev/null; then
    for f in po/*.po; do
        lang=$(basename "$f" .po)
        mkdir -p "$BUNDLE_NAME/Contents/Resources/locale/${lang}/LC_MESSAGES"
        msgfmt -o "$BUNDLE_NAME/Contents/Resources/locale/${lang}/LC_MESSAGES/rustconn.mo" "$f"
    done
    ok "Locales compiled ($(ls po/*.po | wc -l | tr -d ' ') languages)"
else
    info "Skipping locales (msgfmt not found — brew install gettext)"
fi

# ──────────────────────────────────────────────────────────────────────────────
# 6. Copy Adwaita icons (for sidebar/toolbar icons)
# ──────────────────────────────────────────────────────────────────────────────
ADWAITA_ICONS="/opt/homebrew/share/icons/Adwaita"
if [ -d "$ADWAITA_ICONS" ]; then
    # -RL: dereference symlinks (Homebrew uses symlinks to Cellar)
    cp -RL "$ADWAITA_ICONS" "$BUNDLE_NAME/Contents/Resources/share/icons/"
    ok "Adwaita icons bundled"
else
    ADWAITA_ICONS="/usr/local/share/icons/Adwaita"
    if [ -d "$ADWAITA_ICONS" ]; then
        cp -RL "$ADWAITA_ICONS" "$BUNDLE_NAME/Contents/Resources/share/icons/"
        ok "Adwaita icons bundled"
    else
        info "Skipping Adwaita icons (not found — brew install adwaita-icon-theme)"
    fi
fi

# Also copy hicolor theme from system and add our app icon
HICOLOR_ICONS="/opt/homebrew/share/icons/hicolor"
if [ -d "$HICOLOR_ICONS" ]; then
    # -RL: dereference symlinks (Homebrew uses symlinks to Cellar)
    cp -RL "$HICOLOR_ICONS" "$BUNDLE_NAME/Contents/Resources/share/icons/"
    ok "hicolor icons bundled"
fi
# Ensure our app icon is present (after hicolor copy so directory exists)
mkdir -p "$BUNDLE_NAME/Contents/Resources/share/icons/hicolor/scalable/apps"
if [ -f "$ICON_SVG" ]; then
    cp "$ICON_SVG" "$BUNDLE_NAME/Contents/Resources/share/icons/hicolor/scalable/apps/"
fi

# Update icon caches for GTK4 lookup
if command -v gtk4-update-icon-cache &>/dev/null; then
    gtk4-update-icon-cache -f -t "$BUNDLE_NAME/Contents/Resources/share/icons/Adwaita" 2>/dev/null || true
    gtk4-update-icon-cache -f -t "$BUNDLE_NAME/Contents/Resources/share/icons/hicolor" 2>/dev/null || true
    ok "Icon caches updated"
fi

# ──────────────────────────────────────────────────────────────────────────────
# 7. Create wrapper script
# ──────────────────────────────────────────────────────────────────────────────
cat > "$BUNDLE_NAME/Contents/MacOS/rustconn-wrapper" << 'EOF'
#!/bin/bash
DIR="$(cd "$(dirname "$0")/.." && pwd)"
export XDG_DATA_DIRS="$DIR/Resources/share:/opt/homebrew/share:/usr/local/share:/usr/share"
export GSETTINGS_SCHEMA_DIR="/opt/homebrew/share/glib-2.0/schemas"
export LOCALEDIR="$DIR/Resources/locale"
cd "$HOME"
exec "$DIR/MacOS/rustconn" "$@"
EOF
chmod +x "$BUNDLE_NAME/Contents/MacOS/rustconn-wrapper"

# ──────────────────────────────────────────────────────────────────────────────
# 8. Create Info.plist
# ──────────────────────────────────────────────────────────────────────────────
cat > "$BUNDLE_NAME/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>rustconn-wrapper</string>
    <key>CFBundleIconFile</key>
    <string>RustConn</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleName</key>
    <string>RustConn</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
    <key>NSDocumentsFolderUsageDescription</key>
    <string>RustConn needs access to import SSH configs and connection files.</string>
    <key>NSAppleEventsUsageDescription</key>
    <string>RustConn needs to open URLs in your default browser.</string>
</dict>
</plist>
EOF
ok "Info.plist created (version $VERSION)"

# ──────────────────────────────────────────────────────────────────────────────
# 9. Ad-hoc code sign
# ──────────────────────────────────────────────────────────────────────────────
codesign --force --deep --sign - "$BUNDLE_NAME" 2>/dev/null && ok "Code signed (ad-hoc)" || true

# ──────────────────────────────────────────────────────────────────────────────
# Done
# ──────────────────────────────────────────────────────────────────────────────
echo ""
ok "Bundle ready: $BUNDLE_NAME ($PROFILE, v$VERSION)"
echo "   Launch:  open $BUNDLE_NAME"
echo "   CLI:     ./$BUNDLE_NAME/Contents/MacOS/rustconn-cli --help"
echo ""

if $LAUNCH; then
    info "Launching..."
    open "$BUNDLE_NAME"
fi
