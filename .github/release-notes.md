PenguinFin is an offline personal finance planner for Linux and Windows. User guide: https://github.com/tobeoffline64/PenguinFin/wiki

## Install on Windows

Download `PenguinFin-<version>-windows-x64.zip` below, extract it and run **PenguinFin.exe**. Windows may show *"Windows protected your PC"* because the app isn't code-signed: click **More info → Run anyway**. It needs the Microsoft Edge WebView2 Runtime (built into Windows 11 and up-to-date Windows 10). To update, replace `PenguinFin.exe` with the new one; your data in `%APPDATA%\PenguinFin` is kept.

## Install on Linux (Ubuntu / any distro with Flatpak)

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

## New in 1.10.0
- **Export for PenguinFin 2.** Settings › Your data has a new button that saves a `.pfexport` file: a checked copy of all your data (accounts, buckets, bills, income, debts and payment history, goals, settings) that PenguinFin 2 will import on any computer or phone. It works as a backup today, and never includes your FRED key.
- **Safer restore.** Restore now accepts a backup, an export, or a `penguinfin.db` copied from another computer, and shows what the file holds before it replaces anything. Damaged exports are refused.
- PenguinFin 1.x now refuses to open a database saved by a newer version, so it can't damage data after you move to PenguinFin 2.
- Fix: after exporting, the restore box now shows the file you just saved.

## New in 1.9.0
- **Log one-time payments toward a debt.** Each debt on the Debt payoff page has a **Log a payment** button: enter the amount, date, the account it came from and an optional note (e.g. "tax refund"). The debt's balance drops right away, and the money is subtracted from that account too (switch that off if you've already updated the account's balance). The payoff plan, debt-free date, buckets and dashboard update immediately.
- **Schedule a future payment.** Pick a date in the future and the payment is planned instead: your payoff plan, schedule and projections include it now (paid from the chosen account in that month). Once the date passes, the debt asks "Did you make it?" with **Log it** or **Remove**.
- **Payment history** card lists every one-time payment; **Undo** on any of them puts the debt and account balances back. Payments are included in JSON backups.
- Chart date labels no longer overlap at the right edge.

## New in 1.8.0
- **Debt payments are set aside automatically.** Adding a debt now creates a "… payment" bucket in the account that pays it (switch it off on the form if you don't want one; existing debts get a "Set aside payments in a bucket" button). The bucket always holds that debt's planned payment for the month (minimum, plus the extra when it's the current target) and updates by itself when balances, the extra amount or the strategy change. Bucket money is reserved, so savings goals won't spend it.
- In projections each debt bucket follows the payoff plan and empties when the debt is paid off; the Accounts & buckets chart shows it.
- Accounts list their planned debt payments under "Pays".
- **Debt on the dashboard.** The overview chart shows a Debt line and your debt-free date, and a new Debt payoff card summarises what you owe, your monthly payment, interest, and each balance month by month.
- **Fix:** the app could crash right after adding or removing a debt on the Debt payoff page.

## New in 1.7.0
- **Debt payoff** (new page). Add credit cards and loans with their balance, APR and minimum payment to see the interest you pay each month, how long minimum payments would take, and the payment needed to be debt-free in 1, 2 or 3 years.
- **A payoff plan that still pays rent and utilities.** Your take-home pay minus bills, expenses and savings shows what's actually available; choose an extra monthly amount (with a warning if it's more than you have) and **avalanche** (highest rate first) or **snowball** (smallest balance first). Paid-off debts roll their payment into the next. You get a debt-free date, interest saved vs. minimums only, a payoff chart and a month-by-month schedule.
- Debt now counts against net worth, is paid from its account in projections, and the dashboard's "Left over" includes your debt plan.

## New in 1.6.0
- **Windows build**: a portable `PenguinFin-1.6.0-windows-x64.zip` is now attached to every release. Data is stored in `%APPDATA%\PenguinFin`, backups go to your Downloads folder.
- The penguin icon now shows on the window and taskbar (and on the Windows `.exe`).
- The sample household now includes minimum balances, account purposes and savings goals.
- New **[wiki](https://github.com/tobeoffline64/PenguinFin/wiki)** with a user guide and screenshots.

## New in 1.5.0
- **Buckets.** Split any account into named portions such as *Rainy day money* or *Car repairs*: an amount set aside now, an optional monthly top-up and an optional target. Unallocated money is shown per account. Bucket money is reserved, so savings goals won't spend it.
- **Choose where money comes from and goes.** Each bill and expense has a **Paid from** setting (any account, or a bucket inside one) and each income a **Deposited to** setting. Accounts list what they pay and receive each month.
- **Accounts & buckets chart** (Projections): pick an account to see its balance stacked by bucket and unallocated money over 2 years, 5 years or the whole projection, with its minimum balance and warnings when a bucket runs dry, the account drops below its minimum, or buckets exceed the balance.

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
- `penguinfin.flatpak`: Linux bundle (x86_64)
- `PenguinFin-<version>-windows-x64.zip`: Windows (x64), portable
- `penguinfin-<version>-source.tar.gz`: source code
