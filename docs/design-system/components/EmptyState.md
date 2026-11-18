What a list shows before it has anything in it. Source: `src/widgets.rs`.

Props: `title`, `text`, and children for the action. A dashed, centred box.

You provide a title that names what is missing ("No debts yet"), one sentence saying what will appear once something is added, and usually the primary button that adds it. Use it in place of the list, not in addition to an empty table.

```rust
EmptyState { title: "No debts yet", text: "Add a credit card or loan to see what it costs each month and get a plan to pay it off.",
    Button { onclick: start_adding, Icon { name: IconName::Plus } "Add a debt" } }
```
