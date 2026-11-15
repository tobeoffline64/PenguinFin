//! Small app-specific widgets built on top of the Rust/UI components.

use crate::ui::card::*;
use crate::ui::label::Label;
use dioxus::prelude::*;

// ---------- formatting ----------

fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "$1,234" (rounded to whole units).
pub fn money(v: f64, sym: &str) -> String {
    let neg = v < -0.5;
    let n = v.abs().round() as u64;
    format!("{}{sym}{}", if neg { "-" } else { "" }, group_thousands(n))
}

/// "$1,234.56"
pub fn money_cents(v: f64, sym: &str) -> String {
    let neg = v < -0.005;
    let cents = (v.abs() * 100.0).round() as u64;
    format!("{}{sym}{}.{:02}", if neg { "-" } else { "" }, group_thousands(cents / 100), cents % 100)
}

/// Short axis labels: "$1.2M", "$350k".
pub fn money_short(v: f64, sym: &str) -> String {
    let a = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if a >= 1e6 {
        format!("{sign}{sym}{:.1}M", a / 1e6)
    } else if a >= 1e3 {
        format!("{sign}{sym}{:.0}k", a / 1e3)
    } else {
        format!("{sign}{sym}{a:.0}")
    }
}

pub fn parse_num(s: &str) -> Option<f64> {
    let cleaned: String = s.chars().filter(|c| !matches!(c, ',' | '$' | ' ' | '%')).collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

pub fn num_str(v: f64) -> String {
    if v == v.trunc() { format!("{v:.0}") } else { format!("{v}") }
}

// ---------- icons (Lucide, ISC license) ----------

#[derive(Clone, Copy, PartialEq)]
pub enum IconName {
    Dashboard,
    Income,
    Bill,
    Expense,
    Accounts,
    Chart,
    Settings,
    Plus,
    Pencil,
    Trash,
    Alert,
}

fn icon_paths(i: IconName) -> &'static [&'static str] {
    match i {
        IconName::Dashboard => &["M3 3h7v9H3z", "M14 3h7v5h-7z", "M14 12h7v9h-7z", "M3 16h7v5H3z"],
        IconName::Income => &["M12 2v20", "M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"],
        IconName::Bill => &["M4 2v20l2-1 2 1 2-1 2 1 2-1 2 1 2-1 2 1V2l-2 1-2-1-2 1-2-1-2 1-2-1-2 1Z", "M16 8h-6a2 2 0 1 0 0 4h4a2 2 0 1 1 0 4H8", "M12 17.5v-11"],
        IconName::Expense => &["M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z", "M3 6h18", "M16 10a4 4 0 0 1-8 0"],
        IconName::Accounts => &["M3 22h18", "M6 18v-7", "M10 18v-7", "M14 18v-7", "M18 18v-7", "M12 2 20 7H4z"],
        IconName::Chart => &["M3 3v18h18", "m19 9-5 5-4-4-3 3"],
        IconName::Settings => &["M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z", "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z"],
        IconName::Plus => &["M5 12h14", "M12 5v14"],
        IconName::Pencil => &["M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"],
        IconName::Trash => &["M3 6h18", "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6", "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"],
        IconName::Alert => &["m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z", "M12 9v4", "M12 17h.01"],
    }
}

#[component]
pub fn Icon(name: IconName, #[props(into, default = "size-4".to_string())] class: String) -> Element {
    rsx! {
        svg {
            class: "{class}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            for d in icon_paths(name).iter() {
                path { d: "{d}" }
            }
        }
    }
}

// ---------- form helpers ----------

#[component]
pub fn Field(#[props(into)] label: String, #[props(into, optional)] hint: Option<String>, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-col gap-1.5",
            Label { class: "text-xs font-medium text-muted-foreground", "{label}" }
            {children}
            if let Some(h) = hint {
                p { class: "text-xs text-muted-foreground", "{h}" }
            }
        }
    }
}

pub const SELECT_CLASS: &str = "border-input dark:bg-input/30 h-9 w-full rounded-md border bg-transparent px-2 text-sm shadow-xs outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/50";

/// A native `<select>` styled like the Rust/UI Input.
#[component]
pub fn NativeSelect(value: String, options: Vec<(String, String)>, onchange: EventHandler<String>) -> Element {
    rsx! {
        select {
            class: SELECT_CLASS,
            onchange: move |e| onchange.call(e.value()),
            for (k, label) in options {
                option { value: "{k}", selected: k == value, "{label}" }
            }
        }
    }
}

