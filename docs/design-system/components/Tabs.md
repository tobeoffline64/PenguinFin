Switches between views of the same data inside one card. Source: `src/ui/tabs.rs`.

Parts: `Tabs { default_value }`, `TabsList { variant }`, `TabsTrigger { value }`, `TabsContent { value }`. `TabsVariant::Default` is a segmented control on a `muted` track; `Line` underlines the active tab. `Tabs` keeps the active value itself.

Use tabs for two to four views of one thing (yearly or monthly, chart or table). Do not use them for page navigation, which is the sidebar, or for steps in a form. Labels are one or two words.

```rust
Tabs { default_value: "yearly",
    TabsList { TabsTrigger { value: "yearly", "Yearly" } TabsTrigger { value: "monthly", "Monthly" } }
    TabsContent { value: "yearly", /* … */ }
    TabsContent { value: "monthly", /* … */ }
}
```
