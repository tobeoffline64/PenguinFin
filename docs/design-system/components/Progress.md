A bar showing how far along something is. Source: `src/ui/progress.rs`.

Props: `value: f64` (default 0), `max: f64` (default 100), `class`. The fill is `primary` on a `secondary` track, 0.5rem tall, and is clamped to 0 to 100%.

Use it for progress toward a known target: a savings goal, a bucket's target. Always show the numbers beside it ("$3,000 of $5,000"); the bar alone is not enough. Do not use it for loading, which uses skeletons.

```rust
Progress { value: plan.progress * 100.0 }
```
