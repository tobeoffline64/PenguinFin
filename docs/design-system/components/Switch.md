An on/off setting that takes effect at once. Source: `src/ui/switch.rs`.

| Prop | Type | Notes |
|---|---|---|
| `checked` | `bool` | Controlled |
| `onchange` | `EventHandler<bool>` | Receives the new state |
| `aria_label` | `String` | Required: the switch has no visible label of its own |

You provide the row around it: a title and a one-line caption on the left, the switch on the right (`flex items-start justify-between gap-3`). Phrase the title so that "on" is the affirmative: "Also subtract it from Checking's balance". Use a switch for a setting; use buttons for an action.

```rust
div { class: "flex items-start justify-between gap-3",
    div { class: "flex flex-col",
        span { class: "text-sm", "Set aside payments in a bucket" }
        span { class: "text-xs text-muted-foreground", "Keeps this debt's planned payment out of your spendable cash." }
    }
    Switch { checked: set_aside(), aria_label: "Set aside payments in a bucket", onchange: move |v| set_aside.set(v) }
}
```
