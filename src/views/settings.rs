use crate::app::{today, use_store};
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::separator::Separator;
use crate::ui::switch::Switch;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use dioxus::prelude::*;
use penguinfin::db::export_json;
use penguinfin::export::{self as pfexport, Imported};
use penguinfin::model::{AppData, UiScale};
use std::path::PathBuf;

/// Where backups go: the user's Downloads folder (the flatpak is granted access to it).
fn backup_dir() -> PathBuf {
    let home = penguinfin::db::home_dir();
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
                .is_some_and(|n| n.starts_with("penguinfin-") && (n.ends_with(".json") || n.ends_with(".pfexport")))
        })
        .collect();
    // Newest first by time written, whichever kind of file it is.
    files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    files.pop()
}

#[component]
pub fn SettingsPage() -> Element {
    let mut store = use_store();
    let settings = store.data.read().settings.clone();
    let fred_inflation = store.data.read().fred.inflation_source.indicator();
    let mut currency = use_signal(|| settings.currency_symbol.clone());
    // Only what the user typed; otherwise the current (possibly FRED-synced) value is kept.
    let mut inflation = use_signal(|| None::<String>);
    let mut years = use_signal(|| settings.projection_years.to_string());
    let mut import_path = use_signal(|| latest_backup().map(|p| p.display().to_string()).unwrap_or_default());
    let mut confirm_erase = use_signal(|| false);
    // A file that was read and is waiting for "Replace my data".
    let mut pending_import = use_signal(|| None::<Imported>);
    let db_path = store.db_path();

    let s_dark = settings.clone();
    let s_scroll = settings.clone();
    let s_save = settings.clone();
    let save = move |_| {
        let mut s = s_save.clone();
        s.currency_symbol = currency.read().trim().chars().take(4).collect();
        // When inflation follows FRED, the field is read-only and keeps the synced value.
        if fred_inflation.is_none() {
            let typed = inflation.read().clone().unwrap_or_else(|| num_str(s.inflation_pct));
            match parse_num(&typed) {
                Some(v) if (-5.0..=30.0).contains(&v) => s.inflation_pct = v,
                _ => return store.notify("Inflation must be a number between -5 and 30.", true),
            }
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

    let export_v2 = move |_| {
        let data = store.data.read().clone();
        let path = backup_dir().join(format!(
            "penguinfin-{}.{}",
            chrono::Local::now().format("%Y%m%d-%H%M%S"),
            pfexport::EXTENSION
        ));
        match pfexport::write_export(&data, &path, env!("CARGO_PKG_VERSION")) {
            Ok(()) => {
                import_path.set(path.display().to_string());
                store.notify(format!("Export saved to {}", path.display()), false);
            }
            Err(e) => store.notify(format!("Export failed: {e}"), true),
        }
    };

    // Read the file first, so the confirmation can say what it holds.
    let check_import = move |_| {
        let p = PathBuf::from(import_path.read().trim());
        match pfexport::read_file(&p) {
            Ok(found) => pending_import.set(Some(found)),
            Err(e) => store.notify(format!("Could not read that file: {e}"), true),
        }
    };

    let do_import = move |_| {
        let Some(found) = pending_import.take() else { return };
        store.mutate("Data restored", |db| db.replace_all(&found.data));
    };

    rsx! {
        PageHeader { title: "Settings & data" }

        Card {
            CardHeader { CardTitle { "Preferences & interface" } }
            CardContent { class: "flex flex-col gap-4",
                div { class: "grid grid-cols-1 gap-4 @2xl:grid-cols-3",
                    Field { label: "Currency symbol",
                        Input { value: currency(), oninput: move |e: FormEvent| currency.set(e.value()) } }
                    if let Some(ind) = fred_inflation {
                        Field { label: "Inflation % per year",
                            hint: format!("Following FRED: {}. Change this under Economic data below.", ind.label().to_lowercase()),
                            // Keyed by value so it re-mounts (inputs are uncontrolled) when a sync changes it.
                            for v in [num_str(settings.inflation_pct)] {
                                Input { key: "fred-{v}", value: v.clone(), readonly: true }
                            }
                        }
                    } else {
                        Field { label: "Inflation % per year", hint: "Used for 'today's dollars' and as the default for new expenses",
                            for v in [num_str(settings.inflation_pct)] {
                                Input { key: "manual-{v}", value: v.clone(), oninput: move |e: FormEvent| inflation.set(Some(e.value())) }
                            }
                        }
                    }
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
                Separator {}
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    div {
                        div { class: "text-sm font-medium", "Interface size" }
                        div { class: "text-xs text-muted-foreground",
                            "Auto scales everything to fit the window. Shortcuts: Ctrl + bigger, Ctrl − smaller, Ctrl 0 auto." }
                    }
                    div { class: "inline-flex rounded-lg bg-muted p-1", role: "radiogroup", aria_label: "Interface size",
                        for opt in UiScale::ALL {
                            button {
                                key: "{opt.key()}",
                                role: "radio",
                                aria_checked: "{settings.ui_scale == opt}",
                                title: opt.long_label(),
                                class: if settings.ui_scale == opt {
                                    "rounded-md bg-background px-3 py-1 text-sm font-medium shadow-sm"
                                } else {
                                    "rounded-md px-3 py-1 text-sm text-muted-foreground hover:text-foreground cursor-pointer"
                                },
                                onclick: move |_| crate::app::set_ui_scale(&mut store, opt),
                                "{opt.label()}"
                            }
                        }
                    }
                }
                Separator {}
                div { class: "flex items-center justify-between gap-3",
                    div {
                        div { class: "text-sm font-medium", "Natural scrolling" }
                        div { class: "text-xs text-muted-foreground",
                            "Content moves with your fingers, like a phone. Leave off if your system already uses natural scrolling." }
                    }
                    Switch { checked: settings.natural_scroll, aria_label: "Natural scrolling",
                        onchange: move |v| {
                            let mut s = s_scroll.clone();
                            s.natural_scroll = v;
                            store.mutate("", |db| db.save_settings(&s));
                        } }
                }
            }
        }

        FredCard {}

        Card {
            CardHeader {
                CardTitle { "Your data" }
                CardDescription { "Everything is stored in one SQLite file on this computer. Your financial data never leaves it." }
            }
            CardContent { class: "flex flex-col gap-5",
                Field { label: "Database file",
                    code { class: "rounded bg-muted px-2 py-1 text-xs break-all", "{db_path}" } }
                Separator {}
                div { class: "flex flex-col gap-2",
                    div { class: "text-sm font-medium", "Backup" }
                    p { class: "text-xs text-muted-foreground", "Saves a copy of all data to {backup_dir().display()}." }
                    div { class: "flex flex-wrap gap-2",
                        Button { variant: ButtonVariant::Outline, onclick: export, "Export backup" }
                        Button { variant: ButtonVariant::Outline, onclick: export_v2, "Export for PenguinFin 2" }
                    }
                    p { class: "text-xs text-muted-foreground",
                        "The PenguinFin 2 export is a checked file that PenguinFin 2 imports on any computer or phone. Your FRED key is never included." }
                }
                div { class: "flex flex-col gap-2",
                    div { class: "text-sm font-medium", "Restore" }
                    p { class: "text-xs text-muted-foreground",
                        "Replaces all current data with a backup (.json), a PenguinFin 2 export (.pfexport) or a penguinfin.db copied from another computer." }
                    div { class: "flex gap-2",
                        // Keyed by the path so a new export shows up here (inputs are uncontrolled).
                        for v in [import_path()] {
                            Input { key: "{v}", value: v.clone(), placeholder: "/home/you/Downloads/penguinfin-backup-….json",
                                oninput: move |e: FormEvent| { pending_import.set(None); import_path.set(e.value()) } }
                        }
                        if pending_import.read().is_none() {
                            Button { variant: ButtonVariant::Outline, onclick: check_import, "Restore…" }
                        }
                    }
                    if let Some(found) = pending_import() {
                        div { class: "flex flex-col gap-2 rounded-md border bg-muted/40 p-3",
                            p { class: "text-sm",
                                "This file holds {pfexport::summary(&found.counts())}. Restoring replaces everything you have now." }
                            div { class: "flex gap-2",
                                Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive, onclick: do_import, "Replace my data" }
                                Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| pending_import.set(None), "Cancel" }
                            }
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

/// FRED economic data: API key, sync controls, inflation source and the latest values.
#[component]
fn FredCard() -> Element {
    let mut store = use_store();
    let fred = store.data.read().fred.clone();
    let market = store.data.read().market.clone();
    let syncing = (store.syncing)();
    let mut key_text = use_signal(String::new);
    let mut key_gen = use_signal(|| 0u32);
    let source_options: Vec<(String, String)> =
        InflationSource::ALL.iter().map(|s| (s.key().to_string(), s.label().to_string())).collect();

    let save_config = move |store: &mut crate::app::Store, cfg: FredConfig, msg: &str| {
        store.mutate(msg, |db| {
            db.save_fred_config(&cfg)?;
            crate::app::apply_market_inflation(db)
        });
    };

    let fred_save = fred.clone();
    let save_key = move |_| {
        let k = key_text.read().trim().to_string();
        if !fred::valid_api_key(&k) {
            return store.notify("A FRED API key is 32 lowercase letters and digits.", true);
        }
        let cfg = FredConfig { api_key: k, ..fred_save.clone() };
        save_config(&mut store, cfg, "FRED key saved");
        key_text.set(String::new());
        key_gen += 1;
        crate::app::sync_market_data(store, true);
    };
    let fred_remove = fred.clone();
    let fred_launch = fred.clone();
    let fred_source = fred.clone();

    rsx! {
        Card {
            CardHeader {
                CardTitle { "Economic data (FRED)" }
                CardDescription {
                    "Optional. Pulls inflation, interest-rate and market figures from the St. Louis Fed's FRED service to keep your projection assumptions realistic. "
                    "Only these public figures are downloaded; nothing about your finances is sent."
                }
            }
            CardContent { class: "flex flex-col gap-5",
                if fred.enabled() {
                    div { class: "flex flex-wrap items-center justify-between gap-3",
                        div {
                            div { class: "text-sm font-medium", "API key" }
                            div { class: "text-xs text-muted-foreground font-mono", "••••••••••••••••••••••••••••{key_tail(&fred.api_key)}" }
                        }
                        Button { variant: ButtonVariant::Ghost, class: "text-destructive",
                            onclick: move |_| {
                                let cfg = FredConfig { api_key: String::new(), ..fred_remove.clone() };
                                save_config(&mut store, cfg, "FRED key removed; syncing is off");
                            },
                            "Remove key" }
                    }
                } else {
                    div { class: "flex flex-col gap-2",
                        div { class: "text-sm font-medium", "API key" }
                        p { class: "text-xs text-muted-foreground",
                            "Get a free key at "
                            a { class: "underline", href: fred::KEY_SIGNUP_URL, target: "_blank", "{fred::KEY_SIGNUP_URL}" }
                            " (create an account, then “Request API Key”), and paste it here."
                        }
                        div { class: "flex gap-2",
                            for g in [key_gen()] {
                                Input { key: "{g}", r#type: InputType::Password, placeholder: "32-character key",
                                    oninput: move |e: FormEvent| key_text.set(e.value()) }
                            }
                            Button { onclick: save_key, "Save & sync" }
                        }
                    }
                }

                Separator {}
                div { class: "flex flex-wrap items-center justify-between gap-3",
                    div {
                        div { class: "text-sm font-medium", "Sync" }
                        div { class: "text-xs text-muted-foreground",
                            if syncing { "Syncing…" }
                            else if let Some(t) = &market.last_sync { "Last synced {t}" }
                            else { "Never synced" }
                        }
                    }
                    div { class: "flex items-center gap-4",
                        div { class: "flex items-center gap-2",
                            span { class: "text-sm", "On launch" }
                            Switch { checked: fred.sync_on_launch, aria_label: "Sync when PenguinFin starts",
                                onchange: move |v| {
                                    let cfg = FredConfig { sync_on_launch: v, ..fred_launch.clone() };
                                    save_config(&mut store, cfg, "");
                                } }
                        }
                        Button { variant: ButtonVariant::Outline, disabled: syncing || !fred.enabled(),
                            onclick: move |_| crate::app::sync_market_data(store, true),
                            if syncing { "Syncing…" } else { "Sync now" } }
                    }
                }
                if let Some(err) = &market.last_error {
                    p { class: "text-xs text-destructive", "Last sync problem: {err}" }
                }

                Field { label: "Inflation assumption",
                    hint: "Used for projections in today's dollars and as the default for new expenses.",
                    NativeSelect {
                        value: fred.inflation_source.key().to_string(),
                        options: source_options,
                        onchange: move |v: String| {
                            if let Some(src) = InflationSource::from_key(&v) {
                                let cfg = FredConfig { inflation_source: src, ..fred_source.clone() };
                                save_config(&mut store, cfg, "");
                            }
                        },
                    }
                }

                if !market.values.is_empty() {
                    div { class: "grid grid-cols-1 gap-3 @2xl:grid-cols-2 @5xl:grid-cols-3",
                        for v in market.values.iter() {
                            div { key: "{v.indicator.key()}", class: "rounded-lg border p-3",
                                div { class: "text-xs text-muted-foreground", "{v.indicator.label()}" }
                                div { class: "text-xl font-semibold tabular-nums", "{v.value:.2}%" }
                                div { class: "text-xs text-muted-foreground", "{v.indicator.note()} As of {v.as_of.format(\"%b %-d, %Y\")}." }
                            }
                        }
                    }
                    p { class: "text-xs text-muted-foreground", "Source: FRED®, Federal Reserve Bank of St. Louis." }
                }
            }
        }
    }
}

fn key_tail(key: &str) -> &str {
    &key[key.len().saturating_sub(4)..]
}

use penguinfin::fred::{self, FredConfig, InflationSource};
