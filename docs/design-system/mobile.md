# Mobile: iPhone and Android

A plan for bringing PenguinFin to phones without forking the product. Nothing here is built yet. The phone apps are the same app: the same Rust engine, the same SQLite file format, the same tokens and components, arranged for a thumb instead of a mouse.

## What stays the same

- **One codebase.** Dioxus 0.7 builds iOS and Android apps (`dx bundle --platform ios` / `android`). They render in the system webview (WKWebView on iOS, Android System WebView), the same way the desktop app renders in WebKitGTK and WebView2. `model.rs`, `db.rs`, `projection.rs`, `debt.rs`, `goals.rs` and `fred.rs` compile unchanged.
- **Offline and private.** The database lives in the app's own sandbox (`Application Support` on iOS, `filesDir` on Android) and never leaves the phone unless the person exports it. No account, no cloud, no analytics. The only network call stays the optional FRED sync.
- **Same data file.** A JSON backup made on the desktop restores on the phone and back. Moving data between devices is an export and an import; there is no sync service.
- **Same tokens.** Every colour, radius and text style in this system applies. Mobile adds the size tokens in the `mobile-*` and `touch-*` names and nothing else.
- **Same words.** Content rules from the README hold: "you", sentence case, money via `money()`, toasts in the past tense.

## Navigation

The desktop sidebar has nine pages. A phone gets five tabs in a bottom bar, `mobile-tab-bar` tall plus the home-indicator inset:

| Tab | Icon | Holds | Desktop pages |
|---|---|---|---|
| Home | `Dashboard` | Stats, net-worth chart, next 30 days, debt summary | Dashboard |
| Cash flow | `Income` | A segmented control: Income, Bills, Expenses | Income, Bills, Recurring expenses |
| Accounts | `Accounts` | Accounts, buckets, holdings | Accounts & investments |
| Plan | `Chart` | A segmented control: Debt, Goals, Projections | Debt payoff, Savings goals, Projections |
| More | `Settings` | Preferences, Your data, Economic data, About | Settings & data |

- The tab bar is always visible except while a sheet is open or the keyboard is up.
- Inside a tab, drilling into an item (a debt, an account) pushes a detail screen with a back control. Never more than two levels deep.
- Tabs keep their scroll position and segment when you switch away and back. This is the opposite of the desktop, where a page opens at the top, because on a phone people hop between tabs mid-task.

## Screen anatomy

1. **Top bar.** The tab or detail title. iOS: a large title that collapses into a centred inline title on scroll. Android: a Material top app bar, title left-aligned. Actions on the right as `IconSm` buttons (at most two).
2. **Summary.** Stat tiles in two columns (`grid grid-cols-2 gap-3`), at most four. Same `Stat` component.
3. **Content.** One column. Cards become full-width sections with `mobile-gutter` side padding. Item cards (debts, accounts) become rows: name and caption on the left, the figure on the right, the whole row is the tap target.
4. **Primary action.** iOS: a `Plus` button in the top bar. Android: a floating action button above the navigation bar, `mobile-fab` square, `radius-xl`, `primary` fill. One per screen.

## Forms become sheets

There are no side-column form cards on a phone. "Add a debt", "Log a payment", "Edit account" open a sheet from the bottom:

- iOS: a page sheet with a grabber, Cancel on the left of its top bar and the action ("Add", "Log") on the right. Swipe down dismisses; if anything was typed, ask "Discard this payment?" first.
- Android: a full-screen dialog for long forms (account, debt) and a modal bottom sheet for short ones (log a payment). The system back gesture closes it, with the same discard question.
- Fields stack one per row except pairs of short numbers (APR and minimum). Field order and labels are identical to desktop.
- Amount fields open the decimal keyboard (`inputmode="decimal"`), date fields the native date picker. Keep `Input` uncontrolled; commit on blur exactly as on desktop.

## Touch

