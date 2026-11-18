# Patterns

How the existing screens are put together. Follow these before inventing a layout.

## Anatomy of a page

Every page component returns a fragment that the shell places in a scrolling column (`max-w-6xl`, `gap-6`, padding `p-4 @2xl:p-6 @3xl:p-8`). Top to bottom:

1. `PageHeader` with the page title and a one-sentence subtitle.
2. A row of up to four `Stat` tiles: `grid grid-cols-2 gap-4 @3xl:grid-cols-4`.
3. The main content. For a list with an editor: `grid gap-6 @5xl:grid-cols-3`, the list in `@5xl:col-span-2` and the form card in the third column. For a chart: a full-width `Card`.
4. Detail tables and history, each in its own `Card`.

```rust
rsx! {
    PageHeader { title: "Debt payoff", subtitle: "Credit cards and loans: what they cost, and a plan to pay them off." }
    div { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
        Stat { label: "Total debt", value: money(total, &sym), sub: "3 debts", tone: "bad" }
        Stat { label: "Debt-free", value: "Nov 2029", tone: "good", loading: plans.is_none() }
    }
    div { class: "grid gap-6 @5xl:grid-cols-3",
        div { class: "flex flex-col gap-6 @5xl:col-span-2", /* list cards */ }
        DebtFormCard { form, generation, sym: sym.clone() }
    }
}
```

## Reading and writing data

- Get the store with `let mut store = use_store();`. Read with `store.data.read()`; clone what a closure needs.
- Write only through `store.mutate("Toast text", |db| db.some_write(...))`. It runs the write, reloads all data, and shows the toast, or an error toast if the write fails. Pass `""` for a silent save.
- Database writes take `&mut Db` and return `Result<(), String>`. Add `.map(|_| ())` when the method returns an id.

## Forms

- A form lives in a `Card` in the side column, titled "Add a …" or "Edit …". Fields are `Field { label, hint }` wrapping an `Input`, `NativeSelect` or `Switch`, stacked with `gap-3`; pairs of short fields go in `grid grid-cols-2 gap-3`.
- `Input` is uncontrolled: it reads `value` once when it mounts and reports the new value on commit (blur or Enter), not per keystroke. This is deliberate: controlled inputs dropped keystrokes in the desktop webview.
- To put new contents into the inputs (starting an edit, clearing after save), remount them with a generation key. Keys only work inside a list, hence the one-item loop:

```rust
let mut generation = use_signal(|| 0u32);
// when the form changes from outside: form.set(...); generation += 1;
for g in [generation()] {
    div { key: "{g}", class: "flex flex-col gap-3",
        Field { label: "Name", Input { value: form().name, oninput: move |e: FormEvent| form.write().name = e.value() } }
    }
}
```

- Parse amounts with `parse_num` (it accepts "$1,200.50") and print them back with `num_str`. Put the unit in the label: "Balance owed ($)", "Interest rate (APR %)".
- Validate on submit. Show the problem as `p { class: "text-xs text-destructive", "{e}" }` above the buttons, saying what to do: "Enter the amount you paid."
- Fields change with the choice made above them (an account form shows different fields per account type). Do not show fields that do not apply.
- Buttons at the bottom, primary first: `Button { "Add debt" }` then `Button { variant: ButtonVariant::Ghost, "Cancel" }`.
- A small form that belongs to one item opens inside that item's card, in a `rounded-md border bg-muted/40 p-3` box, with Sm buttons (see "Log a payment").

## Lists of items

- Each item is a `rounded-lg border p-4` block inside a `Card`, stacked with `gap-3`. Left: name (`font-semibold`), badges, a `caption` line of facts. Right: the `figure`, then the edit and delete icon buttons.
- The selected or edited item gets `border-ring`.
- Secondary facts go in two-up `rounded-md bg-muted/40 px-3 py-2 text-xs` boxes.
- Sub-rows (payments, buckets) are `text-xs` rows inside a `rounded-md border px-3 py-2` box: a small badge, the text with `flex-1`, a Ghost Sm action on the right.

## Calculated values

Projections and plans are computed off the UI thread and shown with skeletons while they run:

```rust
let plans = use_resource(move || {
    let data = store.data.read().clone();
    async move { off_thread(move || plan_with(&data.debts, /* … */)).await }
});
let plans_value = plans.read().clone();
// A result computed for older data must not be indexed with newer data: treat it as loading.
let plans_value = plans_value.filter(|p| p.chosen.payoff.len() == data.debts.len());
```

Render `Stat { loading: plans_value.is_none() }`, `ChartSkeleton {}` and `TableSkeleton { rows: 6 }` until the value arrives.

## Charts

- Charts sit in a full-width `Card` whose description states the conclusion in words ("Debt-free in Nov 2029, with $2,028 of interest along the way.").
- `LineChart` for values over time that can cross (net worth, cash, debt). `StackedChart` for parts of a whole over time (balances by debt, an account split into buckets), with the total as a line and an optional dashed reference line.
- Keep to about 120 points: monthly for short spans, every few months or yearly for long ones. Label points "Nov 26" (months) or "2030" (years).
- Colours follow the order in the Colour rules. Series names are plain nouns: "Net worth", "Cash", "Total owed".

## Tables

- Wrap in `TableWrapper` (it scrolls and caps the height; override with `class: "max-h-[24rem]"`). The header is sticky.
- First column is the label, `font-medium whitespace-nowrap`. Amounts are `text-right tabular-nums`; less important amounts add `text-muted-foreground`. An empty cell is an em dash.
- Long tables show the first rows and a Ghost "Show all 32 months" button.

## Settings and switches

A setting is a row: title (`text-sm`) and a one-line `caption` on the left, the `Switch` or control on the right (`flex items-start justify-between gap-3`). It saves immediately; there is no Save button.
