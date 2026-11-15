//! PenguinFin desktop app entry point.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;
mod views;
mod widgets;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use penguinfin::db::{Db, default_db_path};
use penguinfin::model::Settings;

/// Tailwind output, generated from `style/app.css` (see README).
const APP_CSS: &str = include_str!("../assets/app.css");

/// Shown while the webview starts and connects to the app (the window would otherwise
/// be blank for a moment). Removed by the app once it has rendered.
const BOOT_SKELETON: &str = r#"<div id="boot" aria-hidden="true" class="@container/app fixed inset-0 flex bg-background">
  <div class="flex w-14 @5xl/app:w-60 shrink-0 flex-col gap-3 border-r bg-sidenav p-3">
    <div class="pf-sk h-10 rounded-md"></div>
    <div class="pf-sk h-8 rounded-md"></div><div class="pf-sk h-8 rounded-md"></div>
    <div class="pf-sk h-8 rounded-md"></div><div class="pf-sk h-8 rounded-md"></div>
    <div class="pf-sk h-8 rounded-md"></div><div class="pf-sk h-8 rounded-md"></div>
  </div>
  <div class="@container flex flex-1 flex-col gap-6 p-4 @3xl:p-8">
    <div class="pf-sk h-8 w-56 rounded-md"></div>
    <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
      <div class="pf-sk h-24 rounded-xl"></div><div class="pf-sk h-24 rounded-xl"></div>
      <div class="pf-sk h-24 rounded-xl"></div><div class="pf-sk h-24 rounded-xl"></div>
    </div>
    <div class="pf-sk flex-1 rounded-xl"></div>
  </div>
</div>"#;

fn index_html(settings: &Settings) -> String {
    format!(
        r#"<!DOCTYPE html>
<html class="{dark}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="color-scheme" content="light">
<title>PenguinFin</title>
<style>{APP_CSS}</style>
<style id="pf-scale">html {{ font-size: {size}; }}</style>
</head>
<body>
{BOOT_SKELETON}
<div id="main"></div>
</body>
</html>"#,
        dark = if settings.dark_mode { "dark" } else { "" },
        size = settings.ui_scale.root_font_size(),
    )
}

/// Initial window size; `PENGUINFIN_WINDOW_SIZE=WxH` overrides it (handy for testing layouts).
fn initial_size() -> LogicalSize<f64> {
    std::env::var("PENGUINFIN_WINDOW_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some(LogicalSize::new(w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
        .unwrap_or(LogicalSize::new(1280.0, 860.0))
}

/// `PENGUINFIN_COMPAT_RENDERING=1` restores the slower but most compatible rendering path
/// (XWayland, no DMA-BUF) in case a graphics driver shows a blank or flickering window.
fn compat_rendering() -> bool {
    std::env::var("PENGUINFIN_COMPAT_RENDERING").is_ok_and(|v| !v.is_empty() && v != "0")
}

fn main() {
    // Open the database before the window appears, so the first frame already uses the
    // saved theme and UI size. The App takes ownership of it in `app::App`.
    let path = std::env::var_os("PENGUINFIN_DB").map(std::path::PathBuf::from).unwrap_or_else(default_db_path);
    let boot = Db::open(&path)
        .and_then(|db| db.load().map(|d| (db, d)))
        .map_err(|e| format!("{}: {e}", path.display()));
    let settings = boot.as_ref().map(|(_, d)| d.settings.clone()).unwrap_or_default();
    app::set_boot(boot);

    // The app sets its own GTK theme to match its light/dark setting (see app::match_gtk_theme),
    // so native popups like dropdown lists don't follow a dark desktop theme. A GTK_THEME
    // environment variable would override that, so clear it before GTK starts.
    // SAFETY: still single-threaded here; nothing else reads the environment concurrently.
    unsafe { std::env::remove_var("GTK_THEME") };

    let (bg_r, bg_g, bg_b) = if settings.dark_mode { (19, 19, 19) } else { (255, 255, 255) };
    let window = WindowBuilder::new()
        .with_title("PenguinFin")
        .with_inner_size(initial_size())
        .with_min_inner_size(LogicalSize::new(720.0, 540.0));
    let cfg = Config::new()
        .with_window(window)
        .with_menu(None)
        .with_custom_index(index_html(&settings))
        .with_background_color((bg_r, bg_g, bg_b, 255))
        .with_data_directory(penguinfin::db::data_dir().join("webview"))
        .with_disable_context_menu(!cfg!(debug_assertions))
        // Dioxus defaults to forcing XWayland and turning off WebKit's DMA-BUF renderer on
        // Wayland (a workaround for some NVIDIA drivers). That makes every frame slower and
        // adds input latency for everyone else, so run natively unless asked not to.
        .with_disable_dma_buf_on_wayland(compat_rendering());
    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(app::App);
}
