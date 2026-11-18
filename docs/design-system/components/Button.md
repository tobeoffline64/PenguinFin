The control for every action. Source: `src/ui/button.rs`.

Use **Default** (solid `primary`) once per card or form, for the thing the card is for. Use **Outline** for a secondary action that should still be seen ("Log a payment", "Use $625"), **Ghost** for cancel, undo, remove and icon buttons, and **Destructive** only for the confirm step of a delete. Secondary, Warning, Success and Link exist in the source but no screen uses them; Warning and Success fail text contrast in light (about 3:1), so avoid them.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `variant` | `ButtonVariant` | `Default` | `Default`, `Destructive`, `Outline`, `Ghost`, `Secondary`, `Warning`, `Success`, `Link` |
| `size` | `ButtonSize` | `Default` | `Default` (h-9), `Sm` (h-8), `Lg` (h-10), `Icon`, `IconSm`, `IconXs` (square) |
| `disabled` | `bool` | `false` | 50% opacity, no pointer events |
| `onclick` | `EventHandler<MouseEvent>` | none | |
| `title`, `aria_label` | `String` | none | Required on icon-only buttons |
| `class` | `String` | none | Merged with `tw_merge!`, so it can override |

You provide the label as children: a verb and an object in sentence case, optionally after an `Icon`.

- Use `Sm` inside item cards and inline forms; `Default` at the bottom of a side-column form.
- Icon-only actions on a row are `Ghost` + `IconSm`.
- Do not put two solid buttons side by side. Do not use a button for navigation between pages.

```rust
Button { onclick: submit, Icon { name: IconName::Plus } "Add debt" }
Button { variant: ButtonVariant::Ghost, onclick: move |_| on_done.call(()), "Cancel" }
Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Delete", aria_label: "Delete",
    onclick: move |_| confirm.set(true), Icon { name: IconName::Trash } }
```
