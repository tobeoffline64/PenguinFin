//! App state, the shell (sidebar, toasts) and the first-launch screens.

use crate::api::{self, NoArgs};
use crate::pages;
use crate::ui::icon::{Icon, IconName};
use crate::ui::*;
use leptos::prelude::*;
use leptos::task::spawn_local;
use penguinfin_core::format::money;
use penguinfin_core::ipc::{Boot, BootStatus, ImportPreview, Snapshot};
use penguinfin_core::model::UiScale;
use serde::Serialize;
use std::time::Duration;
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Overview,
    Income,
    Bills,
    Expenses,
    Debts,
    Accounts,
    Goals,
    Projections,
    Settings,
}

const NAV: [(Page, &str, IconName); 9] = [
    (Page::Overview, "Dashboard", IconName::Dashboard),
    (Page::Income, "Income", IconName::Income),
    (Page::Bills, "Bills", IconName::Bill),
    (Page::Expenses, "Recurring expenses", IconName::Expense),
    (Page::Debts, "Debt payoff", IconName::CreditCard),
    (Page::Accounts, "Accounts & investments", IconName::Accounts),
    (Page::Goals, "Savings goals", IconName::Target),
    (Page::Projections, "Projections", IconName::Chart),
    (Page::Settings, "Settings & data", IconName::Settings),
];

#[derive(Clone, PartialEq)]
pub struct Flash {
    pub text: String,
    pub error: bool,
    id: u32,
}

/// Shared state. Every change goes through `call`: the backend writes, sends back a fresh
/// `Snapshot`, and a toast says what happened (the same flow as 1.x's `store.mutate`).
#[derive(Clone, Copy)]
pub struct Store {
    pub snap: RwSignal<Option<Snapshot>>,
    pub flash: RwSignal<Option<Flash>>,
    pub boot: RwSignal<Option<Boot>>,
    /// The PenguinFin 1 copy saved during an upgrade at this start, until dismissed.
    pub upgraded: RwSignal<Option<String>>,
    flash_id: RwSignal<u32>,
}

impl Store {
    pub fn notify(&self, text: impl Into<String>, error: bool) {
        let id = self.flash_id.get_untracked() + 1;
        self.flash_id.set(id);
        self.flash.set(Some(Flash { text: text.into(), error, id }));
        let flash = self.flash;
        set_timeout(
            move || {
                if flash.get_untracked().is_some_and(|f| f.id == id) {
                    flash.set(None);
                }
            },
            Duration::from_millis(if error { 8000 } else { 3500 }),
        );
    }

    /// Run a command that returns a new snapshot. `ok` is the toast on success ("" = none).
    pub fn call<A: Serialize + 'static>(&self, cmd: &'static str, args: A, ok: &'static str) {
        let store = *self;
        spawn_local(async move {
            match api::invoke::<Snapshot, _>(cmd, &args).await {
                Ok(s) => {
                    store.snap.set(Some(s));
                    if !ok.is_empty() {
                        store.notify(ok, false);
                    }
                }
                Err(e) => store.notify(format!("Could not save: {e}"), true),
            }
        });
    }

    pub fn currency(&self) -> String {
        self.snap.with(|s| s.as_ref().map(|s| s.data.settings.currency_symbol.clone()).unwrap_or_else(|| "$".into()))
    }

    pub fn platform(&self) -> String {
        self.boot.with(|b| b.as_ref().map(|b| b.platform.clone()).unwrap_or_default())
    }
}

pub fn use_store() -> Store {
    expect_context::<Store>()
}

fn root() -> Option<web_sys::HtmlElement> {
    web_sys::window()?.document()?.document_element()?.dyn_into::<web_sys::HtmlElement>().ok()
}

/// Theme, text size and natural scrolling follow the saved settings.
fn apply_settings(snap: &Snapshot) {
    let s = &snap.data.settings;
    if let Some(r) = root() {
        let _ = if s.dark_mode { r.class_list().add_1("dark") } else { r.class_list().remove_1("dark") };
        let _ = r.style().set_property("font-size", s.ui_scale.root_font_size());
    }
    if let Some(w) = web_sys::window() {
        let _ = js_sys::Reflect::set(&w, &"__pfNatural".into(), &s.natural_scroll.into());
    }
}

