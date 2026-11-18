Parts of a whole over time, as stacked areas with a total line. Source: `src/widgets.rs`.

| Prop | Type | Notes |
|---|---|---|
| `layers` | `Vec<Series>` | Stacked bottom-up; negative values are treated as 0 |
| `total` | `Series` | Drawn as a line on top; use `"var(--foreground)"` |
| `reference` | `Option<(String, f64)>` | A dashed `destructive` line, e.g. `("Minimum", 2000.0)` |
| `labels`, `currency` | | As `LineChart` |

Use it when the parts add up to something the reader cares about: each debt in the total owed, an account split into buckets. The first layer sits on the axis, so put the largest or longest-lived part first. Layer colours come from the categorical order (`destructive`, `warning`, `chart-3`, `chart-5`, `info`, `success` for debts; `chart-3`, `success`, `warning`, `info`, `chart-5`, `destructive`, `chart-2` for buckets), with `chart-muted` for an unallocated remainder. Six layers at most; beyond that, group the rest.

```rust
StackedChart { layers, total: Series { name: "Total owed".into(), color: "var(--foreground)".into(), values: totals, bold: true },
    reference: None, labels, currency: sym.clone() }
```
