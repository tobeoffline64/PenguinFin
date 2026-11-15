PenguinFin is an offline personal finance planner for Linux.

## Install (Ubuntu / any distro with Flatpak)

```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user penguinfin.flatpak
flatpak run io.github.tobeoffline64.PenguinFin
```

The first install also pulls the GNOME 49 runtime from Flathub.

## Update an existing install

Your data is kept. With the [GitHub CLI](https://cli.github.com) (`sudo apt install gh`, then `gh auth login` once):

```bash
gh release download --repo tobeoffline64/penguinfin --pattern penguinfin.flatpak --dir /tmp --clobber
flatpak install --user -y --reinstall /tmp/penguinfin.flatpak
```

Or download `penguinfin.flatpak` from this page and run `flatpak install --user -y --reinstall penguinfin.flatpak` in the folder you saved it to.

If flatpak says *"This version … is already installed"*, you already have the latest version.

If the repository is public, this also works without the GitHub CLI:

```bash
curl -fL -o /tmp/penguinfin.flatpak https://github.com/tobeoffline64/penguinfin/releases/latest/download/penguinfin.flatpak
flatpak install --user -y --reinstall /tmp/penguinfin.flatpak
```

## New in 1.1.0
- **Interface size**: Auto (scales with the window) or S / M / L / XL, from the sidebar, Settings, or Ctrl + / Ctrl − / Ctrl 0
- **Responsive layout**: content reflows to the available space; the sidebar collapses to icons when space is tight
- **Natural scrolling** option (Settings → Preferences & interface)
- **Loading skeletons** at startup and while projections are calculated

## Features
- Income, bills and recurring expenses on any schedule (weekly → yearly, or one-time), with end dates, yearly raises/price changes and autopay
- Checking, savings, brokerage, 401(k), IRA, HSA and pension accounts, with contributions, employer match and manually priced holdings
- Dashboard: net worth, monthly cash flow, savings rate, next 30 days of payments, spending by category
- Projections up to 80 years, in nominal or today's dollars, with a cash-shortfall warning and what-if scenarios
- Local SQLite storage, JSON backup/restore, dark mode

## Notes
- Data lives in `~/.var/app/io.github.tobeoffline64.PenguinFin/data/penguinfin/penguinfin.db`.
- The flatpak has the network permission only because the Dioxus webview talks to the app over a local 127.0.0.1 socket. PenguinFin makes no outbound requests.
- Stock prices are entered by hand in this version; a live quote API is planned.

## Files
- `penguinfin.flatpak`: installable bundle (x86_64)
- `penguinfin-1.0.0-source.tar.gz`: source code
