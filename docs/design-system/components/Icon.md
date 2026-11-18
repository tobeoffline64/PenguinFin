The 14 Lucide icons the app uses. Source: `src/widgets.rs` (`IconName`, `icon_paths`).

Props: `name: IconName`, `class` (default `"size-4"`). Drawn on a 24 by 24 grid with a 2px round stroke in `currentColor`, so it takes the text colour of its parent.

| Icons | Meaning |
|---|---|
| `Dashboard`, `Income`, `Bill`, `Expense`, `CreditCard`, `Accounts`, `Target`, `Chart`, `Settings` | The nine pages, in sidebar order |
| `Plus`, `Pencil`, `Trash` | Add, edit, delete |
| `Alert` | A warning beside text |
| `ChevronDown` | The select chevron |

Keep these meanings: `Trash` always deletes, `Pencil` always edits. To add an icon, add a variant and copy its path data from Lucide. Icons sit beside a label or inside a titled icon button; they are never decoration.

```rust
Icon { name: IconName::Alert, class: "size-4 text-destructive" }
```