#[component]
pub fn App() -> impl IntoView {
    let store = Store {
        snap: RwSignal::new(None),
        flash: RwSignal::new(None),
        boot: RwSignal::new(None),
        upgraded: RwSignal::new(None),
        flash_id: RwSignal::new(0),
    };
    provide_context(store);
    let boot_error = RwSignal::new(None::<String>);

    spawn_local(async move {
        match api::invoke::<Boot, _>("boot", &NoArgs {}).await {
            Ok(b) => {
                if let Some(r) = root() {
                    let _ = r.set_attribute("data-platform", &b.platform);
                    let _ = r.set_attribute("data-form", &b.form);
                }
                if let BootStatus::Ready { upgraded_backup: Some(p) } = &b.status {
                    store.upgraded.set(Some(p.clone()));
                }
                store.snap.set(b.snapshot.clone());
                store.boot.set(Some(b));
            }
            Err(e) => boot_error.set(Some(e)),
        }
    });
    Effect::new(move |_| store.snap.with(|s| if let Some(s) = s { apply_settings(s) }));

    // Ctrl/Cmd + / - / 0 from bridge.js.
    let on_scale = Closure::<dyn Fn(web_sys::CustomEvent)>::new(move |e: web_sys::CustomEvent| {
        let Some(snap) = store.snap.get_untracked() else { return };
        let mut s = snap.data.settings.clone();
        s.ui_scale = match e.detail().as_string().as_deref() {
            Some("bigger") => s.ui_scale.bigger(),
            Some("smaller") => s.ui_scale.smaller(),
            _ => UiScale::Auto,
        };
        store.call("save_settings", SettingsArg { settings: s }, "");
    });
    if let Some(w) = web_sys::window() {
        let _ = w.add_event_listener_with_callback("pf-scale", on_scale.as_ref().unchecked_ref());
    }
    on_scale.forget();

    // Which screen to show. A memo, so saving data (a new snapshot) doesn't rebuild the shell
    // and lose the current page: only a change of screen does.
    let screen = Memo::new(move |_| {
        if let Some(e) = boot_error.get() {
            return Screen::Failed(e);
        }
        match store.boot.with(|b| b.as_ref().map(|b| b.status.clone())) {
            None => Screen::Starting,
            Some(BootStatus::Failed { error }) => Screen::Failed(error),
            Some(_) if store.snap.with(|s| s.is_none()) => Screen::Welcome,
            Some(_) => Screen::Main,
        }
    });

    view! {
        {move || match screen.get() {
            Screen::Starting => view! { <div class="h-full bg-background"></div> }.into_any(),
            Screen::Failed(error) => view! { <ErrorScreen error=error /> }.into_any(),
            Screen::Welcome => view! { <Welcome /> }.into_any(),
            Screen::Main => view! { <Shell /> }.into_any(),
        }}
        <Toast />
    }
}

#[derive(Clone, PartialEq)]
enum Screen {
    Starting,
    Failed(String),
    Welcome,
    Main,
}

#[derive(Serialize)]
pub struct SettingsArg {
    pub settings: penguinfin_core::model::Settings,
}

#[component]
fn Toast() -> impl IntoView {
    let store = use_store();
    move || {
        store.flash.get().map(|f| {
            let class = if f.error {
                "fixed bottom-6 right-6 z-50 max-w-md rounded-lg border border-destructive bg-card px-4 py-3 text-sm text-destructive shadow-lg"
            } else {
                "fixed bottom-6 right-6 z-50 max-w-md rounded-lg border bg-card px-4 py-3 text-sm text-card-foreground shadow-lg"
            };
            view! { <div role="status" class=class on:click=move |_| store.flash.set(None)>{f.text}</div> }
        })
    }
}

#[component]
fn ErrorScreen(error: String) -> impl IntoView {
    let store = use_store();
    let path = store.boot.with_untracked(|b| b.as_ref().map(|b| b.db_path.clone()).unwrap_or_default());
    view! {
        <div class="mx-auto flex max-w-2xl flex-col gap-4 p-10">
            <h1 class="text-xl font-semibold">"PenguinFin could not open your data"</h1>
            <p class="text-sm text-muted-foreground">"Nothing was changed. The file is at:"</p>
            <code class="rounded bg-muted px-2 py-1 text-xs break-all">{path}</code>
            <pre class="text-sm text-destructive whitespace-pre-wrap">{error}</pre>
        </div>
    }
}

