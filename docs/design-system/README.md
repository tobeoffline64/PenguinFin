# PenguinFin design system

PenguinFin is an offline personal finance planner for the desktop (Linux flatpak and Windows). It is written in Rust with Dioxus; the UI is Rust/UI components copied into `src/ui`, a few app widgets in `src/widgets.rs`, and Tailwind v4 classes compiled to `assets/app.css`. Use this system when adding or changing a screen in that app. The plan for iPhone and Android apps built from the same code is in [mobile.md](mobile.md).

## Principles

- **Money first.** Every screen answers a money question. Put the answer as a figure at the top (`Stat`), the explanation under it, the editing form beside it.
- **Quiet neutrals, colour only for meaning.** The interface is black, white and greys. Hue appears only where it says something: `destructive` for money owed or lost, `success` for money gained or a goal met, `warning` and `info` for states, and the chart palette for series.
- **Fast like a local app.** Nothing blurs, nothing animates on load, typing never re-renders the page. A change that makes a frame slower is a regression.
- **Private and offline.** Never add a network call to a screen. The only network feature is the optional FRED sync in Settings.
- **One size system.** Everything is sized in `rem` and follows the Text size setting (Auto, S, M, L, XL). Never use `px` for layout, and never use viewport media queries.

## Content

- Write to the person as "you": "You're debt-free in November 2029", "Did you make it?".
- Sentence case everywhere: titles, buttons, labels, badges. The only uppercase text is the `overline` label above a stat value.
- Buttons are a verb and an object: "Add debt", "Log a payment", "Schedule payment". The safe side of a confirmation is the plain word: "Keep", "Cancel".
- Toasts are a past-tense fact with no full stop: "Payment logged", "Debt removed". Errors start "Could not save: " and give the reason.
- Explain consequences in the card description or a hint, in one sentence: "Rent, utilities and your other bills come first. The plan only uses what's left."
- Placeholders are examples, not instructions: "e.g. Visa card", "e.g. 24.99", "Optional".
- Money: `money()` for whole amounts in figures and summaries ("$17,850"), `money_cents()` wherever a payment or statement amount is shown ("$147.05"), `money_short()` only on chart axes ("$350k"). The currency symbol comes from settings; never hard-code "$".
- Dates read "Nov 16, 2026" in rows and "Nov 2029" for months. Separate facts on one line with a middle dot: "29.99% APR · minimum $35.00/mo · from Checking".
- No emoji, no exclamation marks.

## Colour

- Ground is `background`; panels are `card` with a `border`. Text is `foreground`; anything secondary is `muted-foreground`.
- One solid `primary` button per card or form. Everything else is Outline or Ghost.
- `destructive` means money owed, a negative amount, an error, or deleting. `success` means money gained, a paid-off debt, a met goal.
- `success` and `warning` are too light for small text on a light `card` (3.0:1 and 3.4:1). Use them at `figure` size or larger, or put the word in a Success/Warning badge, whose `success-dark` on `success-light` pair is 9:1.
- A status is never colour alone. Pair it with a word ("Paid", "Was planned") or a sign ("-$950", "+ $6,142"). `success` and `destructive` have nearly the same lightness, so they cannot be told apart by people with red-green colour blindness.
- Chart series colours, in this order: `chart-3` (the lead series, drawn bold), `success`, `warning`, `chart-1`, `destructive`, then `info`, `chart-5`, `chart-2`. Net worth is always `chart-3`, retirement `success`, investments `warning`, cash `chart-1`, debt `destructive`, a total line `foreground`.
- Pass chart colours as CSS variables (`"var(--chart-3)"`), never as literal values, so both themes work.
- Dark is a setting inside the app (`.dark` on the root element), not the desktop's theme. Use `dark:` variants only where a component already does.

## Type

- One family, `sans` (Inter, falling back to Cantarell, Noto Sans, then the system font). The app does not bundle a font file.
- Weights: 400 for text, 500 for labels and emphasis, 600 for titles and figures. 700 appears once, in the sidebar wordmark.
- A page has one `page-title`. Cards use `card-title`. Inside a card, use `row-title`, `figure`, `body` and `caption`; never a second card title.
- Every number that can change or sit in a column gets `tabular-nums`. Right-align amounts in tables.
- Nothing is smaller than `micro` (0.6875rem) except the text of a small badge.

## Space and shape

- The page is one column of sections with `space-6` between them, inside `max-w-6xl`.
- Stat tiles sit in a grid with `space-4` gaps. A list beside its form is a three-column grid with `space-6` gaps (list spans two).
- Inside a card: `space-4` between blocks, `space-3` between form fields, `space-2` between inline things, `space-1.5` between a label and its control.
- Radius steps down as things nest: `radius-xl` for a card, `radius-lg` for an item or inset box inside it, `radius-md` for controls and badges. Circles (`rounded-full`) are only for position numbers, legend dots, the progress bar and the switch.
- Layout responds to the size of its container, never the window: `@3xl:` and `@5xl:` on the page, `@5xl/app:` on the shell. This is what makes the Text size setting reflow the layout.

