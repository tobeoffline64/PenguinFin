A short confirmation at the bottom right after a change is saved. Source: `src/app.rs` (the `flash` signal on `Store`).

You do not render it. Call `store.mutate("Payment logged", |db| …)` and the message appears if the write succeeds; if it fails, a red-bordered toast says "Could not save: " and the reason. `store.notify(text, error)` shows one without a write. One toast at a time; a click dismisses it.

Write a past-tense fact, no full stop: "Payment logged", "Debt removed", "Payment scheduled; your plan now includes it". Pass `""` for saves that need no confirmation (typing in a settings field). Do not use a toast for something the person must read to continue; put that inline.
