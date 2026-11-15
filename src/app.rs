use crate::views;
use crate::widgets::{Icon, IconName};
use dioxus::prelude::*;
use penguinfin::db::{Db, default_db_path};
use penguinfin::model::AppData;

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

#[component]
pub fn App() -> Element {
    let opened: Result<Store, String> = use_hook(|| {
        let path = std::env::var_os("PENGUINFIN_DB").map(std::path::PathBuf::from).unwrap_or_else(default_db_path);
        let (db, data) = Db::open(&path)
            .and_then(|db| db.load().map(|d| (db, d)))
            .map_err(|e| format!("{}: {e}", path.display()))?;
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

#[component]
fn Shell() -> Element {
    let store = use_store();
    let mut page = use_signal(|| Page::Dashboard);
    let dark = store.data.read().settings.dark_mode;

    let mut flash = store.flash;
    use_effect(move || {
        if flash.read().is_some() {
            spawn(async move {
                tokio_sleep(3500).await;
                flash.set(None);
            });
        }
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
        div { class: if dark { "dark h-full" } else { "h-full" },
            div { class: "flex h-screen bg-background text-foreground",
                nav { class: "flex w-60 shrink-0 flex-col gap-1 border-r bg-sidenav p-3",
                    div { class: "px-3 pb-4 pt-2",
                        div { class: "text-lg font-bold tracking-tight", "PenguinFin" }
                        div { class: "text-xs text-muted-foreground", "Offline personal finance" }
                    }
                    for (p, label, icon) in nav {
                        button {
                            class: if page() == p {
                                "flex items-center gap-3 rounded-md px-3 py-2 text-left text-sm font-medium bg-sidenav-accent text-sidenav-accent-foreground"
                            } else {
                                "flex items-center gap-3 rounded-md px-3 py-2 text-left text-sm text-muted-foreground hover:bg-sidenav-accent hover:text-sidenav-accent-foreground cursor-pointer"
                            },
                            onclick: move |_| page.set(p),
                            Icon { name: icon }
                            "{label}"
                        }
                    }
                    div { class: "mt-auto px-3 text-[11px] text-muted-foreground", "Data stays on this computer." }
                }
                main { class: "flex-1 overflow-y-auto",
                    div { class: "mx-auto flex max-w-6xl flex-col gap-6 p-8",
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

/// Async sleep without pulling in a timer crate: a helper thread wakes a oneshot.
async fn tokio_sleep(ms: u64) {
    let (tx, rx) = futures_channel::oneshot::channel::<()>();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        let _ = tx.send(());
    });
    let _ = rx.await;
}
