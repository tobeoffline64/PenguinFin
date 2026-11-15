//! PenguinFin desktop app entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;
mod views;
mod widgets;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

/// Tailwind output, generated from `style/app.css` (see README).
const APP_CSS: &str = include_str!("../assets/app.css");

fn main() {
    let head = format!(
        r#"<meta name="color-scheme" content="light dark"><title>PenguinFin</title><style>{APP_CSS}</style>"#
    );
    let window = WindowBuilder::new()
        .with_title("PenguinFin")
        .with_inner_size(LogicalSize::new(1280.0, 860.0))
        .with_min_inner_size(LogicalSize::new(900.0, 600.0));
    let cfg = Config::new()
        .with_window(window)
        .with_menu(None)
        .with_custom_head(head)
        .with_data_directory(penguinfin::db::data_dir().join("webview"))
        .with_disable_context_menu(!cfg!(debug_assertions));
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(app::App);
}
