PenguinFin 2 for Windows (beta)
===============================

Offline personal finance planner: bills, recurring expenses, income, accounts,
debt payoff, savings goals and projections. https://github.com/tobeoffline64/PenguinFin

This is the portable copy. The installer (PenguinFin_<version>_x64-setup.exe) on
the Releases page adds a Start menu entry and an uninstaller instead.

Running it
----------
1. Extract this folder anywhere (for example Documents\PenguinFin).
2. Double-click PenguinFin.exe.

Windows may show "Windows protected your PC" the first time because the app isn't
code-signed. Click "More info" and then "Run anyway".

PenguinFin needs the Microsoft Edge WebView2 Runtime, which is built into Windows 11
and up-to-date Windows 10.

Your data
---------
Everything is stored on this computer in:
  %APPDATA%\PenguinFin\penguinfin.db
the same file PenguinFin 1 used. The first time PenguinFin 2 opens it, it saves a
copy named penguinfin-1.x-backup-<date>.db in the same folder and then upgrades it.
PenguinFin 1 can't open the upgraded file; to go back to 1.x, rename that copy to
penguinfin.db.

Your FRED key (if you use one) is kept in Windows Credential Manager.
