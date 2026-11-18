Rows of figures: schedules, year-by-year projections, history. Source: `src/ui/table.rs`.

Parts: `TableWrapper` (scrolls, bordered, `max-h-96`), `Table`, `TableHeader` (sticky), `TableBody`, `TableRow`, `TableHead`, `TableCell`, plus `TableFooter` and `TableCaption`.

- First column is the row label: `class: "font-medium whitespace-nowrap"`.
- Amounts: `class: "text-right tabular-nums"` on both the head and the cell. Supporting amounts add `text-muted-foreground`.
- An empty value is an em dash, not "0" or blank.
- Mark an event in a cell with colour and a sign together (a paid-off month is `text-success` with a tick).
- Row actions are Ghost Sm buttons in a last, right-aligned column with an empty head.
- Give every row a `key`.

```rust
TableWrapper { class: "max-h-[24rem]",
    Table {
        TableHeader { TableRow { TableHead { "Date" } TableHead { class: "text-right", "Amount" } } }
        TableBody {
            for p in payments.iter().cloned() {
                TableRow { key: "{p.id}",
                    TableCell { class: "whitespace-nowrap", "{p.date.format(\"%b %-d, %Y\")}" }
                    TableCell { class: "text-right tabular-nums", {money_cents(p.amount, &sym)} }
                }
            }
        }
    }
}
```