- Every tap target is at least `touch-target-ios` (44pt) on iOS and `touch-target-android` (48dp) on Android, even when the drawn control is smaller (pad the hit area).
- Rows swipe left to reveal Delete (iOS trailing swipe; Android long-press to select, then a Delete action). Delete still confirms, now with a sheet: "Delete Visa card? Its payments are deleted too." with Delete and Keep.
- Logging a payment, undoing one, and finishing a goal give a light haptic tick. Nothing else vibrates.
- Charts: no hover. Touch and drag across the plot shows the readout line and values in the legend, exactly as hover does on desktop. Lift to clear.
- Tables wider than the screen (payment schedule, year by year) become a list of month or year rows with the two or three key columns, and a "Show all columns" detail screen that scrolls sideways.

## Platform conventions

| | iPhone | Android |
|---|---|---|
| Font | `system-ui` resolves to SF Pro | `system-ui` resolves to Roboto |
| Text size | Follows Dynamic Type; map the OS size to the root font size, replacing the Auto/S/M/L/XL control | Follows the system font scale the same way |
| Dark mode | Follows the system by default; Settings offers System / Light / Dark | Same |
| Back | Swipe from the left edge, back button in the top bar | System back gesture or button |
| Primary add | `Plus` in the top bar | Floating action button |
| Delete a row | Trailing swipe | Long-press, then Delete |
| Backups | Share sheet to save to Files; document picker to restore | Storage Access Framework save and open dialogs |
| App lock | Face ID / Touch ID | Fingerprint / face unlock (BiometricPrompt) |
| Hide in app switcher | Blur the snapshot when the app goes inactive | `FLAG_SECURE` on the window |

## Privacy on a phone

A phone is easier to lose than a laptop, so the phone apps add two settings under More, both off by default and offered on first launch:

- **Lock with Face ID / fingerprint.** Asks on open and after 5 minutes in the background. The data file itself is protected by the OS (iOS Data Protection, Android file-based encryption).
- **Hide balances in the app switcher.**

## Performance

- The engine is the same, so projections and plans still run in `off_thread` with skeletons. Budget: the first frame of any tab in under 300 ms on a mid-range phone from 2022.
- No blurred shadows (the desktop rule matters more on a phone GPU). The tab bar and top bar are opaque `background`, not frosted glass.
- Charts keep to about 60 points on a phone, not 120.

## Releases

- **Android:** an APK attached to each GitHub release, built in CI next to the flatpak and the Windows zip. Later, F-Droid, which suits an offline open-source app.
- **iPhone:** needs an Apple Developer account and a Mac runner in CI. TestFlight first, then the App Store. Without the account, the iPhone app can only be installed from Xcode onto the developer's own phone.
- Version numbers stay in step with the desktop: one `Cargo.toml` version, one release.

## Phases

1. **Engine check.** Build the library crate for `aarch64-apple-ios` and `aarch64-linux-android`; run the existing tests on an emulator. Bundled SQLite and `ureq` with rustls need no changes.
2. **Shell.** A Dioxus mobile target with the five tabs, the top bar and the sheet container, using the existing pages unchanged inside it. This shows every feature on a phone, cramped but working.
3. **Phone layouts.** Page by page, swap the desktop grids for the phone anatomy above: Home and Cash flow first (most-used), then Plan, Accounts, More.
4. **Touch details.** Swipe and long-press actions, drag-to-inspect charts, haptics, keyboard types.
5. **Platform.** App lock, switcher privacy, share-sheet backups, Dynamic Type mapping, icons and splash screens.
6. **Ship.** Android APK in releases, then iOS through TestFlight.

## Open questions

- Is a phone-to-desktop sync wanted later (for example through a file in iCloud Drive or a folder the person already syncs)? This plan assumes export and import only.
- Should the phone apps be able to add entries quickly (a "Log a payment" home-screen shortcut or widget)? Useful, but it adds platform code outside Dioxus.
- An iPad layout can reuse the desktop layout at its larger widths; the container queries already handle it. Worth a check once the iPhone app runs.
