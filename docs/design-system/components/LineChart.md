Lines over time with a hover readout. Source: `src/widgets.rs`.

| Prop | Type | Notes |
|---|---|---|
| `series` | `Vec<Series>` | `Series { name, color, values, bold }`; `color` is any CSS colour, use `"var(--chart-3)"` |
| `labels` | `Vec<String>` | One per point: "2030" or "Nov 26" |
| `currency` | `String` | The settings currency symbol |

It draws a legend, four or five gridlines at round values, a stronger zero line, and x labels thinned to about eight. Hovering a column shows every series' value in the legend. With fewer than two points it shows "Not enough data to chart yet."

- Mark exactly one series `bold: true`: the one the card is about. It uses `chart-3`.
- Keep the colour meanings: retirement `success`, investments `warning`, cash `chart-1`, debt `destructive`, a comparison line `muted-foreground`.
- Five series at most. Put the chart in a `Card` whose description says what the lines show.
- While the data is being calculated, render `ChartSkeleton {}`, which has the same footprint.

```rust
LineChart {
    series: vec![Series { name: "Net worth".into(), color: "var(--chart-3)".into(), values, bold: true }],
    labels, currency: sym.clone(),
}
```
