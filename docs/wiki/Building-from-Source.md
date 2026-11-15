# Building from source

PenguinFin is written in Rust with [Dioxus](https://dioxuslabs.com) (desktop, WebView) and components from
[Rust/UI](https://github.com/rust-ui/ui), styled with Tailwind CSS v4. Data is stored with SQLite
(rusqlite).

## Linux

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libsoup-3.0-dev
cargo run --release
cargo test
```

Flatpak bundle (needs `flatpak`, `flatpak-builder` and Rust):

```bash
./build-flatpak.sh            # fast incremental build (~15 s after the first)
./build-flatpak.sh --release  # with link-time optimisation
./build-flatpak.sh --clean    # from scratch, like the release build
```

## Windows

Install Rust (MSVC toolchain) with [rustup](https://rustup.rs), then:

```powershell
cargo build --release
.\target\release\penguinfin.exe
```

## Styles

The compiled CSS (`assets/app.css`) is committed. After changing Tailwind classes:

```bash
npm install
npx @tailwindcss/cli -i style/app.css -o assets/app.css --minify
```

## Developer options

| Variable | Effect |
|---|---|
| `PENGUINFIN_DB=/path/file.db` | Use another database |
| `PENGUINFIN_WINDOW_SIZE=900x700` | Initial window size |
| `PENGUINFIN_PERF=1` | Print input-to-frame latency |
| `PENGUINFIN_SLOW=1` (debug builds) | Delay background work to inspect loading states |
| `PENGUINFIN_FRED_URL` | Point the FRED client at another server |
| `PENGUINFIN_COMPAT_RENDERING=1` | Linux: XWayland, no DMA-BUF (for problem GPU drivers) |

`cargo run --example seed -- /path/file.db` fills a database with the sample household.

## Releases

Bump `version` in `Cargo.toml` (and the release entry in `flatpak/*.metainfo.xml`), update
`.github/release-notes.md`, and push to `main`. GitHub Actions builds the flatpak and the Windows zip
and publishes `v<version>`. CI runs tests and a Windows build on every push. The wiki is published from
`docs/wiki/`.
