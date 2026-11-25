**PenguinFin 2 beta.** This is a pre-release; PenguinFin 1.10 stays the current release (1.x is archived on the `archive/v1` branch). The new interface is built with Tauri and Leptos. On the same data it shows the same numbers as 1.x, with every page carried over. User guide: https://github.com/tobeoffline64/PenguinFin/wiki

## What's new

- A new, faster interface: calculations run inside the window, so pages recalculate instantly.
- Your FRED API key is moved into the desktop keychain (Secret Service on Linux, Credential Manager on Windows). It is never put in backups or exports.
- Native Save and Open dialogs for backups, exports and restores.
- Checked `.pfexport` files: a copy of your data with a checksum, for moving to another computer (and later to the phone apps).
- Restore accepts a 1.x JSON backup, a `.pfexport` file or a `penguinfin.db` from another computer, and shows what it contains before replacing anything.

## Before you install: make a backup

PenguinFin 2 upgrades your data the first time it opens it. It saves a copy of the old file first, but an extra backup costs nothing: in PenguinFin 1.10, open **Settings & data → Export a checked copy (.pfexport)** (or **Export backup**).

## Install on Linux (replaces 1.x, same app)

```bash
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user -y --reinstall penguinfin.flatpak
flatpak run io.github.tobeoffline64.PenguinFin
```

Run these in the folder where you saved `penguinfin.flatpak`. It has the same app ID as 1.x, so your menu entry and data carry over.

## Install on Windows

- **Installer:** download `PenguinFin-<version>-windows-x64-setup.exe` and run it. It adds a Start menu entry and an uninstaller.
- **Portable:** download `PenguinFin-<version>-windows-x64-portable.zip`, extract it and run **PenguinFin.exe**.

Windows may show *"Windows protected your PC"*, because the app isn't code-signed. Click **More info → Run anyway**.

Both versions use the same data as 1.x (`%APPDATA%\PenguinFin\penguinfin.db`).

## What happens to your 1.x data

On first launch, PenguinFin 2:

1. Saves a copy named `penguinfin-1.x-backup-<date>-<time>.db` next to your database.
2. Upgrades the database to the new format (format 8). If anything fails, it puts the copy back and tells you.
3. Moves your FRED key, if you had one, into the keychain.

**Settings & data** then shows *"Upgraded from PenguinFin 1"*.

Your database is in:

- **Linux (flatpak):** `~/.var/app/io.github.tobeoffline64.PenguinFin/data/penguinfin/`
- **Windows:** `%APPDATA%\PenguinFin\`

## Going back to 1.x

PenguinFin 1.10 refuses to open the upgraded file; it says the file needs a newer version rather than risk damaging it. To go back:

1. Close PenguinFin.
2. In the data folder above, rename `penguinfin.db` (for example to `penguinfin-2.db`).
3. Rename `penguinfin-1.x-backup-<date>-<time>.db` to `penguinfin.db`.
4. Reinstall 1.10.0 from its release page (`flatpak install --user -y --reinstall penguinfin.flatpak`, or the Windows zip).

Changes you made in 2.0 aren't in the 1.x copy. Older versions than 1.10 can't read `.pfexport` files, but **Restore** in 1.10 can.

## Known limits of the beta

- No automatic updates yet. Install new betas from this page the same way.
- macOS, iPhone and Android builds come later.

Please report anything that looks different from 1.x: https://github.com/tobeoffline64/PenguinFin/issues
