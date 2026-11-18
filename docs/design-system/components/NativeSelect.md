A dropdown for picking one of a known set. Source: `src/widgets.rs`.

It is a native `<select>` styled like `Input`, with its own chevron. `appearance-none` matters: without it WebKitGTK draws the control in the desktop's GTK theme, which made it black on dark desktops.

| Prop | Type | Notes |
|---|---|---|
| `value` | `String` | The key of the selected option |
| `options` | `Vec<(String, String)>` | `(key, label)` pairs, in display order |
| `onchange` | `EventHandler<String>` | Receives the chosen key |

You provide options with plain labels. Put the default first and say what it resolves to: "Default (Checking)", "Checking (usual)". Use an empty key for "none". For two or three choices that each need a sentence of explanation, use option tiles instead (see the strategy picker on the Debt payoff page).

```rust
Field { label: "Paid from",
    NativeSelect { value: account(), options: account_options.clone(), onchange: move |v: String| account.set(v) } }
```
