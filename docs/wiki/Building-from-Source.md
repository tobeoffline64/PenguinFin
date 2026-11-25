# Building from source

PenguinFin 2 is written in Rust: a [Tauri 2](https://tauri.app) backend and a [Leptos](https://leptos.dev)
interface compiled to WebAssembly, with components from [Rust/UI](https://github.com/rust-ui/ui), styled
with Tailwind CSS v4. Data is stored with SQLite (rusqlite). The 1.x app (Dioxus) is archived on the
`archive/v1` branch.

## One-time setup

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cargo install --locked tauri-cli --version '^2'
npm ci      # Tailwind CLI
```

On Ubuntu, also:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libdbus-1-dev
```

On Windows, install Rust with the MSVC toolchain from [rustup](https://rustup.rs).

## Run and test

```bash
cd src-tauri
cargo tauri dev       # live reload
cargo tauri build     # release build (and the installer on Windows)
cargo test --workspace
```

Flatpak bundle (needs `flatpak` and `flatpak-builder`):

```bash
./build-flatpak.sh    # writes dist/penguinfin.flatpak
```

## Styles

The compiled CSS (`assets/app.css`) is committed. After changing Tailwind classes:

```bash
npx @tailwindcss/cli -i style/app.css -o assets/app.css --minify
```

## Developer options

| Variable | Effect |
|---|---|
| `PENGUINFIN_DB=/path/file.db` | Use another database |
| `PENGUINFIN_NO_KEYCHAIN=1` | Keep the FRED key in a private file instead of the system keychain |
| `PENGUINFIN_FRED_URL` | Point the FRED client at another server |

`cargo run -p penguinfin-store --example seed -- /path/file.db` fills a database with the sample household.

## Releases

Bump `version` in `Cargo.toml` (and add a release entry in `flatpak/*.metainfo.xml`), update
`.github/release-notes.md`, and push to `main`. GitHub Actions builds the flatpak, the Windows installer
and portable zip, and publishes `v<version>` (a pre-release when the version has a suffix such as
`-beta.1`). CI runs tests and clippy and a Windows build on every push. The wiki is published from
`docs/wiki/`.
