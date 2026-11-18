The app's only navigation: nine pages in a fixed left rail. Source: `src/app.rs`.

It is 15rem wide with labels when the window is at least 64rem wide in the current text size (`@5xl/app:`), and collapses to a 3.5rem icon rail below that; collapsed items carry the label as a `title`. The current page has the `sidenav-accent` fill and medium weight. The Text size control and the "Data stays on this computer." note sit at the bottom.

A new page needs: a `Page` variant, a row in the `nav` list (page, label, icon), a match arm that renders the view, and one more bar in the boot skeleton in `src/main.rs`. Keep labels to three words. Order follows the money: what comes in, what goes out, what you owe, what you have, what you plan. Settings is always last. There are no nested menus; a page that grows too large gets tabs or a second page.
