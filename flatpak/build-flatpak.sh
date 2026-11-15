#!/usr/bin/env bash
#
# Builds the Linux .flatpak package for Rongyok Desk.
#
# Run this on Linux, from the app/ folder:
#   ./flatpak/build-flatpak.sh
#
# You get dist/flatpak/Rongyok-Desk-<version>-x86_64.flatpak, ready to install with:
#   flatpak install --user dist/flatpak/Rongyok-Desk-<version>-x86_64.flatpak
#   flatpak run com.rongyok.ssb
#
# What it needs on the machine:
#   flatpak, flatpak-builder, and the GNOME runtime + SDK this manifest names.
#   The script offers to install those for you if they are missing.

set -euo pipefail

APP_ID="com.rongyok.ssb"
MANIFEST="flatpak/${APP_ID}.yml"
ARCH="${FLATPAK_ARCH:-x86_64}"
RUNTIME_VERSION="49"

# Work from app/, wherever the script was called from.
cd "$(dirname "$0")/.."
APP_DIR="$PWD"
DIST_DIR="${APP_DIR}/dist/flatpak"
BUILD_DIR="${APP_DIR}/target/flatpak-build"

say() { printf '\033[1m==> %s\033[0m\n' "$*"; }
die() { printf '\033[1;31mERROR:\033[0m %s\n' "$*" >&2; exit 1; }

# --- 1. Check we are where we think we are -------------------------------
[ -f Cargo.toml ] || die "Run this from the app/ folder (Cargo.toml not found)."
[ -f tauri.conf.json ] || die "Run this from the app/ folder (tauri.conf.json not found)."
[ -f "$MANIFEST" ] || die "Manifest $MANIFEST is missing."

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
[ -n "$VERSION" ] || die "Could not read the version from Cargo.toml."
say "Building Rongyok Desk ${VERSION} for ${ARCH}"

# --- 2. Check the tools --------------------------------------------------
missing=()
command -v flatpak >/dev/null 2>&1 || missing+=("flatpak")
command -v flatpak-builder >/dev/null 2>&1 || missing+=("flatpak-builder")

if [ ${#missing[@]} -gt 0 ]; then
  say "Missing: ${missing[*]}"
  case "$(uname -s)" in
    Linux)  echo "  Install them with your package manager, for example:"
            echo "    Debian/Ubuntu: sudo apt install flatpak flatpak-builder"
            echo "    Fedora:        sudo dnf install flatpak flatpak-builder"
            echo "    Arch:          sudo pacman -S flatpak flatpak-builder" ;;
    Darwin) echo "  flatpak builds Linux packages; run this script on Linux,"
            echo "  or use the GitHub Actions workflow, which does it for you." ;;
    *)      echo "  flatpak builds Linux packages; run this on Linux." ;;
  esac
  die "Install those, then run this script again."
fi

# --- 3. Make sure the runtime and SDK are there --------------------------
# The SDK extension carries Rust, which the manifest puts on the PATH.
RUNTIME_REF="runtime/org.gnome.Platform/${ARCH}/${RUNTIME_VERSION}"
SDK_REF="runtime/org.gnome.Sdk/${ARCH}/${RUNTIME_VERSION}"
SIMPLE_SDK_REF="runtime/org.freedesktop.Sdk.Extension.rust-stable/${ARCH}/25.08"

for ref in "$RUNTIME_REF" "$SDK_REF" "$SIMPLE_SDK_REF"; do
  if ! flatpak info "$ref" >/dev/null 2>&1; then
    say "Installing $ref (one time, this can take a while)"
    flatpak install --user --noninteractive flathub "$ref" \
      || die "Could not install $ref. Add the flathub remote first: flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo"
  fi
done

# --- 4. Build --------------------------------------------------------------
mkdir -p "$DIST_DIR"

say "Running flatpak-builder (this compiles the app, so give it a few minutes)"
flatpak-builder \
  --user \
  --force-clean \
  --arch="$ARCH" \
  --state-dir="${BUILD_DIR}/.flatpak-builder" \
  --repo="${BUILD_DIR}/repo" \
  "${BUILD_DIR}/build" \
  "$MANIFEST"

# --- 5. Package it -------------------------------------------------------
OUT="${DIST_DIR}/Rongyok-Desk-${VERSION}-${ARCH}.flatpak"
say "Packing ${OUT}"
flatpak build-bundle \
  "${BUILD_DIR}/repo" \
  "$OUT" \
  "$APP_ID" \
  --arch="$ARCH" \
  --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo

[ -s "$OUT" ] || die "The bundle was not written."

say "Done: $OUT ($(du -h "$OUT" | cut -f1))"
echo
echo "Install it with:"
echo "  flatpak install --user \"$OUT\""
echo "Then run it from your app menu, or:"
echo "  flatpak run ${APP_ID}"