## Borders, shadows, motion

- Separation comes from a 1px `border`, not from shadow. `shadow-xs` and `shadow-sm` are flat 1px lines by design.
- Only the toast and the switch thumb use the blurred `shadow-lg`.
- Motion is limited to `transition-colors` on controls, the switch thumb, and the pulse on loading skeletons. Skeletons fade in after 150 ms (`pf-delayed`) so fast loads never flash.
- Focus is a 2 to 3px `ring` at 50%. It is faint; do not make it fainter.

## Icons

- Icons are Lucide (ISC licence) paths stored in `src/widgets.rs` and drawn by the `Icon` component: 24 by 24 grid, 2px stroke, round caps and joins, `currentColor`. Default size is `size-4`.
- To add one, add a variant to `IconName` and its path data from Lucide. Do not mix in another icon set, and do not use emoji or text glyphs as icons.
- An icon-only button always has a `title` and an `aria_label`.

## States

- Loading: show the page frame at once and put `LoadingBlock`, `ChartSkeleton` or `TableSkeleton` where a calculated value will appear. `Stat` has a `loading` prop.
- Empty: `EmptyState` with one sentence saying what will appear, and the primary action.
- Destructive actions confirm in place: the edit and delete icons are replaced by "Delete" (Destructive, Sm) and "Keep" (Ghost, Sm). There are no modal dialogs.
- Every saved change shows a toast through `store.mutate("Message", …)`. Anything that changes balances offers Undo.

## Not synced

- The components are Rust (Dioxus) and cannot run in a browser page, so each preview is a static rendition: the same class strings the component emits, styled by the app's own Tailwind build (`components/bundle.css`). Hover and focus work; the charts' hover readout and Tabs switching do not.
- Skipped variables: `--bottom__nav__height` (left over from the source theme, unused), `--chart-1-rgb` and `--chart-2-rgb` (bare number lists, unused).
- `radius-sm`, `radius-md` and `radius-xl` are `calc(var(--radius) ± px)` in code; the values here are what they compute to at a 16px root.
- No font file: Inter is named in the stack but not shipped with the app.

## Files

- [patterns.md](patterns.md): how existing screens are put together (page anatomy, forms, lists, charts, tables).
- [adding-a-feature.md](adding-a-feature.md): the order to build a feature in, and a checklist before calling a screen done.
- [mobile.md](mobile.md): the plan for iPhone and Android apps (not built yet).
- [tokens.json](tokens.json): every colour (light and dark), text style, spacing step, radius, shadow and phone size, with where it is used. [tokens.css](tokens.css) is the same as CSS variables.
- [feature-tree.html](feature-tree.html): every user-facing feature as a tree, with its Rust component and planned phone tab. Download it and open it in a browser.

## Components

Each component has guidelines (`components/`) and a static preview (`previews/`, open in a browser; add `#dark` to the address for the dark theme). The previews are styled by `tokens.css` and `previews/bundle.css`, which is the app's Tailwind build of `style/app.css` with the preview files added as sources.

**Actions**

- [Button](components/Button.md) · [preview](previews/Button.html)

**Forms**

- [Field](components/Field.md) · [preview](previews/Field.html)
- [Input](components/Input.md) · [preview](previews/Input.html)
- [NativeSelect](components/NativeSelect.md) · [preview](previews/NativeSelect.html)
- [Switch](components/Switch.md) · [preview](previews/Switch.html)

**Data display**

- [Badge](components/Badge.md) · [preview](previews/Badge.html)
- [Card](components/Card.md) · [preview](previews/Card.html)
- [Icon](components/Icon.md) · [preview](previews/Icon.html)
- [Progress](components/Progress.md) · [preview](previews/Progress.html)
- [Separator](components/Separator.md) · [preview](previews/Separator.html)
- [Stat](components/Stat.md) · [preview](previews/Stat.html)
- [Table](components/Table.md) · [preview](previews/Table.html)

**Charts**

- [LineChart](components/LineChart.md) · [preview](previews/LineChart.html)
- [StackedChart](components/StackedChart.md) · [preview](previews/StackedChart.html)

**Feedback**

- [EmptyState](components/EmptyState.md) · [preview](previews/EmptyState.html)
- [LoadingBlock](components/LoadingBlock.md) · [preview](previews/LoadingBlock.html)
- [Toast](components/Toast.md) · [preview](previews/Toast.html)

**Navigation**

- [PageHeader](components/PageHeader.md) · [preview](previews/PageHeader.html)
- [SideNav](components/SideNav.md) · [preview](previews/SideNav.html)
- [Tabs](components/Tabs.md) · [preview](previews/Tabs.html)

**Mobile**

- [MobileScreens](components/MobileScreens.md) · [preview](previews/MobileScreens.html)
- [MobileSheet](components/MobileSheet.md) · [preview](previews/MobileSheet.html)

