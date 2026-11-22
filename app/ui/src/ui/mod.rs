//! Rust/UI components for Leptos, with the same class strings as the 1.x (Dioxus) copies, so
//! both apps look the same and follow the design system. Some variants are only used by pages
//! still being ported.
#![allow(dead_code)]

pub mod icon;

use leptos::ev::MouseEvent;
use leptos::prelude::*;
use tw_merge::tw_merge;

// ---------- Button ----------

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Destructive,
    Outline,
    Secondary,
    Ghost,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    #[default]
    Default,
    Sm,
    IconSm,
}

impl ButtonVariant {
    fn class(self) -> &'static str {
        match self {
            Self::Default => "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90",
            Self::Destructive => "bg-destructive text-white shadow-xs hover:bg-destructive/90 focus-visible:ring-destructive/20 dark:focus-visible:ring-destructive/40 dark:bg-destructive/60",
            Self::Outline => "border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground dark:bg-input/30 dark:border-input dark:hover:bg-input/5",
            Self::Secondary => "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80",
            Self::Ghost => "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
        }
    }
}

impl ButtonSize {
    fn class(self) -> &'static str {
        match self {
            Self::Default => "h-9 px-4 py-2 has-[>svg]:px-3",
            Self::Sm => "h-8 rounded-md gap-1.5 px-3 has-[>svg]:px-2.5",
            Self::IconSm => "size-8 rounded-md",
        }
    }
}

const BUTTON_BASE: &str = "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4 shrink-0 [&_svg]:shrink-0 outline-none focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] w-fit hover:cursor-pointer active:scale-[0.98] select-none";

#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(into, optional)] class: String,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(into, optional)] title: Option<String>,
    #[prop(into, optional)] on_click: Option<Callback<MouseEvent>>,
    children: Children,
) -> impl IntoView {
    let class = tw_merge!(BUTTON_BASE, variant.class(), size.class(), class);
    view! {
        <button
            type="button"
            class=class
            disabled=move || disabled.get()
            title=title.clone()
            aria-label=title
            on:click=move |ev| {
                if let Some(cb) = on_click {
                    cb.run(ev)
                }
            }
        >
            {children()}
        </button>
    }
}

// ---------- Card ----------

#[component]
pub fn Card(#[prop(into, optional)] class: String, #[prop(optional)] small: bool, children: Children) -> impl IntoView {
    let size = if small { "py-4 gap-3" } else { "py-6 gap-4" };
    let class = tw_merge!("bg-card text-card-foreground flex flex-col rounded-xl border shadow-sm", size, class);
    view! { <div data-name="Card" data-size=if small { "sm" } else { "default" } class=class>{children()}</div> }
}

#[component]
pub fn CardHeader(children: Children) -> impl IntoView {
    view! { <div class="flex flex-col items-start gap-1.5 px-6 [[data-size=sm]_&]:px-4">{children()}</div> }
}

#[component]
pub fn CardTitle(children: Children) -> impl IntoView {
    view! { <h2 class="leading-none font-semibold">{children()}</h2> }
}

#[component]
pub fn CardDescription(children: Children) -> impl IntoView {
    view! { <p class="text-muted-foreground text-sm">{children()}</p> }
}

#[component]
pub fn CardContent(#[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let class = tw_merge!("px-6 [[data-size=sm]_&]:px-4", class);
    view! { <div class=class>{children()}</div> }
}

// ---------- Input ----------

const INPUT: &str = "text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground dark:bg-input/30 border-input flex h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-2 aria-invalid:border-destructive read-only:bg-muted";

/// A controlled text input. In 2.0 typing stays inside the webview, so the 1.x workaround
/// (uncontrolled inputs remounted by key) is not needed.
#[component]
pub fn Input(
    value: RwSignal<String>,
    #[prop(into, optional)] placeholder: String,
    #[prop(into, default = "text".to_string())] kind: String,
    #[prop(optional)] readonly: bool,
    #[prop(into, optional)] id: Option<String>,
) -> impl IntoView {
    view! {
        <input
            data-name="Input"
            id=id
            type=kind
            class=INPUT
            placeholder=placeholder
            readonly=readonly
            prop:value=move || value.get()
            on:input=move |ev| value.set(event_target_value(&ev))
        />
    }
}

// ---------- Field ----------

#[component]
pub fn Field(#[prop(into)] label: String, #[prop(into, optional)] hint: Option<String>, children: Children) -> impl IntoView {
    view! {
        <div class="flex flex-col gap-1.5">
            <label class="flex items-center gap-2 leading-none select-none text-xs font-medium text-muted-foreground">{label}</label>
            {children()}
            {hint.map(|h| view! { <p class="text-xs text-muted-foreground">{h}</p> })}
        </div>
    }
}

// ---------- NativeSelect ----------

