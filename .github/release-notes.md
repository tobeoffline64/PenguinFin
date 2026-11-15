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

## New in 1.4.0
- **Savings goals.** List things you want to buy with their price, an optional "want it by" date, and a priority order. PenguinFin shows when each becomes affordable from checking and savings without dipping into your minimum balances, now or later in the projection, plus how much more to save per month if a date is too soon, and a chart of spendable cash after each purchase.
- **Account purposes and minimum balances.** Give any account a purpose (Medical, Food, Emergency fund…) and a minimum balance to keep. Accounts below their minimum are flagged; goals never spend those baselines.

## New in 1.3.0
- **Economic data from FRED (optional).** Add a free API key in Settings → Economic data to pull 10-year expected inflation, current CPI inflation, the fed funds rate, the 10-year Treasury yield, the 30-year mortgage rate and the S&P 500's 10-year average return.
  - Syncs in the background when PenguinFin starts (can be turned off) and on demand with **Sync now**. Startup and the UI never wait for it.
  - Your inflation assumption can follow FRED automatically.
  - Account forms show live guidance (fed funds rate for savings APY, S&P 500 return for investments), and Projections shows a market-context card.
  - Only these public figures are downloaded; nothing about your finances is sent. The API key is never included in backups.

## New in 1.2.0
- **Faster, more responsive UI.** Runs natively on Wayland with WebKit's GPU buffer path (previously forced through XWayland with it turned off), cheaper shadows that are much faster to paint, and forms that no longer re-render the whole page as you edit them.
- **Account form adapts to the account type**: e.g. savings asks for APY and monthly transfers, a 401(k) for paycheck contributions and employer match, other assets for value and yearly change.
- **Pages open at the top** instead of where you last scrolled.
- If the window is ever blank or flickers on your graphics driver, start it with `flatpak run --env=PENGUINFIN_COMPAT_RENDERING=1 io.github.tobeoffline64.PenguinFin`.

## New in 1.1.1
- Dropdown menus now match the app's light/dark theme. On a dark desktop they were drawn dark even in light mode.

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
