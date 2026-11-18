Placeholders for values that are still being calculated. Source: `src/widgets.rs`.

- `LoadingBlock { class }`: one pulsing block; give it the size of the text it replaces (`"h-8 w-32"`).
- `ChartSkeleton {}`: legend row and a 3:1 plot, the footprint of `LineChart` and `StackedChart`.
- `TableSkeleton { rows }`: that many row-height bars.

All three fade in only after 150 ms (`pf-delayed`), so a fast calculation never flashes a skeleton, and all set `aria-busy`. Use them for anything computed with `use_resource` and `off_thread`. Do not use a spinner, and do not hold back the page frame while waiting: titles, labels and forms render at once.

```rust
match plans_value {
    Some(p) => rsx! { PayoffChart { plan: p.chosen.clone(), sym: sym.clone() } },
    None => rsx! { ChartSkeleton {} },
}
```
