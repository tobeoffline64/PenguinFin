use crate::app::{today, use_store};
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::separator::Separator;
use crate::ui::switch::Switch;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use dioxus::prelude::*;
use penguinfin::db::{export_json, import_json};
use penguinfin::model::AppData;
use std::path::PathBuf;

/// Where backups go: the user's Downloads folder (the flatpak is granted access to it).
fn backup_dir() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    if let Some(d) = std::env::var_os("XDG_DOWNLOAD_DIR").map(PathBuf::from).filter(|p| p.is_absolute()) {
        return d;
    }
    home.join("Downloads")
}

fn latest_backup() -> Option<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(backup_dir())
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("penguinfin-backup-") && n.ends_with(".json"))
        })
        .collect();
    files.sort();
    files.pop()
}

#[component]
pub fn SettingsPage() -> Element {
    let mut store = use_store();
    let settings = store.data.read().settings.clone();
    let mut currency = use_signal(|| settings.currency_symbol.clone());
    let mut inflation = use_signal(|| num_str(settings.inflation_pct));
    let mut years = use_signal(|| settings.projection_years.to_string());
    let mut import_path = use_signal(|| latest_backup().map(|p| p.display().to_string()).unwrap_or_default());
    let mut confirm_erase = use_signal(|| false);
    let mut confirm_import = use_signal(|| false);
    let db_path = store.db_path();

    let s_dark = settings.clone();
    let s_save = settings.clone();
    let save = move |_| {
        let mut s = s_save.clone();
        s.currency_symbol = currency.read().trim().chars().take(4).collect();
        match parse_num(&inflation.read()) {
            Some(v) if (-5.0..=30.0).contains(&v) => s.inflation_pct = v,
            _ => return store.notify("Inflation must be a number between -5 and 30.", true),
        }
        match years.read().trim().parse::<u32>() {
            Ok(y) if (1..=80).contains(&y) => s.projection_years = y,
            _ => return store.notify("Projection years must be between 1 and 80.", true),
        }
        store.mutate("Settings saved", |db| db.save_settings(&s));
    };

    let export = move |_| {
        let data = store.data.read().clone();
        let path = backup_dir().join(format!("penguinfin-backup-{}.json", chrono::Local::now().format("%Y%m%d-%H%M%S")));
        match export_json(&data, &path) {
            Ok(()) => {
                import_path.set(path.display().to_string());
                store.notify(format!("Backup saved to {}", path.display()), false);
            }
            Err(e) => store.notify(format!("Backup failed: {e}"), true),
        }
    };

    let do_import = move |_| {
        confirm_import.set(false);
        let p = PathBuf::from(import_path.read().trim());
        match import_json(&p) {
            Ok(d) => store.mutate("Backup restored", |db| db.replace_all(&d)),
            Err(e) => store.notify(format!("Could not read backup: {e}"), true),
        }
    };

    rsx! {
        PageHeader { title: "Settings & data" }

        Card {
            CardHeader { CardTitle { "Preferences" } }
            CardContent { class: "flex flex-col gap-4",
                div { class: "grid grid-cols-1 gap-4 md:grid-cols-3",
                    Field { label: "Currency symbol",
                        Input { value: currency(), oninput: move |e: FormEvent| currency.set(e.value()) } }
                    Field { label: "Inflation % per year", hint: "Used for 'today's dollars' and as the default for new expenses",
                        Input { value: inflation(), oninput: move |e: FormEvent| inflation.set(e.value()) } }
                    Field { label: "Default projection length (years)",
                        Input { value: years(), oninput: move |e: FormEvent| years.set(e.value()) } }
                }
                div { Button { onclick: save, "Save preferences" } }
                Separator {}
                div { class: "flex items-center justify-between",
                    div {
                        div { class: "text-sm font-medium", "Dark mode" }
                        div { class: "text-xs text-muted-foreground", "Saved with your data" }
                    }
                    Switch { checked: settings.dark_mode, aria_label: "Dark mode",
                        onchange: move |v| {
                            let mut s = s_dark.clone();
                            s.dark_mode = v;
                            store.mutate("", |db| db.save_settings(&s));
                        } }
                }
            }
        }

        Card {
            CardHeader {
                CardTitle { "Your data" }
                CardDescription { "Everything is stored in one SQLite file on this computer. Nothing is sent anywhere." }
            }
            CardContent { class: "flex flex-col gap-5",
                Field { label: "Database file",
                    code { class: "rounded bg-muted px-2 py-1 text-xs break-all", "{db_path}" } }
                Separator {}
                div { class: "flex flex-col gap-2",
                    div { class: "text-sm font-medium", "Backup" }
                    p { class: "text-xs text-muted-foreground", "Saves a JSON copy of all data to {backup_dir().display()}." }
                    div { Button { variant: ButtonVariant::Outline, onclick: export, "Export backup" } }
                }
                div { class: "flex flex-col gap-2",
                    div { class: "text-sm font-medium", "Restore" }
                    p { class: "text-xs text-muted-foreground", "Replaces all current data with the contents of a backup file." }
                    div { class: "flex gap-2",
                        Input { value: import_path(), placeholder: "/home/you/Downloads/penguinfin-backup-….json",
                            oninput: move |e: FormEvent| import_path.set(e.value()) }
                        if confirm_import() {
                            Button { variant: ButtonVariant::Destructive, onclick: do_import, "Replace my data" }
                            Button { variant: ButtonVariant::Ghost, onclick: move |_| confirm_import.set(false), "Cancel" }
                        } else {
                            Button { variant: ButtonVariant::Outline, onclick: move |_| confirm_import.set(true), "Restore…" }
                        }
                    }
                }
                Separator {}
                div { class: "flex flex-wrap items-center gap-2",
                    Button { variant: ButtonVariant::Secondary,
                        onclick: move |_| {
                            let mut d = penguinfin::sample::sample_data(today());
                            d.settings = store.data.read().settings.clone();
                            store.mutate("Sample data loaded", |db| db.replace_all(&d));
                        },
                        "Replace with sample data" }
                    if confirm_erase() {
                        Button { variant: ButtonVariant::Destructive,
                            onclick: move |_| {
                                confirm_erase.set(false);
                                let d = AppData { settings: store.data.read().settings.clone(), ..Default::default() };
                                store.mutate("All data erased", |db| db.replace_all(&d));
                            },
                            "Yes, erase everything" }
                        Button { variant: ButtonVariant::Ghost, onclick: move |_| confirm_erase.set(false), "Cancel" }
                    } else {
                        Button { variant: ButtonVariant::Ghost, class: "text-destructive", onclick: move |_| confirm_erase.set(true), "Erase all data…" }
                    }
                }
            }
        }

        Card {
            CardHeader { CardTitle { "About" } }
            CardContent { class: "text-sm text-muted-foreground flex flex-col gap-1",
                p { "PenguinFin {env!(\"CARGO_PKG_VERSION\")} · built with Rust, Dioxus and Rust/UI components." }
                p { "Stock prices: {penguinfin::prices::ManualPrices.name()}. An online quote provider can be plugged in later." }
            }
        }
    }
}

use penguinfin::prices::PriceProvider as _;
