# PenguinFin

Offline personal finance planner for Linux. Track income, bills and recurring expenses, add your
checking/savings/brokerage/retirement accounts, and see month-by-month projections of where your
money goes over the next decades.

Built in **Rust** with **[Dioxus](https://dioxuslabs.com) desktop** and components from
**[Rust/UI](https://github.com/rust-ui/ui)** (Tailwind CSS v4). Data is stored in a single local
**SQLite** file. The app makes no network requests.

## Features

- **Income, Bills, Recurring expenses**: weekly, every 2 weeks, twice a month, monthly, quarterly,
  twice a year, yearly or one-time; start/due date, optional end date (for example when a car loan ends),
  yearly raise or price change, and autopay or direct-deposit flag.
- **Dashboard**: net worth, monthly income vs. outflow, leftover cash, savings rate, projected net worth,
  the next 30 days of payments, and spending by category.
- **Accounts & investments**: checking, savings, brokerage, 401(k)/403(b), Traditional/Roth IRA, HSA,
  pension, other. Each account has an expected return/APY, a monthly contribution (from checking or via
  payroll), employer match, and optional **holdings** (symbol × shares × price).
- **Savings goals**: things to buy with a price, optional "want it by" date and priority. Shows when each becomes
  affordable from checking + savings while keeping every account's minimum balance, even through later bills in the
  projection; how much more to save per month to hit a date; and spendable cash over time after the purchases.
- **Account purposes & minimum balances**: tag accounts (Medical, Food, Emergency fund…) and set an operating
  baseline; accounts below it are flagged and goals never spend it.
- **Projections**: 1–80 years, nominal or inflation-adjusted ("today's dollars"), milestone cards,
  a year-by-year table, a warning when cash would run short, and **what-if** scenarios (extra income,
  extra spending, extra investing, lower/higher returns) that are not saved.
- **Settings & data**: currency symbol, inflation, dark mode, JSON backup/restore (in `~/Downloads`),
  sample data, erase all.
- **Interface**: UI size *Auto* (scales with the window) or S / M / L / XL, from the sidebar, Settings,
  or Ctrl + / Ctrl − / Ctrl 0. The layout reflows to the space it has, and the sidebar collapses to icons
  when it's tight. Optional **natural scrolling** (reversed wheel/touchpad direction).
- **Economic data (FRED, optional)**: with a free API key, pulls expected and current inflation, the fed
  funds rate, 10-year Treasury, 30-year mortgage rate and the S&P 500's 10-year return. Syncs in the background
  on launch and on demand (Settings → Economic data); inflation can follow FRED automatically; account forms and
  Projections show the figures as guidance. Without a key the app makes no network requests.
- **Loading skeletons** while the window starts up and while projections are calculated (shown only if a
  calculation takes more than ~150 ms, so quick updates don't flicker).

### How projections work
Each month: every account grows by its own expected return, then receives its contribution and match.
Income minus bills, expenses and contributions "from checking" lands in your first checking account.
Each cash-flow amount changes by its own yearly % once per elapsed year. Paydays and due dates are
counted exactly, so a biweekly paycheck lands three times in some months.

### Stock prices
In v1, prices are typed in by hand. `src/prices.rs` defines a `PriceProvider` trait, so a live quote
API can be added later without touching storage or the UI.

## Install the flatpak

Download `penguinfin.flatpak` from the [latest release](https://github.com/tobeoffline64/PenguinFin/releases/latest), then:

```bash
flatpak install --user penguinfin.flatpak
flatpak run io.github.tobeoffline64.PenguinFin
```

If Flathub isn't set up on your machine yet, add it first so the GNOME 49 runtime can be installed:

```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
```

The flatpak has the `network` permission because the Dioxus webview talks to the app over a local
`127.0.0.1` socket, and WebKit's sandboxed network process can't reach it otherwise (the window stays
blank). PenguinFin makes no outbound requests.

### Updating

Your data is kept. With the [GitHub CLI](https://cli.github.com) (`sudo apt install gh`, then `gh auth login` once):

```bash
gh release download --repo tobeoffline64/penguinfin --pattern penguinfin.flatpak --dir /tmp --clobber
flatpak install --user -y --reinstall /tmp/penguinfin.flatpak
```

Or download `penguinfin.flatpak` from the [latest release](https://github.com/tobeoffline64/penguinfin/releases/latest) and run `flatpak install --user -y --reinstall penguinfin.flatpak` in the folder you saved it to.

If flatpak says *"This version … is already installed"*, you already have the latest version.

If the repository is public, this also works without the GitHub CLI:

```bash
curl -fL -o /tmp/penguinfin.flatpak https://github.com/tobeoffline64/penguinfin/releases/latest/download/penguinfin.flatpak
flatpak install --user -y --reinstall /tmp/penguinfin.flatpak
```

If the window is blank or flickers (some graphics drivers, especially NVIDIA on Wayland), start it in
compatibility mode: `flatpak run --env=PENGUINFIN_COMPAT_RENDERING=1 io.github.tobeoffline64.PenguinFin`.

Your database is at `~/.var/app/io.github.tobeoffline64.PenguinFin/data/penguinfin/penguinfin.db`.

## Releasing

`.github/workflows/release.yml` builds the flatpak on GitHub Actions and publishes a release.
Bump `version` in `Cargo.toml` (and `flatpak/*.metainfo.xml`), update `.github/release-notes.md`,
and push to `main`. The workflow creates the `v<version>` tag and release with `penguinfin.flatpak`
and the source archive attached.

## Build from source

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libxdo-dev libsoup-3.0-dev   # Ubuntu deps
npm install                                       # Tailwind CLI (only to change styles)
npx @tailwindcss/cli -i style/app.css -o assets/app.css --minify
cargo run --release
cargo test
```

`PENGUINFIN_DB=/path/to/file.db cargo run` uses a different database.
`PENGUINFIN_WINDOW_SIZE=900x700` sets the initial window size (handy for checking layouts), and in debug
builds `PENGUINFIN_SLOW=1` delays background work so loading states can be inspected.
`PENGUINFIN_PERF=1` prints input-to-frame latency for each click/keystroke to stderr. It also reports when the first frame was committed.
`PENGUINFIN_FRED_URL` points the FRED client at another server (e.g. a local mock); `cargo run --example fred_check -- <key>`
fetches every indicator from the command line.
`cargo run --example seed -- /path/to/file.db` fills one with sample data.

Build the flatpak bundle (needs `sudo apt install flatpak flatpak-builder` and Rust from rustup.rs):

```bash
./build-flatpak.sh            # fast: incremental, no LTO
./build-flatpak.sh --release  # incremental with the release profile (thin LTO)
./build-flatpak.sh --clean    # from scratch in flatpak-builder (what the GitHub release uses)
```

Typical times on a 4-core machine:

| | First build | After a code change | No change |
|---|---|---|---|
| default (fast) | ~3.5 min | ~15 s | ~8 s |
| `--release` | ~3.5 min | ~40 s | — |
| `--clean` | ~3.5 min | ~3.5 min | ~3.5 min |

The fast modes compile with cargo inside the same GNOME SDK flatpak-builder uses, but keep cargo's
output in `target/flatpak/` (and link with `mold`), then let flatpak-builder package the binary.
Crates are re-vendored only when `Cargo.lock` changes, and Flathub is only contacted if a runtime is
missing. State lives in `.flatpak-dev/`; delete it (and `target/flatpak/`) to start over.

For quick UI work you can skip flatpak entirely: `cargo run` builds a debug binary and rebuilds in seconds.

## Layout

| Path | What |
| --- | --- |
| `src/model.rs` | Domain types, schedule math (occurrence dates) |
| `src/projection.rs` | Monthly projection engine, upcoming payments, category totals |
| `src/db.rs` | SQLite schema, migrations, CRUD, JSON backup |
| `src/goals.rs` | Savings-goal planner (affordability under minimum balances) |
| `src/fred.rs` | FRED API client: indicators, parallel fetch, parsing |
| `src/prices.rs` | Price provider trait (manual in v1) |
| `src/ui/` | Rust/UI components (copied from the registry, MIT; see `LICENSE-rust-ui`) |
| `src/views/` | Dashboard, cash flows, accounts, projections, settings |
| `style/app.css` | Tailwind source with Rust/UI theme tokens; compiled to `assets/app.css` |
| `flatpak/` | Manifest, desktop file, AppStream metadata, icon |

Small changes to the vendored Rust/UI components: `Button` dropped its router `href` mode, `Switch`
became a controlled component with `onchange`, and `Input` is uncontrolled. It reports its value on
commit (blur or Enter), so typing never re-renders the page.
