# Installation

Download the latest version from the [Releases page](https://github.com/tobeoffline64/PenguinFin/releases/latest).

## Linux (flatpak)

1. Make sure Flathub is set up (it provides the GNOME runtime PenguinFin uses):
   ```bash
   flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
   ```
2. Install the downloaded file:
   ```bash
   flatpak install --user penguinfin.flatpak
   ```
3. Start it from your app grid, or with:
   ```bash
   flatpak run io.github.tobeoffline64.PenguinFin
   ```

Your data lives in `~/.var/app/io.github.tobeoffline64.PenguinFin/data/penguinfin/penguinfin.db`.

### Updating

Your data is kept. With the GitHub CLI:
```bash
gh release download --repo tobeoffline64/penguinfin --pattern penguinfin.flatpak --dir /tmp --clobber
flatpak install --user -y --reinstall /tmp/penguinfin.flatpak
```
Or download `penguinfin.flatpak` and run `flatpak install --user -y --reinstall penguinfin.flatpak`.
If flatpak says *"This version … is already installed"*, you're up to date.

## Windows

1. Download `PenguinFin-<version>-windows-x64.zip` and extract it anywhere.
2. Double-click **PenguinFin.exe**.

Windows may show *"Windows protected your PC"* because the app isn't code-signed: click
**More info → Run anyway**.

PenguinFin needs the Microsoft Edge **WebView2 Runtime**, which is built into Windows 11 and almost every
up-to-date Windows 10. If the window doesn't open, install the *Evergreen Standalone Installer* from
<https://developer.microsoft.com/microsoft-edge/webview2/>.

Your data lives in `%APPDATA%\PenguinFin\penguinfin.db`. To update, replace `PenguinFin.exe` with the
one from the newer zip.

## Moving between computers

Use **Settings → Your data → Export backup** on the old computer and **Restore** on the new one. Backups
are plain JSON files and work across Linux and Windows. See [Settings & backups](Settings-and-Backups).
