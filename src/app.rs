use crate::views;
use crate::widgets::{Icon, IconName};
use dioxus::prelude::*;
use penguinfin::db::Db;
use penguinfin::model::{AppData, UiScale};
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Dashboard,
    Income,
    Bills,
    Expenses,
    Accounts,
    Projections,
    Settings,
}

#[derive(Clone, PartialEq)]
pub struct Flash {
    pub text: String,
    pub error: bool,
}

/// Shared app state: the open database and an in-memory copy of its contents.
#[derive(Clone, Copy)]
pub struct Store {
    db: Signal<Db>,
    pub data: Signal<AppData>,
    pub flash: Signal<Option<Flash>>,
}

impl Store {
    /// Run a database write, then reload everything and show a message.
    pub fn mutate(&mut self, ok_msg: &str, f: impl FnOnce(&mut Db) -> Result<(), String>) {
        let res = {
            let mut db = self.db.write();
            f(&mut db).and_then(|_| db.load())
        };
        match res {
            Ok(data) => {
                self.data.set(data);
                if !ok_msg.is_empty() {
                    self.flash.set(Some(Flash { text: ok_msg.to_string(), error: false }));
                }
            }
            Err(e) => self.flash.set(Some(Flash { text: format!("Could not save: {e}"), error: true })),
        }
    }

    pub fn db_path(&self) -> String {
        self.db.read().path().display().to_string()
    }

    pub fn notify(&mut self, text: impl Into<String>, error: bool) {
        self.flash.set(Some(Flash { text: text.into(), error }));
    }
}

pub fn use_store() -> Store {
    use_context::<Store>()
}

pub fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

type Boot = Result<(Db, AppData), String>;

/// The database opened in `main` before the window exists, handed to `App` once.
static BOOT: Mutex<Option<Boot>> = Mutex::new(None);

pub fn set_boot(boot: Boot) {
    *BOOT.lock().expect("boot lock") = Some(boot);
}

#[component]
pub fn App() -> Element {
    let opened: Result<Store, String> = use_hook(|| {
        let (db, data) = BOOT.lock().expect("boot lock").take().unwrap_or_else(|| Err("database was not opened".into()))?;
        let store = Store { db: Signal::new(db), data: Signal::new(data), flash: Signal::new(None) };
        provide_context(store);
        Ok(store)
    });

    match opened {
        Ok(_) => rsx! { Shell {} },
        Err(e) => rsx! {
            div { class: "p-10",
                h1 { class: "text-xl font-semibold", "PenguinFin could not open its database" }
                pre { class: "mt-4 text-sm text-destructive whitespace-pre-wrap", "{e}" }
            }
        },
    }
}

/// Browser-side helpers, installed once:
/// - Ctrl + / Ctrl - / Ctrl 0 send "bigger" / "smaller" / "auto" to Rust.
/// - When `window.__pfNatural` is true, wheel and touchpad scrolling is reversed
///   ("natural scrolling"), applied to the nearest element that can scroll that way.
const BRIDGE_JS: &str = r#"
if (!window.__pfBridge) {
  window.__pfBridge = true;
  window.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
    let cmd = null;
    if (e.key === '=' || e.key === '+') cmd = 'bigger';
    else if (e.key === '-' || e.key === '_') cmd = 'smaller';
    else if (e.key === '0') cmd = 'auto';
    if (cmd) { e.preventDefault(); dioxus.send(cmd); }
  });
  const canScroll = (el, dx, dy) => {
    const s = getComputedStyle(el);
    const y = /(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight + 1 &&
      ((dy < 0 && el.scrollTop > 0) || (dy > 0 && el.scrollTop + el.clientHeight < el.scrollHeight - 1));
    const x = /(auto|scroll)/.test(s.overflowX) && el.scrollWidth > el.clientWidth + 1 &&
      ((dx < 0 && el.scrollLeft > 0) || (dx > 0 && el.scrollLeft + el.clientWidth < el.scrollWidth - 1));
    return x || y;
  };
  window.addEventListener('wheel', (e) => {
    if (!window.__pfNatural || e.ctrlKey) return;
    const unit = e.deltaMode === 1 ? 16 : (e.deltaMode === 2 ? window.innerHeight : 1);
    const dx = -e.deltaX * unit, dy = -e.deltaY * unit;
    let el = e.target instanceof Element ? e.target : null;
    while (el && el !== document.documentElement && !canScroll(el, dx, dy)) el = el.parentElement;
    if (!el || el === document.documentElement) el = document.scrollingElement;
    e.preventDefault();
    el.scrollBy({ left: dx, top: dy, behavior: e.deltaMode === 0 && Math.abs(e.deltaY) < 40 ? 'auto' : 'smooth' });
  }, { passive: false, capture: true });
}
"#;