const SELECT: &str = "appearance-none border-input bg-background text-foreground dark:bg-input/30 h-9 w-full rounded-md border pl-3 pr-9 text-sm shadow-xs outline-none cursor-pointer focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50";

#[component]
pub fn NativeSelect(
    #[prop(into)] value: Signal<String>,
    options: Vec<(String, String)>,
    #[prop(into)] on_change: Callback<String>,
    #[prop(into, optional)] label: String,
) -> impl IntoView {
    view! {
        <div class="relative w-full">
            <select class=SELECT aria-label=label prop:value=move || value.get() on:change=move |ev| on_change.run(event_target_value(&ev))>
                {options
                    .into_iter()
                    .map(|(k, l)| {
                        let k2 = k.clone();
                        view! { <option value=k selected=move || value.get() == k2>{l}</option> }
                    })
                    .collect_view()}
            </select>
            <icon::Icon name=icon::IconName::ChevronDown class="pointer-events-none absolute right-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
        </div>
    }
}

// ---------- Switch ----------

#[component]
pub fn Switch(#[prop(into)] checked: Signal<bool>, #[prop(into)] label: String, #[prop(into)] on_change: Callback<bool>) -> impl IntoView {
    let state = move || if checked.get() { "checked" } else { "unchecked" };
    view! {
        <button
            data-name="Switch"
            type="button"
            role="switch"
            aria-label=label
            aria-checked=move || checked.get().to_string()
            data-state=state
            class="inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:cursor-not-allowed disabled:opacity-50 data-[state=checked]:bg-primary data-[state=unchecked]:bg-input"
            on:click=move |_| on_change.run(!checked.get_untracked())
        >
            <span
                data-state=state
                class="block rounded-full ring-0 shadow-lg transition-transform pointer-events-none size-5 bg-background data-[state=checked]:translate-x-5 data-[state=unchecked]:translate-x-0"
            ></span>
        </button>
    }
}

// ---------- Separator, Badge ----------

#[component]
pub fn Separator() -> impl IntoView {
    view! { <div class="shrink-0 bg-border w-full h-[1px]" role="separator"></div> }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    #[default]
    Secondary,
    Success,
    Warning,
    Info,
}

#[component]
pub fn Badge(#[prop(optional)] variant: BadgeVariant, children: Children) -> impl IntoView {
    let v = match variant {
        BadgeVariant::Secondary => "border-transparent bg-secondary text-secondary-foreground",
        BadgeVariant::Success => "border-transparent bg-success-light text-success-dark",
        BadgeVariant::Warning => "border-transparent bg-warning-light text-warning-dark",
        BadgeVariant::Info => "border-transparent bg-info-light text-info-dark",
    };
    let class = tw_merge!("inline-flex items-center font-semibold rounded-md border w-fit px-1.5 py-0.5 text-[0.625rem]", v);
    view! { <span class=class>{children()}</span> }
}

// ---------- Stat, EmptyState, PageHeader, LoadingBlock ----------

#[component]
pub fn Stat(
    #[prop(into)] label: String,
    #[prop(into)] value: String,
    #[prop(into, optional)] sub: Option<String>,
    #[prop(into, optional)] tone: String,
) -> impl IntoView {
    let color = match tone.as_str() {
        "good" => "text-success",
        "bad" => "text-destructive",
        _ => "",
    };
    view! {
        <Card small=true class="gap-1">
            <CardContent class="flex flex-col gap-1">
                <span class="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</span>
                <span class=format!("text-2xl font-semibold tabular-nums {color}")>{value}</span>
                {sub.map(|s| view! { <span class="text-xs text-muted-foreground">{s}</span> })}
            </CardContent>
        </Card>
    }
}

#[component]
pub fn EmptyState(#[prop(into)] title: String, #[prop(into)] text: String, #[prop(optional)] children: Option<Children>) -> impl IntoView {
    view! {
        <div class="flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed p-10 text-center">
            <h3 class="text-base font-semibold">{title}</h3>
            <p class="max-w-md text-sm text-muted-foreground">{text}</p>
            {children.map(|c| c())}
        </div>
    }
}

#[component]
pub fn PageHeader(#[prop(into)] title: String, #[prop(into, optional)] subtitle: Option<String>) -> impl IntoView {
    view! {
        <div class="flex items-end justify-between gap-4">
            <div>
                <h1 class="text-2xl font-semibold tracking-tight">{title}</h1>
                {subtitle.map(|s| view! { <p class="text-sm text-muted-foreground">{s}</p> })}
            </div>
        </div>
    }
}

#[component]
pub fn LoadingBlock(#[prop(into, optional)] class: String) -> impl IntoView {
    view! {
        <div class="pf-delayed" aria-busy="true">
            <div class=format!("animate-pulse rounded-md bg-secondary {class}")></div>
        </div>
    }
}
