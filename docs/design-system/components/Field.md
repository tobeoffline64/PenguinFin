A label, a control, and an optional hint. Source: `src/widgets.rs`.

| Prop | Type | Notes |
|---|---|---|
| `label` | `String` | Shown above the control in `field-label` style |
| `hint` | `Option<String>` | One caption line under the control |
| children | | One `Input`, `NativeSelect` or other control |

Every form control goes inside a `Field`. Put the unit in the label ("Balance owed ($)", "Interest rate (APR %)"); use the hint for limits and consequences ("Up to $625 keeps every bill covered."). Stack fields with `gap-3`; put two short related fields side by side with `grid grid-cols-2 gap-3`.

```rust
Field { label: "Extra toward debt each month ({sym})", hint: "On top of the minimums.",
    Input { value: num_str(extra), oninput: move |e: FormEvent| save_extra(e.value()) } }
```
