//! PenguinFin 2's interface: Leptos in the webview, talking to the Tauri backend.

mod api;
mod app;
mod pages;
mod ui;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