// ---------- stat card ----------

#[component]
pub fn Stat(
    #[props(into)] label: String,
    #[props(into)] value: String,
    #[props(into, optional)] sub: Option<String>,
    #[props(into, default)] tone: String,
    /// Show a skeleton instead of the value/sub text (value still being calculated).
    #[props(default)] loading: bool,
) -> Element {
    let color = match tone.as_str() {
        "good" => "text-success",
        "bad" => "text-destructive",
        _ => "",
    };
    rsx! {
        Card { size: CardSize::Sm, class: "gap-1",
            CardContent { class: "flex flex-col gap-1",
                span { class: "text-xs font-medium uppercase tracking-wide text-muted-foreground", "{label}" }
                if loading {
                    LoadingBlock { class: "h-8 w-32" }
                    LoadingBlock { class: "h-3 w-40" }
                } else {
                    span { class: "text-2xl font-semibold tabular-nums {color}", "{value}" }
                    if let Some(s) = sub {
                        span { class: "text-xs text-muted-foreground", "{s}" }
                    }
                }
            }
        }
    }
}

// ---------- line chart ----------

#[derive(Clone, PartialEq)]
pub struct Series {
    pub name: String,
    /// Any CSS color, e.g. "var(--chart-2)".
    pub color: String,
    pub values: Vec<f64>,
    pub bold: bool,
}

/// Multi-series SVG line chart with a hover readout. `labels[i]` names point `i`.
#[component]
pub fn LineChart(series: Vec<Series>, labels: Vec<String>, #[props(into)] currency: String) -> Element {
    let mut hover = use_signal(|| None::<usize>);
    let n = labels.len();
    if n < 2 || series.is_empty() {
        return rsx! { div { class: "text-sm text-muted-foreground p-6", "Not enough data to chart yet." } };
    }
    let (w, h) = (900.0_f64, 300.0_f64);
    let (pl, pr, pt, pb) = (64.0, 16.0, 12.0, 28.0);
    let pw = w - pl - pr;
    let ph = h - pt - pb;

    let mut lo = series.iter().flat_map(|s| s.values.iter().copied()).fold(f64::INFINITY, f64::min).min(0.0);
    let mut hi = series.iter().flat_map(|s| s.values.iter().copied()).fold(f64::NEG_INFINITY, f64::max).max(0.0);
    if (hi - lo).abs() < 1e-9 {
        hi += 1.0;
        lo -= 1.0;
    }
    // Nice tick step.
    let raw = (hi - lo) / 4.0;
    let mag = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(raw);
    lo = (lo / step).floor() * step;
    hi = (hi / step).ceil() * step;

    let x = move |i: usize| pl + pw * i as f64 / (n - 1) as f64;
    let y = move |v: f64| pt + ph * (1.0 - (v - lo) / (hi - lo));

    let mut ticks = Vec::new();
    let mut t = lo;
    while t <= hi + step * 0.01 {
        ticks.push(t);
        t += step;
    }
    let label_every = n.div_ceil(8).max(1);
    let col_w = pw / (n - 1) as f64;

    let paths: Vec<(String, String, bool)> = series
        .iter()
        .map(|s| {
            let d = s
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, x(i), y(*v)))
                .collect::<String>();
            (d, s.color.clone(), s.bold)
        })
        .collect();

    let hovered = hover();
    rsx! {
        div { class: "flex flex-col gap-2",
            div { class: "flex flex-wrap gap-4 text-xs",
                for s in series.iter() {
                    div { class: "flex items-center gap-1.5",
                        span { class: "inline-block h-2.5 w-2.5 rounded-full", style: "background: {s.color}" }
                        span { class: "text-muted-foreground", "{s.name}" }
                        if let Some(i) = hovered {
                            span { class: "font-medium tabular-nums",
                                {money(s.values.get(i).copied().unwrap_or(0.0), &currency)}
                            }
                        }
                    }
                }
                if let Some(i) = hovered {
                    span { class: "ml-auto font-medium", "{labels[i]}" }
                }
            }
            svg {
                class: "w-full h-auto select-none",
                view_box: "0 0 {w} {h}",
                onmouseleave: move |_| hover.set(None),
                for t in ticks.iter() {
                    line {
                        x1: "{pl}", x2: "{w - pr}", y1: "{y(*t)}", y2: "{y(*t)}",
                        stroke: if t.abs() < 1e-9 { "var(--muted-foreground)" } else { "var(--chart-grid)" },
                        stroke_width: "1",
                    }
                    text {
                        x: "{pl - 8.0}", y: "{y(*t) + 4.0}", text_anchor: "end",
                        font_size: "11", fill: "var(--muted-foreground)",
                        {money_short(*t, &currency)}
                    }
                }
                for (i, l) in labels.iter().enumerate() {
                    if i % label_every == 0 || i == n - 1 {
                        text {
                            x: "{x(i)}", y: "{h - 8.0}", text_anchor: "middle",
                            font_size: "11", fill: "var(--muted-foreground)",
                            "{l}"
                        }
                    }
                }
                for (d, color, bold) in paths.iter() {
                    path {
                        d: "{d}", fill: "none", stroke: "{color}",
                        stroke_width: if *bold { "3" } else { "1.75" },
                        stroke_linejoin: "round", stroke_linecap: "round",
                    }
                }
                if let Some(i) = hovered {
                    line {
                        x1: "{x(i)}", x2: "{x(i)}", y1: "{pt}", y2: "{pt + ph}",
                        stroke: "var(--muted-foreground)", stroke_dasharray: "4 4",
                    }
                    for s in series.iter() {
                        circle {
                            cx: "{x(i)}", cy: "{y(s.values.get(i).copied().unwrap_or(0.0))}", r: "4",
                            fill: "{s.color}", stroke: "var(--background)", stroke_width: "2",
                        }
                    }
                }
                // Invisible hover columns.
                for i in 0..n {
                    rect {
                        x: "{x(i) - col_w / 2.0}", y: "{pt}", width: "{col_w}", height: "{ph}",
                        fill: "transparent",
                        onmouseenter: move |_| hover.set(Some(i)),
                    }
                }
            }
        }
    }
}

