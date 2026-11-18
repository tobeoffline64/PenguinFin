A tile with one headline number. Source: `src/widgets.rs`.

| Prop | Type | Notes |
|---|---|---|
| `label` | `String` | Shown as the uppercase `overline`. Two or three words. |
| `value` | `String` | Already formatted, e.g. `money(total, &sym)` or "Nov 2029". |
| `sub` | `Option<String>` | One caption line of context under the value. |
| `tone` | `String` | `"good"` colours the value `success`, `"bad"` `destructive`, anything else neutral. |
| `loading` | `bool` | Shows skeletons in place of value and caption. |

Put two to four in `grid grid-cols-2 gap-4 @3xl:grid-cols-4` directly under the `PageHeader`. Use a tone only when the number is plainly good or bad (debt owed, money left over); most stats are neutral. Pass `loading` while a projection is still being calculated so the tile keeps its size.

```rust
Stat { label: "Debt-free", value: date_text, sub: "In 3 yr 1 mo with this plan", tone: "good", loading: plans.is_none() }
```
