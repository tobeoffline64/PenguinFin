How a desktop page becomes a phone screen, on iPhone and Android. Planned: nothing here is built yet; see [mobile.md](../mobile.md).

- The shell is a bottom tab bar with five tabs (Home, Cash flow, Accounts, Plan, More), `mobile-tab-bar` tall plus the safe-area inset. Icons are the same `Icon` set at `size-5`; labels are always shown.
- iPhone: a large title that collapses on scroll, the add action as a `Plus` in the top bar, the home indicator clear of content.
- Android: a top app bar with a left-aligned title, the active tab marked by a `secondary` pill, a floating action button (`mobile-fab`, `radius-xl`, `primary`) for the one add action.
- Content is one column with `mobile-gutter` sides. `Stat` tiles go two across. Item cards become rows at least `mobile-row` tall; the whole row opens the item.
- Pages that were several sidebar entries share a tab and switch with a segmented control (the `Tabs` Default variant): Income / Bills / Expenses, and Debt / Goals / Projections.
- Charts read by touch and drag instead of hover.

The previews are static mock-ups drawn with the app's own tokens and classes. The phone frames are illustration only.
