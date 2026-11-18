The title block at the top of every page. Source: `src/views/dashboard.rs`.

Props: `title`, optional `subtitle`, and children placed at the right (a primary action or a control that applies to the whole page).

One per page, first. The title is the sidebar label or a short form of it. The subtitle is one sentence saying what the page is for, in the reader's terms.

```rust
PageHeader { title: "Savings goals", subtitle: "Things you want to buy, and when you can afford them without dipping into your baselines." }
```