/// First launch with no database: import from PenguinFin 1, load the sample, or start empty.
#[component]
fn Welcome() -> impl IntoView {
    let store = use_store();
    let preview = RwSignal::new(None::<ImportPreview>);
    let pick = move |_| {
        spawn_local(async move {
            match api::invoke::<Option<ImportPreview>, _>("pick_import_file", &NoArgs {}).await {
                Ok(p) => preview.set(p),
                Err(e) => store.notify(format!("Could not read that file: {e}"), true),
            }
        })
    };
    #[derive(Serialize)]
    struct Kind {
        kind: &'static str,
    }
    view! {
        <div class="@container/app flex min-h-full items-center justify-center bg-background p-6">
            <div class="flex w-full max-w-xl flex-col gap-6">
                <div class="flex flex-col gap-1">
                    <h1 class="text-2xl font-semibold tracking-tight">"Welcome to PenguinFin 2"</h1>
                    <p class="text-sm text-muted-foreground">"Your data stays on this computer. How would you like to start?"</p>
                </div>
                <Card>
                    <CardHeader>
                        <CardTitle>"Bring your data from PenguinFin 1"</CardTitle>
                        <CardDescription>
                            "Choose a .pfexport file (Settings › Export for PenguinFin 2 in version 1.10), a backup (.json), or a penguinfin.db copied from another computer."
                        </CardDescription>
                    </CardHeader>
                    <CardContent class="flex flex-col gap-3">
                        {move || match preview.get() {
                            None => view! { <Button on_click=pick><Icon name=IconName::Plus />"Choose a file…"</Button> }.into_any(),
                            Some(p) => view! { <ImportConfirm preview=p on_done=Callback::new(move |_| preview.set(None)) /> }.into_any(),
                        }}
                    </CardContent>
                </Card>
                <div class="flex flex-wrap gap-2">
                    <Button variant=ButtonVariant::Outline on_click=move |_| store.call("start_fresh", Kind { kind: "sample" }, "Sample data loaded")>
                        "Explore with sample data"
                    </Button>
                    <Button variant=ButtonVariant::Ghost on_click=move |_| store.call("start_fresh", Kind { kind: "empty" }, "")>
                        "Start empty"
                    </Button>
                </div>
            </div>
        </div>
    }
}

/// "This file holds …": shown before an import replaces anything.
#[component]
pub fn ImportConfirm(preview: ImportPreview, #[prop(into)] on_done: Callback<()>) -> impl IntoView {
    let store = use_store();
    let sym = if preview.currency.is_empty() { "$".to_string() } else { preview.currency.clone() };
    let confirm = move |_| {
        spawn_local(async move {
            match api::invoke::<Snapshot, _>("confirm_import", &NoArgs {}).await {
                Ok(s) => {
                    store.snap.set(Some(s));
                    store.notify("Data imported", false);
                }
                Err(e) => store.notify(format!("Could not import: {e}"), true),
            }
            on_done.run(());
        })
    };
    let cancel = move |_| {
        spawn_local(async move {
            let _ = api::invoke::<(), _>("cancel_import", &NoArgs {}).await;
            on_done.run(());
        })
    };
    view! {
        <div class="flex flex-col gap-2 rounded-md border bg-muted/40 p-3">
            <p class="text-sm">
                <span class="font-medium">{preview.file_name.clone()}</span>
                {format!(" is {}. It holds {}.", preview.kind, preview.summary)}
            </p>
            <p class="text-xs text-muted-foreground">
                {format!("Net worth {} · debt {}", money(preview.net_worth, &sym), money(preview.total_debt, &sym))}
            </p>
            <div class="flex gap-2">
                <Button size=ButtonSize::Sm on_click=confirm>"Import"</Button>
                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost on_click=cancel>"Cancel"</Button>
            </div>
        </div>
    }
}

