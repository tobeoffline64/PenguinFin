#!/usr/bin/env bash
# Build dist/penguinfin.flatpak. Requires: flatpak, flatpak-builder, cargo (for vendoring).
set -euo pipefail
cd "$(dirname "$0")"
APP_ID=io.github.tobeoffline64.PenguinFin

missing=()
for tool in flatpak flatpak-builder cargo; do
  command -v "$tool" > /dev/null || missing+=("$tool")
done
if (( ${#missing[@]} )); then
  echo "Missing: ${missing[*]}" >&2
  echo "On Ubuntu/Debian:  sudo apt install flatpak flatpak-builder" >&2
  echo "For cargo:         https://rustup.rs" >&2
  exit 1
fi

flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y --noninteractive flathub \
  org.gnome.Platform//49 org.gnome.Sdk//49 org.freedesktop.Sdk.Extension.rust-stable//25.08

# Vendor crates so the sandboxed build needs no network.
# (The build supplies the matching cargo config itself, so cargo's hint is hidden.)
echo "Vendoring Rust crates..."
cargo vendor --locked --quiet vendor > /dev/null

flatpak-builder --user --force-clean --disable-rofiles-fuse --repo=repo build-dir "flatpak/$APP_ID.yml"
mkdir -p dist
flatpak build-bundle repo dist/penguinfin.flatpak "$APP_ID" \
  --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo
echo "Built dist/penguinfin.flatpak"
