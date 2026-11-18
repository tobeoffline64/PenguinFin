How forms open on a phone: every desktop form card becomes a sheet. Planned: nothing here is built yet; see [mobile.md](../mobile.md).

- iPhone: a page sheet with a grabber. Cancel on the left of its top bar, the action word ("Log", "Add", "Save") on the right in `semibold`. Swipe down dismisses; if anything was typed, confirm with "Discard this payment?".
- Android: a modal bottom sheet for short forms (log a payment, add a bucket) and a full-screen dialog for long ones (an account, a debt). Buttons at the bottom right, primary last. The back gesture closes it, with the same discard question.
- The page behind is dimmed with `foreground` at 40%. The tab bar is hidden while a sheet is open.
- Fields, labels, hints, order and validation messages are the same `Field`, `Input`, `NativeSelect` and `Switch` as desktop. Amounts open the decimal keyboard; dates the native picker.
- Every control in a sheet has a tap area of at least `touch-target-ios` / `touch-target-android`.