#[component]
pub fn EmptyState(#[props(into)] title: String, #[props(into)] text: String, children: Element) -> Element {
    rsx! {
        div { class: "flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed p-10 text-center",
            h3 { class: "text-base font-semibold", "{title}" }
            p { class: "max-w-md text-sm text-muted-foreground", "{text}" }
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatting() {
        assert_eq!(money(1234567.4, "$"), "$1,234,567");
        assert_eq!(money(-950.0, "$"), "-$950");
        assert_eq!(money(0.2, "$"), "$0");
        assert_eq!(money_cents(1234.5, "€"), "€1,234.50");
        assert_eq!(money_short(1_250_000.0, "$"), "$1.2M");
        assert_eq!(money_short(-35_000.0, "$"), "-$35k");
        assert_eq!(parse_num("$1,200.50"), Some(1200.5));
        assert_eq!(parse_num(""), None);
        assert_eq!(parse_num("abc"), None);
    }
}

// ---------- loading skeletons ----------

/// A pulsing placeholder that only fades in if loading takes longer than ~150 ms,
/// so fast loads don't flash.
#[component]
pub fn LoadingBlock(#[props(into, default)] class: String) -> Element {
    rsx! {
        div { class: "pf-delayed", aria_busy: "true",
            crate::ui::skeleton::Skeleton { class: "bg-secondary {class}" }
        }
    }
}

/// Placeholder with the same footprint as `LineChart` (legend row + 3:1 plot).
#[component]
pub fn ChartSkeleton() -> Element {
    rsx! {
        div { class: "pf-delayed flex flex-col gap-2", aria_busy: "true", aria_label: "Calculating projection",
            div { class: "flex gap-4",
                for w in ["w-20", "w-20", "w-24", "w-14"] {
                    crate::ui::skeleton::Skeleton { class: "h-4 bg-secondary {w}" }
                }
            }
            crate::ui::skeleton::Skeleton { class: "aspect-[3/1] w-full bg-secondary" }
        }
    }
}

/// Placeholder rows for a table that is still being calculated.
#[component]
pub fn TableSkeleton(rows: usize) -> Element {
    rsx! {
        div { class: "pf-delayed flex flex-col gap-2", aria_busy: "true",
            for i in 0..rows {
                crate::ui::skeleton::Skeleton { key: "{i}", class: "h-8 w-full bg-secondary" }
            }
        }
    }
}
