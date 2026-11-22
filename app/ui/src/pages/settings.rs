//! Settings & data, ported from 1.x. Saving goes through backend commands; files are chosen in
//! the system's own dialogs, and the FRED key is sent once and never comes back to the interface.

use crate::api::{self, NoArgs};
use crate::app::{ImportConfirm, SettingsArg, use_store};
use crate::ui::*;
use leptos::prelude::*;
use leptos::task::spawn_local;
use penguinfin_core::fred::{self, InflationSource};
use penguinfin_core::format::{num_str, parse_num};
use penguinfin_core::ipc::{FredOptions, ImportPreview, Snapshot, SyncOutcome};
use penguinfin_core::model::{Settings, UiScale};
use serde::Serialize;

fn settings_now() -> Settings {
    use_store().snap.with_untracked(|s| s.as_ref().map(|s| s.data.settings.clone()).unwrap_or_default())
}

#[component]
pub fn SettingsPage() -> impl IntoView {
    view! {
        <PageHeader title="Settings & data" />
        <Preferences />
        <FredCard />
        <YourData />
        <About />
    }
}

#[component]
fn Preferences() -> impl IntoView {
    let store = use_store();
    let s0 = settings_now();
    let currency = RwSignal::new(s0.currency_symbol.clone());
    let inflation = RwSignal::new(num_str(s0.inflation_pct));
    let years = RwSignal::new(s0.projection_years.to_string());
    let settings = move || store.snap.with(|s| s.as_ref().map(|s| s.data.settings.clone()).unwrap_or_default());
    let fred_source = move || store.snap.with(|s| s.as_ref().map(|s| s.fred.inflation_source).unwrap_or_default());
    // A FRED sync can change the inflation figure; show the new value when it does.
    Effect::new(move |_| {
        if fred_source().indicator().is_some() {
            inflation.set(num_str(settings().inflation_pct));
        }
    });

    let save = move |_| {
        let mut s = settings_now();
        s.currency_symbol = currency.get_untracked().trim().chars().take(4).collect();
        if fred_source().indicator().is_none() {
            match parse_num(&inflation.get_untracked()) {
                Some(v) if (-5.0..=30.0).contains(&v) => s.inflation_pct = v,
                _ => return store.notify("Inflation must be a number between -5 and 30.", true),
            }
        }
        match years.get_untracked().trim().parse::<u32>() {
            Ok(y) if (1..=80).contains(&y) => s.projection_years = y,
            _ => return store.notify("Projection years must be between 1 and 80.", true),
        }
        store.call("save_settings", SettingsArg { settings: s }, "Settings saved");
    };
    let change = move |f: Box<dyn Fn(&mut Settings)>| {
        let mut s = settings_now();
        f(&mut s);
        store.call("save_settings", SettingsArg { settings: s }, "");
    };
    let shortcut = move || if store.platform() == "macos" { "⌘+ bigger, ⌘− smaller, ⌘0 auto" } else { "Ctrl + bigger, Ctrl − smaller, Ctrl 0 auto" };

    view! {
        <Card>
            <CardHeader><CardTitle>"Preferences & interface"</CardTitle></CardHeader>
            <CardContent class="flex flex-col gap-4">
                <div class="grid grid-cols-1 gap-4 @2xl:grid-cols-3">
                    <Field label="Currency symbol"><Input value=currency id="currency" /></Field>
                    {move || match fred_source().indicator() {
                        Some(ind) => view! {
                            <Field label="Inflation % per year" hint=format!("Following FRED: {}. Change this under Economic data below.", ind.label().to_lowercase())>
                                <Input value=inflation readonly=true id="inflation" />
                            </Field>
                        }.into_any(),
                        None => view! {
                            <Field label="Inflation % per year" hint="Used for 'today's dollars' and as the default for new expenses">
                                <Input value=inflation id="inflation" />
                            </Field>
                        }.into_any(),
                    }}
                    <Field label="Default projection length (years)"><Input value=years id="years" /></Field>
                </div>
                <div><Button on_click=save>"Save preferences"</Button></div>
                <Separator />
                <div class="flex items-center justify-between gap-3">
                    <div>
                        <div class="text-sm font-medium">"Dark mode"</div>
                        <div class="text-xs text-muted-foreground">"Saved with your data"</div>
                    </div>
                    <Switch checked=Signal::derive(move || settings().dark_mode) label="Dark mode"
                        on_change=move |v| change(Box::new(move |s| s.dark_mode = v)) />
                </div>
                <Separator />
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                        <div class="text-sm font-medium">"Interface size"</div>
                        <div class="text-xs text-muted-foreground">{move || format!("Auto scales everything to fit the window. Shortcuts: {}.", shortcut())}</div>
                    </div>
                    <div class="inline-flex rounded-lg bg-muted p-1" role="radiogroup" aria-label="Interface size">
                        {UiScale::ALL
                            .into_iter()
                            .map(|opt| {
                                view! {
                                    <button
                                        role="radio"
                                        title=opt.long_label()
                                        aria-checked=move || (settings().ui_scale == opt).to_string()
                                        class=move || {
                                            if settings().ui_scale == opt {
                                                "rounded-md bg-background px-3 py-1 text-sm font-medium shadow-sm"
                                            } else {
                                                "rounded-md px-3 py-1 text-sm text-muted-foreground hover:text-foreground cursor-pointer"
                                            }
                                        }
                                        on:click=move |_| change(Box::new(move |s| s.ui_scale = opt))
                                    >
                                        {opt.label()}
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                </div>
                <Separator />
                <div class="flex items-center justify-between gap-3">
                    <div>
                        <div class="text-sm font-medium">"Natural scrolling"</div>
                        <div class="text-xs text-muted-foreground">
                            "Content moves with your fingers, like a phone. Leave off if your system already uses natural scrolling."
                        </div>
                    </div>
                    <Switch checked=Signal::derive(move || settings().natural_scroll) label="Natural scrolling"
                        on_change=move |v| change(Box::new(move |s| s.natural_scroll = v)) />
                </div>
            </CardContent>
        </Card>
    }
}

#[derive(Serialize)]
struct KeyArg {
    key: String,
}
#[derive(Serialize)]
struct OptionsArg {
    options: FredOptions,
}

#[component]
fn FredCard() -> impl IntoView {
    let store = use_store();
    let key_text = RwSignal::new(String::new());
    let syncing = RwSignal::new(false);
    let snap = move || store.snap.get().unwrap_or_default();

    let sync = move || {
        syncing.set(true);
        spawn_local(async move {
            match api::invoke::<SyncOutcome, _>("sync_market_data", &NoArgs {}).await {
                Ok(o) => {
                    store.snap.set(Some(o.snapshot));
                    store.notify(o.message, o.error);
                }
                Err(e) => store.notify(e, true),
            }
            syncing.set(false);
        });
    };
    let save_key = move |_| {
        let key = key_text.get_untracked().trim().to_string();
        if !fred::valid_api_key(&key) {
            return store.notify("A FRED API key is 32 lowercase letters and digits.", true);
        }
        spawn_local(async move {
            match api::invoke::<Snapshot, _>("set_fred_key", &KeyArg { key }).await {
                Ok(s) => {
                    store.snap.set(Some(s));
                    key_text.set(String::new());
                    store.notify("FRED key saved", false);
                    sync();
                }
                Err(e) => store.notify(e, true),
            }
        });
    };
    let save_options = move |sync_on_launch: bool, inflation_source: InflationSource| {
        store.call("save_fred_options", OptionsArg { options: FredOptions { sync_on_launch, inflation_source } }, "");
    };
    let source_options: Vec<(String, String)> =
        InflationSource::ALL.iter().map(|s| (s.key().to_string(), s.label().to_string())).collect();

    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Economic data (FRED)"</CardTitle>
                <CardDescription>
                    "Optional. Pulls inflation, interest-rate and market figures from the St. Louis Fed's FRED service to keep your projection assumptions realistic. Only these public figures are downloaded; nothing about your finances is sent."
                </CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-5">
                {move || {
                    let k = snap().fred_key;
                    if k.set {
                        view! {
                            <div class="flex flex-wrap items-center justify-between gap-3">
                                <div>
                                    <div class="text-sm font-medium">"API key"</div>
                                    <div class="text-xs text-muted-foreground">
                                        <span class="font-mono">{format!("••••••••••••••••••••••••••••{}", k.tail)}</span>
                                        {format!(" · kept in {}", k.stored_in)}
                                    </div>
                                </div>
                                <Button variant=ButtonVariant::Ghost class="text-destructive"
                                    on_click=move |_| store.call("remove_fred_key", NoArgs {}, "FRED key removed; syncing is off")>
                                    "Remove key"
                                </Button>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="flex flex-col gap-2">
                                <div class="text-sm font-medium">"API key"</div>
                                <p class="text-xs text-muted-foreground">
                                    "Get a free key at " <span class="font-mono select-all">{fred::KEY_SIGNUP_URL}</span>
                                    " (create an account, then “Request API Key”), and paste it here. It is kept in your system's keychain."
                                </p>
                                <div class="flex gap-2">
                                    <Input value=key_text kind="password" placeholder="32-character key" id="fred-key" />
                                    <Button on_click=save_key>"Save & sync"</Button>
                                </div>
                            </div>
                        }.into_any()
                    }
                }}
                <Separator />
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <div>
                        <div class="text-sm font-medium">"Sync"</div>
                        <div class="text-xs text-muted-foreground">
                            {move || {
                                if syncing.get() {
                                    "Syncing…".to_string()
                                } else {
                                    snap().market.last_sync.map(|t| format!("Last synced {t}")).unwrap_or_else(|| "Never synced".into())
                                }
                            }}
                        </div>
                    </div>
                    <div class="flex items-center gap-4">
                        <div class="flex items-center gap-2">
                            <span class="text-sm">"On launch"</span>
                            <Switch checked=Signal::derive(move || snap().fred.sync_on_launch) label="Sync when PenguinFin starts"
                                on_change=move |v| save_options(v, snap().fred.inflation_source) />
                        </div>
                        <Button variant=ButtonVariant::Outline disabled=Signal::derive(move || syncing.get() || !snap().fred_key.set)
                            on_click=move |_| sync()>
                            {move || if syncing.get() { "Syncing…" } else { "Sync now" }}
                        </Button>
                    </div>
                </div>
                {move || snap().market.last_error.map(|e| view! { <p class="text-xs text-destructive">{format!("Last sync problem: {e}")}</p> })}
                <Field label="Inflation assumption" hint="Used for projections in today's dollars and as the default for new expenses.">
                    <NativeSelect
                        label="Inflation assumption"
                        value=Signal::derive(move || snap().fred.inflation_source.key().to_string())
                        options=source_options
                        on_change=move |v: String| {
                            if let Some(src) = InflationSource::from_key(&v) {
                                save_options(snap().fred.sync_on_launch, src)
                            }
                        }
                    />
                </Field>
                {move || {
                    let values = snap().market.values;
                    (!values.is_empty()).then(|| view! {
                        <div class="grid grid-cols-1 gap-3 @2xl:grid-cols-2 @5xl:grid-cols-3">
                            {values.into_iter().map(|v| view! {
                                <div class="rounded-lg border p-3">
                                    <div class="text-xs text-muted-foreground">{v.indicator.label()}</div>
                                    <div class="text-xl font-semibold tabular-nums">{format!("{:.2}%", v.value)}</div>
                                    <div class="text-xs text-muted-foreground">{format!("{} As of {}.", v.indicator.note(), v.as_of.format("%b %-d, %Y"))}</div>
                                </div>
                            }).collect_view()}
                        </div>
                        <p class="text-xs text-muted-foreground">"Source: FRED®, Federal Reserve Bank of St. Louis."</p>
                    })
                }}
            </CardContent>
        </Card>
    }
}

#[derive(Serialize)]
struct KindArg {
    kind: &'static str,
}

#[component]
fn YourData() -> impl IntoView {
    let store = use_store();
    let preview = RwSignal::new(None::<ImportPreview>);
    let confirm_erase = RwSignal::new(false);
    let db = move || store.snap.with(|s| s.as_ref().map(|s| s.db.clone()).unwrap_or_default());

    let save_copy = move |cmd: &'static str, what: &'static str| {
        spawn_local(async move {
            match api::invoke::<Option<String>, _>(cmd, &NoArgs {}).await {
                Ok(Some(path)) => store.notify(format!("{what} saved to {path}"), false),
                Ok(None) => {}
                Err(e) => store.notify(format!("{what} failed: {e}"), true),
            }
        })
    };
    let pick = move |_| {
        spawn_local(async move {
            match api::invoke::<Option<ImportPreview>, _>("pick_import_file", &NoArgs {}).await {
                Ok(p) => preview.set(p),
                Err(e) => store.notify(format!("Could not read that file: {e}"), true),
            }
        })
    };

    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Your data"</CardTitle>
                <CardDescription>"Everything is stored in one SQLite file on this computer. Your financial data never leaves it."</CardDescription>
            </CardHeader>
            <CardContent class="flex flex-col gap-5">
                <Field label="Database file">
                    <code class="rounded bg-muted px-2 py-1 text-xs break-all">{move || db().path}</code>
                </Field>
                {move || db().migrated_from.map(|v| view! {
                    <p class="text-xs text-muted-foreground">{format!("Upgraded from PenguinFin 1 (data format {v}). Format now {}.", db().schema_version)}</p>
                })}
                <Separator />
                <div class="flex flex-col gap-2">
                    <div class="text-sm font-medium">"Backup"</div>
                    <p class="text-xs text-muted-foreground">"Save a copy of all your data. You choose where; your Downloads folder is suggested."</p>
                    <div class="flex flex-wrap gap-2">
                        <Button variant=ButtonVariant::Outline on_click=move |_| save_copy("export_backup", "Backup")>"Export backup"</Button>
                        <Button variant=ButtonVariant::Outline on_click=move |_| save_copy("export_for_v2", "Export")>"Export a checked copy (.pfexport)"</Button>
                    </div>
                    <p class="text-xs text-muted-foreground">"The checked copy can be imported by PenguinFin 2 on any computer or phone. Your FRED key is never included."</p>
                </div>
                <div class="flex flex-col gap-2">
                    <div class="text-sm font-medium">"Restore or import"</div>
                    <p class="text-xs text-muted-foreground">
                        "Replaces all current data with a backup (.json), an export (.pfexport) or a penguinfin.db from another computer. Your current data is first saved as a copy beside the database."
                    </p>
                    {move || match preview.get() {
                        None => view! { <div><Button variant=ButtonVariant::Outline on_click=pick>"Choose a file…"</Button></div> }.into_any(),
                        Some(p) => view! { <ImportConfirm preview=p on_done=Callback::new(move |_| preview.set(None)) /> }.into_any(),
                    }}
                </div>
                <Separator />
                <div class="flex flex-wrap items-center gap-2">
                    <Button variant=ButtonVariant::Secondary on_click=move |_| store.call("reset_data", KindArg { kind: "sample" }, "Sample data loaded")>
                        "Replace with sample data"
                    </Button>
                    {move || if confirm_erase.get() {
                        view! {
                            <Button variant=ButtonVariant::Destructive on_click=move |_| {
                                confirm_erase.set(false);
                                store.call("reset_data", KindArg { kind: "empty" }, "All data erased");
                            }>"Yes, erase everything"</Button>
                            <Button variant=ButtonVariant::Ghost on_click=move |_| confirm_erase.set(false)>"Cancel"</Button>
                        }.into_any()
                    } else {
                        view! {
                            <Button variant=ButtonVariant::Ghost class="text-destructive" on_click=move |_| confirm_erase.set(true)>"Erase all data…"</Button>
                        }.into_any()
                    }}
                </div>
            </CardContent>
        </Card>
    }
}

#[component]
fn About() -> impl IntoView {
    let store = use_store();
    let version = move || store.boot.with(|b| b.as_ref().map(|b| b.app_version.clone()).unwrap_or_default());
    view! {
        <Card>
            <CardHeader><CardTitle>"About"</CardTitle></CardHeader>
            <CardContent class="text-sm text-muted-foreground flex flex-col gap-1">
                <p>{move || format!("PenguinFin {} (2.0 preview) · built with Rust, Tauri, Leptos and Rust/UI components.", version())}</p>
            </CardContent>
        </Card>
    }
}
