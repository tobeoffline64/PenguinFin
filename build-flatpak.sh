#!/usr/bin/env bash
# Build dist/penguinfin.flatpak. Requires: flatpak, flatpak-builder, cargo (for vendoring).
set -euo pipefail
cd "$(dirname "$0")"
APP_ID=io.github.tobeoffline64.PenguinFin

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y --noninteractive flathub \
  org.gnome.Platform//49 org.gnome.Sdk//49 org.freedesktop.Sdk.Extension.rust-stable//25.08

# Vendor crates so the sandboxed build needs no network.
cargo vendor --locked vendor > /dev/null

flatpak-builder --user --force-clean --disable-rofiles-fuse --repo=repo build-dir "flatpak/$APP_ID.yml"
mkdir -p dist
flatpak build-bundle repo dist/penguinfin.flatpak "$APP_ID" \
  --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
echo "Built dist/penguinfin.flatpak"
