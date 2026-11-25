# Troubleshooting

### The window is blank or flickers (Linux)
Some graphics drivers (especially NVIDIA on Wayland) have problems with WebKit's GPU path. Start in
compatibility mode:
```bash
flatpak run --env=WEBKIT_DISABLE_DMABUF_RENDERER=1 io.github.tobeoffline64.PenguinFin
```

### The window doesn't open (Windows)
Install the Microsoft Edge WebView2 Runtime (*Evergreen Standalone Installer*) from
<https://developer.microsoft.com/microsoft-edge/webview2/>.

### "Windows protected your PC"
PenguinFin isn't code-signed. Click **More info → Run anyway**.

### Scrolling goes the wrong way
Toggle **Settings → Natural scrolling**. Leave it off if your system already uses natural scrolling.

### Everything is too small or too large
Use the text-size control at the bottom of the sidebar, or **Ctrl +** / **Ctrl −** / **Ctrl 0**.

### FRED sync fails
Check the key in **Settings → Economic data**: it's 32 lowercase letters and digits. The last error is
shown under **Sync**. Your previous values stay available.

### "This version … is already installed" when updating the flatpak
You already have that version.

### PenguinFin 1.x says the data file needs a newer version
PenguinFin 2 upgraded the file. To go back to 1.x, see the "Going back to 1.x" steps in the
[2.0 release notes](https://github.com/tobeoffline64/PenguinFin/releases/tag/v2.0.0-beta.1): rename the
saved `penguinfin-1.x-backup-<date>.db` copy to `penguinfin.db`.