#[component]
fn Shell() -> Element {
    let mut store = use_store();
    let mut page = use_signal(|| Page::Dashboard);
    let settings = store.data.read().settings.clone();
    let dark = settings.dark_mode;
    let scale = settings.ui_scale;

    let mut flash = store.flash;
    use_effect(move || {
        if flash.read().is_some() {
            spawn(async move {
                sleep_ms(3500).await;
                flash.set(None);
            });
        }
    });

    // Keyboard shortcuts from the browser side.
    use_hook(move || {
        spawn(async move {
            let mut bridge = document::eval(BRIDGE_JS);
            while let Ok(cmd) = bridge.recv::<String>().await {
                let current = store.data.read().settings.ui_scale;
                let next = match cmd.as_str() {
                    "bigger" => current.bigger(),
                    "smaller" => current.smaller(),
                    _ => UiScale::Auto,
                };
                set_ui_scale(&mut store, next);
            }
        });
    });

    // Keep browser-side state in sync with settings.
    use_effect(move || {
        let s = store.data.read().settings.clone();
        document::eval(&format!(
            "window.__pfNatural = {}; document.documentElement.classList.toggle('dark', {});",
            s.natural_scroll, s.dark_mode
        ));
    });

    let nav = [
        (Page::Dashboard, "Dashboard", IconName::Dashboard),
        (Page::Income, "Income", IconName::Income),
        (Page::Bills, "Bills", IconName::Bill),
        (Page::Expenses, "Recurring expenses", IconName::Expense),
        (Page::Accounts, "Accounts & investments", IconName::Accounts),
        (Page::Projections, "Projections", IconName::Chart),
        (Page::Settings, "Settings & data", IconName::Settings),
    ];

    rsx! {
        // Root font size drives the whole UI (everything is sized in rem).
        style { "html {{ font-size: {scale.root_font_size()}; }}" }
        div { class: if dark { "dark h-full" } else { "h-full" },
            div { class: "@container/app flex h-screen bg-background text-foreground",
                // Collapses to an icon rail when the window is narrow or the UI is large.
                nav { class: "flex w-14 @5xl/app:w-60 shrink-0 flex-col gap-1 overflow-y-auto border-r bg-sidenav p-2 @5xl/app:p-3",
                    div { class: "flex items-center gap-2 px-1 pb-4 pt-2 @5xl/app:px-3",
                        img { class: "size-8 shrink-0 @5xl/app:hidden", src: LOGO_DATA_URI, alt: "PenguinFin" }
                        div { class: "hidden @5xl/app:block",
                            div { class: "text-lg font-bold tracking-tight", "PenguinFin" }
                            div { class: "text-xs text-muted-foreground", "Offline personal finance" }
                        }
                    }
                    for (p, label, icon) in nav {
                        button {
                            title: label,
                            class: if page() == p {
                                "flex items-center justify-center @5xl/app:justify-start gap-3 rounded-md px-2 py-2 @5xl/app:px-3 text-left text-sm font-medium bg-sidenav-accent text-sidenav-accent-foreground"
                            } else {
                                "flex items-center justify-center @5xl/app:justify-start gap-3 rounded-md px-2 py-2 @5xl/app:px-3 text-left text-sm text-muted-foreground hover:bg-sidenav-accent hover:text-sidenav-accent-foreground cursor-pointer"
                            },
                            onclick: move |_| page.set(p),
                            Icon { name: icon, class: "size-4 shrink-0" }
                            span { class: "hidden @5xl/app:inline truncate", "{label}" }
                        }
                    }
                    div { class: "mt-auto flex flex-col gap-2 pt-4",
                        UiSizeControl {}
                        div { class: "hidden px-3 text-[0.6875rem] text-muted-foreground @5xl/app:block", "Data stays on this computer." }
                    }
                }
                main { class: "@container min-w-0 flex-1 overflow-y-auto",
                    div { class: "mx-auto flex max-w-6xl flex-col gap-6 p-4 @2xl:p-6 @3xl:p-8",
                        match page() {
                            Page::Dashboard => rsx! { views::dashboard::Dashboard { on_nav: move |p| page.set(p) } },
                            Page::Income => rsx! { views::flows::FlowsPage {  kind: penguinfin::model::FlowKind::Income } },
                            Page::Bills => rsx! { views::flows::FlowsPage {  kind: penguinfin::model::FlowKind::Bill } },
                            Page::Expenses => rsx! { views::flows::FlowsPage {  kind: penguinfin::model::FlowKind::Expense } },
                            Page::Accounts => rsx! { views::accounts::AccountsPage {} },
                            Page::Projections => rsx! { views::projections::ProjectionsPage {} },
                            Page::Settings => rsx! { views::settings::SettingsPage {} },
                        }
                    }
                }
            }
            if let Some(f) = flash() {
                div {
                    class: if f.error {
                        "fixed bottom-6 right-6 z-50 max-w-md rounded-lg border border-destructive bg-card px-4 py-3 text-sm text-destructive shadow-lg"
                    } else {
                        "fixed bottom-6 right-6 z-50 max-w-md rounded-lg border bg-card px-4 py-3 text-sm text-card-foreground shadow-lg"
                    },
                    onclick: move |_| flash.set(None),
                    "{f.text}"
                }
            }
        }
    }
}

