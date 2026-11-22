# PenguinFin 2 (Tauri + Leptos)

The next major version, being built next to the 1.x desktop app. It reuses the engine
(`../crates/penguinfin-core`) and storage (`../crates/penguinfin-store`) crates and replaces the
Dioxus interface with a Leptos one running in Tauri 2. The plan and its phases are in the
"PenguinFin: moving to Tauri and Leptos" doc.

**Status (phase 4 of 6, beta):** every page from 1.x is ported and shows the same numbers on the
same data. 2.0.0-beta.1 is packaged as a flatpak (same app ID as 1.x, so it upgrades in place) and
a Windows installer plus portable zip, and published as a GitHub pre-release. 1.x stays the
"latest" release until the beta has been checked on real data; then 2.0.0 replaces it.

## Layout

| Path | What |
| --- | --- |
| `src-tauri/` | The backend. The only part that opens the database, the network or files. Each command in `src/lib.rs` is listed in `build.rs` and allowed in `capabilities/default.json`; nothing else is reachable from the webview. |
| `src-tauri/src/secrets.rs` | The FRED key: OS keychain, or a private file when there is none. |
| `ui/` | The interface: Leptos (client-side), compiled to WebAssembly by Trunk. `src/ui/` holds the Rust/UI components with the same class strings as 1.x, `src/pages/` the pages. |
| `../crates/penguinfin-core/src/ipc.rs` | The types both sides exchange (`Snapshot`, `Boot`, `ImportPreview`…). Every edit is one `Change` sent to the backend's `apply` command; the reply is the new `Snapshot`. |
| `ui/src/charts.rs` | The line and stacked charts. Pointer events, so dragging a finger reads them too. |

Calculations (projections, payoff plans, goal plans) run in the interface: the engine crate
compiles to WebAssembly without its network client, and each page computes its plan in a memo
that re-runs only when the data changes. Nothing is sent to the backend just to calculate.

This is its own Cargo workspace with its own `Cargo.lock`: Dioxus (1.x) and Tauri pin different
versions of the same native webview libraries, so they can't share one.

## Build

One-time setup:

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cargo install --locked tauri-cli --version '^2'
```

On Linux you also need the WebKitGTK 4.1, libsoup 3 and D-Bus development packages (the same
ones 1.x needs, plus `libdbus-1-dev` for the keychain).

Styles come from the shared Tailwind build. After changing classes in `ui/src`, rebuild it from
the repository root:

```bash
npx @tailwindcss/cli -i style/app.css -o assets/app.css --minify
```

Build the app (runs `trunk build --release` for the interface first):

```bash
cd app/src-tauri && cargo tauri build --no-bundle
```

The binary is `app/target/release/penguinfin-app`. During development, `cargo tauri dev` serves
the interface with live reload.

## Trying it on a copy of your data

PenguinFin 2 upgrades the database it opens (it saves a dated copy first). To try it without
touching your real data, point it at a copy:

```bash
PENGUINFIN_DB=/tmp/pf2/penguinfin.db XDG_DATA_HOME=/tmp/pf2/xdg app/target/release/penguinfin-app
```

`PENGUINFIN_NO_KEYCHAIN=1` keeps the FRED key in a file instead of the system keychain.

## Packaging and releases

```bash
app/build-flatpak.sh
```

builds `dist/penguinfin-2.flatpak` (Tailwind, `trunk build --release`, `cargo vendor` into
`app/vendor`, then an offline `flatpak-builder` run with `flatpak/io.github.tobeoffline64.PenguinFin.v2.yml`).
On Windows, `cargo tauri build` in `app/src-tauri` makes the NSIS installer in
`app/target/release/bundle/nsis/`.

`.github/workflows/release-v2.yml` publishes a release whenever the version in `app/Cargo.toml`
has no release yet. A version with a suffix (`2.0.0-beta.1`) is published as a pre-release and
never marked latest. The notes come from `.github/release-notes-v2.md`. CI (`ci.yml`, the `app-*`
jobs) runs the tests and clippy for both the backend and the WebAssembly interface, and builds on
Windows.

### Automatic updates (not enabled yet)

Tauri's updater only installs updates signed with the project's key. To turn it on, the
maintainer generates the key pair on their own computer (`cargo tauri signer generate -w
~/.tauri/penguinfin.key`), stores the private key and its password as the repository secrets
`TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and commits only the public
key into `tauri.conf.json`. The private key never goes into the repository. Flatpak installs
update through flatpak, not this updater.