#[component]
fn Shell() -> impl IntoView {
    let page = RwSignal::new(Page::Overview);
    view! {
        <div class="@container/app flex h-screen bg-background text-foreground">
            <nav class="flex w-14 @5xl/app:w-60 shrink-0 flex-col gap-1 overflow-y-auto border-r bg-sidenav p-2 @5xl/app:p-3">
                <div class="flex items-center gap-2 px-1 pb-4 pt-2 @5xl/app:px-3">
                    <img class="size-8 shrink-0 @5xl/app:hidden" src=LOGO_DATA_URI alt="PenguinFin" />
                    <div class="hidden @5xl/app:block">
                        <div class="text-lg font-bold tracking-tight">"PenguinFin"</div>
                        <div class="text-xs text-muted-foreground">"Offline personal finance"</div>
                    </div>
                </div>
                {NAV
                    .into_iter()
                    .map(|(p, label, icon)| {
                        view! {
                            <button
                                title=label
                                class=move || {
                                    if page.get() == p {
                                        "flex items-center justify-center @5xl/app:justify-start gap-3 rounded-md px-2 py-2 @5xl/app:px-3 text-left text-sm font-medium bg-sidenav-accent text-sidenav-accent-foreground"
                                    } else {
                                        "flex items-center justify-center @5xl/app:justify-start gap-3 rounded-md px-2 py-2 @5xl/app:px-3 text-left text-sm text-muted-foreground hover:bg-sidenav-accent hover:text-sidenav-accent-foreground cursor-pointer"
                                    }
                                }
                                on:click=move |_| page.set(p)
                            >
                                <Icon name=icon class="size-4 shrink-0" />
                                <span class="hidden @5xl/app:inline truncate">{label}</span>
                            </button>
                        }
                    })
                    .collect_view()}
                <div class="mt-auto flex flex-col gap-2 pt-4">
                    <UiSizeControl />
                    <div class="hidden px-3 text-[0.6875rem] text-muted-foreground @5xl/app:block">"Data stays on this computer."</div>
                </div>
            </nav>
            // Keyed by page so every page opens at the top.
            {move || {
                let p = page.get();
                view! {
                    <main class="@container min-w-0 flex-1 overflow-y-auto">
                        <div class="mx-auto flex max-w-6xl flex-col gap-6 p-4 @2xl:p-6 @3xl:p-8">
                            {match p {
                                Page::Overview => view! { <pages::overview::Overview on_nav=Callback::new(move |p| page.set(p)) /> }.into_any(),
                                Page::Settings => view! { <pages::settings::SettingsPage /> }.into_any(),
                                other => {
                                    let label = NAV.iter().find(|(q, _, _)| *q == other).map(|(_, l, _)| *l).unwrap_or("");
                                    view! { <pages::not_ported::NotPorted title=label /> }.into_any()
                                }
                            }}
                        </div>
                    </main>
                }
            }}
        </div>
    }
}

#[component]
fn UiSizeControl() -> impl IntoView {
    let store = use_store();
    let set = move |f: fn(UiScale) -> UiScale| {
        if let Some(snap) = store.snap.get_untracked() {
            let mut s = snap.data.settings.clone();
            s.ui_scale = f(s.ui_scale);
            store.call("save_settings", SettingsArg { settings: s }, "");
        }
    };
    let scale = move || store.snap.with(|s| s.as_ref().map(|s| s.data.settings.ui_scale).unwrap_or_default());
    let mod_key = move || if store.platform() == "macos" { "⌘" } else { "Ctrl " };
    let btn = "inline-flex size-7 items-center justify-center rounded-md border bg-background text-sm hover:bg-accent cursor-pointer";
    view! {
        <div class="flex flex-col items-center gap-1 @5xl/app:flex-row @5xl/app:justify-between @5xl/app:px-3">
            <span class="hidden text-xs text-muted-foreground @5xl/app:inline">"Text size"</span>
            <div class="flex flex-col items-center gap-1 @5xl/app:flex-row">
                <button class=btn title=move || format!("Bigger ({}+)", mod_key()) aria-label="Bigger text" on:click=move |_| set(UiScale::bigger)>"+"</button>
                <button
                    class="px-1 text-xs font-medium cursor-pointer"
                    title=move || format!("Fit the window ({}0)", mod_key())
                    on:click=move |_| set(|_| UiScale::Auto)
                >
                    {move || scale().label()}
                </button>
                <button class=btn title=move || format!("Smaller ({}−)", mod_key()) aria-label="Smaller text" on:click=move |_| set(UiScale::smaller)>"−"</button>
            </div>
        </div>
    }
}

/// The penguin mark for the collapsed sidebar (same as 1.x).
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
