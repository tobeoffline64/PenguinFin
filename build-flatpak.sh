#!/usr/bin/env bash
# Build dist/penguinfin.flatpak.
#
#   ./build-flatpak.sh            fast: incremental build, no LTO (seconds after the first build)
#   ./build-flatpak.sh --release  incremental build with the release profile (thin LTO, ~30 s)
#   ./build-flatpak.sh --clean    from scratch inside flatpak-builder, like the GitHub release build
#
# The fast modes compile with cargo inside the same GNOME SDK that flatpak-builder uses (so the
# binary links against the runtime's libraries), but keep cargo's output in target/flatpak/
# between runs. flatpak-builder then only packages the binary.
#
# Requires: flatpak, flatpak-builder, cargo (for vendoring crates).
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$PWD"
APP_ID=io.github.tobeoffline64.PenguinFin
MANIFEST="flatpak/$APP_ID.yml"
DEV="$ROOT/.flatpak-dev"        # persistent build state for the fast modes (git-ignored)
SDK_DIR="$DEV/sdk"              # flatpak build dir used as a compile sandbox
PREBUILT="$ROOT/.flatpak-prebuilt"

MODE=fast
case "${1:-}" in
  "") ;;
  --release) MODE=release ;;
  --clean) MODE=clean ;;
  -h|--help) sed -n '2,13p' "$0"; exit 0 ;;
  *) echo "Unknown option: $1 (use --release, --clean or --help)" >&2; exit 2 ;;
esac

mkdir -p "$DEV"
CURRENT_STEP="checking requirements"
step() { CURRENT_STEP="$*"; printf '\n\033[1m==> %s\033[0m\n' "$*"; }
# Any failing command stops the script; say where, instead of exiting silently.
trap 'echo >&2; echo "Build failed during: $CURRENT_STEP" >&2' ERR
SECONDS=0

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

# 1. Runtimes: only talk to Flathub when something is missing (also lets you build offline).
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

# 2. Vendor crates (the flatpak build has no network) only when Cargo.lock changed.
LOCK_HASH="$(sha256sum Cargo.lock | cut -d' ' -f1)"
if [[ ! -d vendor || ! -f "$DEV/vendor.lock-hash" || "$(cat "$DEV/vendor.lock-hash")" != "$LOCK_HASH" ]]; then
  step "Vendoring Rust crates"
  cargo vendor --locked --quiet vendor > /dev/null   # flatpak/cargo-vendor-config.toml points cargo at it
  echo "$LOCK_HASH" > "$DEV/vendor.lock-hash"
fi

if [[ "$MODE" == clean ]]; then
  # Make sure flatpak-builder compiles from source instead of using a local binary.
  if [[ -d "$PREBUILT" ]]; then rm -r -- "$PREBUILT"; fi
else
  # 3. One-time setup of the compile sandbox: GNOME SDK + Rust extension + libxdo
  #    (needed to link; the runtime doesn't ship it, the manifest builds it for the app).
  if [[ ! -d "$SDK_DIR" ]]; then
    step "Creating the build sandbox (one time)"
    flatpak build-init --sdk-extension=org.freedesktop.Sdk.Extension.rust-stable \
      "$SDK_DIR" "$APP_ID" org.gnome.Sdk org.gnome.Platform 49
  fi
  if [[ ! -e "$SDK_DIR/files/lib/libxdo.so" ]]; then
    step "Building libxdo (one time)"
    XDO_URL=https://github.com/jordansissel/xdotool/releases/download/v3.20211022.1/xdotool-3.20211022.1.tar.gz
    XDO_SHA=96f0facfde6d78eacad35b91b0f46fecd0b35e474c03e00e30da3fdd345f9ada
    [[ -f "$DEV/xdotool.tar.gz" ]] || curl -fsSL -o "$DEV/xdotool.tar.gz" "$XDO_URL"
    echo "$XDO_SHA  $DEV/xdotool.tar.gz" | sha256sum -c --quiet -
    mkdir -p "$DEV/xdotool"
    tar xzf "$DEV/xdotool.tar.gz" -C "$DEV/xdotool" --strip-components=1
    # xdotool's C code prints harmless compiler warnings; keep them in a log and only
    # show it if the build actually fails.
    if ! flatpak build --filesystem="$DEV" "$SDK_DIR" sh -c "cd '$DEV/xdotool' && make -s PREFIX=/app libxdo.so \
      && make -s PREFIX=/app INSTALLLIB=/app/lib INSTALLINCLUDE=/app/include installlib installheader" \
      > "$DEV/libxdo.log" 2>&1; then
      tail -30 "$DEV/libxdo.log" >&2
      echo "libxdo failed to build; full log: $DEV/libxdo.log" >&2
      exit 1
    fi
  fi
  mkdir -p "$DEV/cargo-home"
  cat > "$DEV/cargo-home/config.toml" <<EOF
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "$ROOT/vendor"

# mold (shipped with the SDK's Rust extension) links much faster than the default linker.
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
EOF

  # 4. Incremental compile inside the SDK. Output persists in target/flatpak/.
  if [[ "$MODE" == release ]]; then PROFILE=release; PROFILE_ENV=(--env=CARGO_PROFILE_RELEASE_INCREMENTAL=true)
  else PROFILE=fast-release; PROFILE_ENV=(); fi
  step "Compiling ($PROFILE profile, incremental)"
  flatpak build --filesystem="$ROOT" \
    --env=PATH=/usr/lib/sdk/rust-stable/bin:/app/bin:/usr/bin \
    --env=CARGO_HOME="$DEV/cargo-home" --env=LIBRARY_PATH=/app/lib "${PROFILE_ENV[@]}" \
    "$SDK_DIR" sh -c "cd '$ROOT' && cargo build --profile $PROFILE --locked --offline --target-dir target/flatpak"
  mkdir -p "$PREBUILT"
  cp "target/flatpak/$PROFILE/penguinfin" "$PREBUILT/penguinfin"
fi

# 5. Package. flatpak-builder caches the libxdo module; with a prebuilt binary the app
#    module only installs files, so the 800 MB vendor/ folder doesn't need copying.
#    That uses a generated copy of the manifest (absolute source path, vendor skipped).
PACK_MANIFEST="$MANIFEST"
if [[ "$MODE" != clean ]]; then
  FAST_MANIFEST="$DEV/$APP_ID.yml"
  sed -e "s|^        path: \.\.\$|        path: $ROOT|" \
      -e 's|^          - \.flatpak-dev$|          - .flatpak-dev\n          - vendor|' "$MANIFEST" > "$FAST_MANIFEST"
  if grep -q "path: $ROOT\$" "$FAST_MANIFEST" && grep -q '^          - vendor$' "$FAST_MANIFEST"; then
    PACK_MANIFEST="$FAST_MANIFEST"
  fi
fi
step "Packaging"
flatpak-builder --user --force-clean --disable-rofiles-fuse --repo=repo build-dir "$PACK_MANIFEST" > "$DEV/flatpak-builder.log" 2>&1 \
  || { tail -40 "$DEV/flatpak-builder.log" >&2; exit 1; }
mkdir -p dist
flatpak build-bundle repo dist/penguinfin.flatpak "$APP_ID" \
  --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo

echo
echo "Built dist/penguinfin.flatpak ($MODE build, ${SECONDS}s)"
echo "Install/update with: flatpak install --user -y --reinstall dist/penguinfin.flatpak"
