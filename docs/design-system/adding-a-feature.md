# Adding a feature

The order that has worked for every feature so far. File paths are in the PenguinFin repository.

1. **Model** (`src/model.rs`). Add the struct or fields. New fields on `AppData` and `Settings` need `#[serde(default)]` so old JSON backups still load. Add helpers that views will call (totals, lookups) here, not in views.
2. **Database** (`src/db.rs`). Bump `SCHEMA_VERSION`, add one migration step that creates the table or column, and add the read and write methods. Anything that changes two balances runs in one transaction. Update `replace_all` so restoring a backup keeps the new data without applying it twice.
3. **Logic** in its own module (`src/debt.rs`, `src/goals.rs`, `src/projection.rs`). Pure functions over `AppData` and a date, with unit tests. If the feature moves money, it must also appear in `project()` so projections, goals and the dashboard agree.
4. **Screen** (`src/views/…`). Build it from the Patterns page: `PageHeader`, `Stat` tiles, list and form cards, charts. Reuse `src/ui` and `src/widgets.rs` before writing new markup. A new page also needs a `Page` variant, a nav entry with an icon in `src/app.rs`, and a boot-skeleton row in `src/main.rs`.
5. **Other screens.** Check whether the dashboard, projections, accounts or goals should show the new thing. A number that appears on two screens must come from the same helper.
6. **Styles.** New Tailwind classes only take effect after `npx @tailwindcss/cli -i style/app.css -o assets/app.css --minify`. Commit `assets/app.css`.
7. **Check.** `cargo test`, `cargo clippy --all-targets`, then run the app and use the feature: light and dark, Text size S and XL, a narrow window. Upgrade from a database made by the previous version.
8. **Words.** Add the feature to `README.md`, the matching page in `docs/wiki/`, `.github/release-notes.md` under a new "New in x.y.z" heading, and a release entry in the flatpak metainfo. Bump `version` in `Cargo.toml`; pushing that to `main` publishes the release.

## Before calling a screen done

- One `page-title`, one solid primary button per card.
- Every amount uses `money`, `money_cents` or `money_short` with the settings currency symbol, and has `tabular-nums`.
- No literal colours, no `px` sizes, no viewport breakpoints (`md:`, `lg:`); container queries only.
- Loading, empty and error states exist. Deleting confirms in place. Saving shows a toast.
- Icon-only buttons have `title` and `aria_label`. A status has a word, not only a colour.
- Typing in a field does not re-render the page, and a calculated value does not block the first frame.
