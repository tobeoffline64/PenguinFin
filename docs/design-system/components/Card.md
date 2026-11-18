The panel every block of a page sits in. Source: `src/ui/card.rs`.

Parts: `Card` (with `size: CardSize::Default | Sm`), `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `CardFooter`, `CardAction`. Default size has `py-6`, `px-6` and `gap-4`; `Sm` has `py-4`, `px-4`, `gap-3` and is used by `Stat`.

You provide a title that names the content, a description that explains or states the conclusion in one sentence, and the content. Items inside are `rounded-lg border p-4` blocks, never another `Card`.

- One idea per card. A page is a column of cards, not one big card.
- Do not add shadow, coloured borders or background tints to a card. The selected item inside a card gets `border-ring`.
- Every class prop is merged, so `Card { class: "gap-1" }` overrides the gap.

```rust
Card {
    CardHeader {
        CardTitle { "Payment history" }
        CardDescription { "One-time payments you've logged. Undo puts the balances back." }
    }
    CardContent { /* table, list or chart */ }
}
```
