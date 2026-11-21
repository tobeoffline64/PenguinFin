# Settings & backups

## Preferences & interface

- **Currency symbol**, **inflation %** (or follow FRED) and **default projection length**.
- **Dark mode**.
- **Interface size**: *Auto* scales everything to the window; or S / M / L / XL.
  Shortcuts: **Ctrl +**, **Ctrl −**, **Ctrl 0**.
- **Natural scrolling**: reverse wheel and touchpad direction (leave off if your system already does this).

![Dark mode](https://raw.githubusercontent.com/tobeoffline64/PenguinFin/main/docs/screenshots/dark-savings-goals.png)

## Your data

All data is in one SQLite file:

| System | Location |
|---|---|
| Linux (flatpak) | `~/.var/app/io.github.tobeoffline64.PenguinFin/data/penguinfin/penguinfin.db` |
| Windows | `%APPDATA%\PenguinFin\penguinfin.db` |

- **Export backup** saves a JSON copy to your Downloads folder.
- **Export for PenguinFin 2** saves a `.pfexport` file to your Downloads folder. It is a checked copy of
  all your data (it records which version made it and a checksum), for moving to PenguinFin 2 on any
  computer or phone. It works as a backup too.
- **Restore** replaces all data with a backup (`.json`), an export (`.pfexport`) or a `penguinfin.db`
  copied from another computer (works across Linux and Windows). It first shows what the file holds
  ("5 accounts, 15 income, bills and expenses, 3 debts…") and only replaces your data when you confirm.
  A damaged export is refused before anything changes.
- **Replace with sample data** / **Erase all data** for starting over.

Backups and exports include accounts, holdings, buckets, bills, income, expenses, debts, debt payments,
goals and preferences. They don't include your FRED API key.

PenguinFin 1.10 and later refuse to open a database saved by a newer version (PenguinFin 2), so going back
to an older version can't damage it. Restore a backup instead.
