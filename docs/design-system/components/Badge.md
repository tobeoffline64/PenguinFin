A small label for the kind or state of an item. Source: `src/ui/badge.rs`.

| Variant | Use for |
|---|---|
| `Secondary` | The type of a thing: "Credit card", "Student loan" |
| `Info` | A neutral state or role: "Planned", "Gets the extra payment", "expense" |
| `Success` | Done or positive: "Paid", "income", "On track for Mar 2027" |
| `Warning` | Needs a look: "Was planned", "bill" |
| `Muted` | A minor attribute: "autopay" |
| `Destructive`, `Default`, `Accent`, `Outline` | Not used by any screen |

`size`: `Sm` (0.625rem text) beside a name or in a sub-row, which is nearly every use; `Default` when it stands alone; `Lg` unused.

You provide the text: one to four words, sentence case. A badge is not clickable. Do not use more than two beside one name, and never rely on its colour without the word.

```rust
Badge { variant: BadgeVariant::Info, size: BadgeSize::Sm, "Planned" }
```
