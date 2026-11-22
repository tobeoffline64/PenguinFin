#!/usr/bin/env bash
# Build dist/penguinfin-2.flatpak (PenguinFin 2, same app ID as 1.x).
#
#   app/build-flatpak.sh          build the interface, vendor crates, compile inside flatpak-builder
#
# Requires: flatpak, flatpak-builder, cargo, trunk, the wasm32-unknown-unknown target, npm
# (for Tailwind). The GNOME 49 runtime and Rust SDK extension are installed from Flathub if missing.
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
APP_ID=io.github.tobeoffline64.PenguinFin
MANIFEST="flatpak/$APP_ID.v2.yml"
CURRENT_STEP="checking requirements"
step() { CURRENT_STEP="$*"; printf '\n\033[1m==> %s\033[0m\n' "$*"; }
trap 'echo >&2; echo "Build failed during: $CURRENT_STEP" >&2' ERR
SECONDS=0

for tool in flatpak flatpak-builder cargo trunk npm npx; do
  command -v "$tool" > /dev/null || { echo "Missing: $tool (see app/README.md)" >&2; exit 1; }
done

RUNTIMES=(org.gnome.Platform//49 org.gnome.Sdk//49 org.freedesktop.Sdk.Extension.rust-stable//25.08)
need=()
for r in "${RUNTIMES[@]}"; do
  flatpak info --user "$r" > /dev/null 2>&1 || flatpak info --system "$r" > /dev/null 2>&1 || need+=("$r")
done
if (( ${#need[@]} )); then
  step "Installing runtimes: ${need[*]}"
  flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
  flatpak install --user -y --noninteractive flathub "${need[@]}"
fi

step "Building styles and the interface"
[ -d node_modules/tailwindcss ] || npm ci --silent
npx @tailwindcss/cli -i style/app.css -o assets/app.css --minify > /dev/null 2>&1
(cd app/ui && trunk build --release > /dev/null)

step "Vendoring Rust crates"
(cd app && cargo vendor --locked --quiet vendor > /dev/null)

step "Compiling and packaging (inside flatpak-builder)"
mkdir -p .flatpak-dev
flatpak-builder --user --force-clean --disable-rofiles-fuse --repo=repo-v2 build-dir-v2 "$MANIFEST" > .flatpak-dev/flatpak-builder-v2.log 2>&1 \
  || { tail -40 .flatpak-dev/flatpak-builder-v2.log >&2; exit 1; }
mkdir -p dist
flatpak build-bundle repo-v2 dist/penguinfin-2.flatpak "$APP_ID" --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
echo
echo "Built dist/penguinfin-2.flatpak in ${SECONDS}s"
echo "Install with: flatpak install --user -y --reinstall dist/penguinfin-2.flatpak"
