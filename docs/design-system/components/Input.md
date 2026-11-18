A single-line text, number or date field. Source: `src/ui/input.rs`.

**It is uncontrolled.** `value` is read once when the input mounts, and `oninput` fires when the value is committed (blur or Enter), not on each keystroke. Controlled inputs dropped keystrokes in the desktop webview. To change what an input shows from outside, remount it with a key (see Patterns, Forms).

| Prop | Type | Notes |
|---|---|---|
| `value` | `Option<String>` | Initial value only |
| `oninput` | `EventHandler<FormEvent>` | Called on commit; read `e.value()` |
| `r#type` | `InputType` | `Text` (default), `Date`, `Number`, `Password`, and the other HTML types |
| `placeholder` | `String` | An example: "e.g. 24.99" |
| `disabled`, `readonly`, `required` | `bool` | Read-only gets a `muted` fill |

Always wrap it in a `Field`. Use `Text` for amounts, not `Number`, and parse with `parse_num`, so "$1,200.50" is accepted. Use `InputType::Date` for dates; its value is `YYYY-MM-DD`.

```rust
Field { label: "Amount ({sym})",
    Input { value: amount(), placeholder: "e.g. 500", oninput: move |e: FormEvent| amount.set(e.value()) } }
```