/// Save a new UI size (shared by the sidebar control, Settings and keyboard shortcuts).
pub fn set_ui_scale(store: &mut Store, scale: UiScale) {
    let mut s = store.data.read().settings.clone();
    if s.ui_scale == scale {
        return;
    }
    s.ui_scale = scale;
    store.mutate("", |db| db.save_settings(&s));
}

/// Compact text-size control for the sidebar footer: [-] size [+], with Auto on click.
#[component]
fn UiSizeControl() -> Element {
    let mut store = use_store();
    let scale = store.data.read().settings.ui_scale;
    let btn = "flex size-7 items-center justify-center rounded-md border bg-background text-sm font-medium hover:bg-accent cursor-pointer disabled:opacity-40 disabled:cursor-default";
    rsx! {
        div { class: "flex flex-col items-center gap-1 @5xl/app:flex-row @5xl/app:justify-between @5xl/app:px-3",
            span { class: "hidden text-xs text-muted-foreground @5xl/app:inline", "Text size" }
            div { class: "flex flex-col items-center gap-1 @5xl/app:flex-row",
                button { class: btn, title: "Bigger (Ctrl +)", aria_label: "Bigger text",
                    disabled: scale == UiScale::ExtraLarge,
                    onclick: move |_| set_ui_scale(&mut store, scale.bigger()), "+" }
                button {
                    class: if scale == UiScale::Auto {
                        "h-7 min-w-9 rounded-md bg-sidenav-accent px-1.5 text-xs font-medium cursor-pointer"
                    } else {
                        "h-7 min-w-9 rounded-md px-1.5 text-xs font-medium text-muted-foreground hover:bg-accent cursor-pointer"
                    },
                    title: "Fit to window (Ctrl 0)",
                    onclick: move |_| set_ui_scale(&mut store, UiScale::Auto),
                    "{scale.label()}"
                }
                button { class: btn, title: "Smaller (Ctrl −)", aria_label: "Smaller text",
                    disabled: scale == UiScale::Small,
                    onclick: move |_| set_ui_scale(&mut store, scale.smaller()), "−" }
            }
        }
    }
}

const LOGO_DATA_URI: &str = concat!(
    "data:image/svg+xml;utf8,",
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='8 8 112 112'>",
    "<rect x='8' y='8' width='112' height='112' rx='26' fill='%232f6fed'/>",
    "<ellipse cx='62' cy='80' rx='25' ry='30' fill='%2317202e'/>",
    "<ellipse cx='62' cy='86' rx='16' ry='21' fill='white'/>",
    "<circle cx='62' cy='54' r='17' fill='%2317202e'/>",
    "<ellipse cx='62' cy='58' rx='11' ry='9' fill='white'/>",
    "<circle cx='57' cy='53' r='2.6' fill='%2317202e'/><circle cx='67' cy='53' r='2.6' fill='%2317202e'/>",
    "<path d='M57.5 59 L66.5 59 L62 65 Z' fill='%23f5a623'/>",
    "</svg>"
);

/// Run `f` on a worker thread and await its result without blocking the UI.
pub async fn off_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = futures_channel::oneshot::channel::<T>();
    std::thread::spawn(move || {
        // Debug builds only: PENGUINFIN_SLOW=1 delays background work to inspect loading states.
        if cfg!(debug_assertions) && std::env::var_os("PENGUINFIN_SLOW").is_some() {
            std::thread::sleep(std::time::Duration::from_millis(2000));
        }
        let _ = tx.send(f());
    });
    rx.await.expect("worker thread finished")
}

/// Async sleep without pulling in a timer crate.
pub async fn sleep_ms(ms: u64) {
    let (tx, rx) = futures_channel::oneshot::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        let _ = tx.send(());
    });
    let _ = rx.await;
}
