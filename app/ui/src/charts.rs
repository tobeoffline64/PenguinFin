//! The two SVG charts, ported from 1.x with the same geometry, ticks and labels. Hover is done
//! with pointer events, so dragging a finger across a chart reads it on touch screens too.

use leptos::prelude::*;
use penguinfin_core::format::{money, money_short};
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq)]
pub struct Series {
    pub name: String,
    /// Any CSS color, e.g. "var(--chart-2)".
    pub color: String,
    pub values: Vec<f64>,
    pub bold: bool,
}

impl Series {
    pub fn new(name: impl Into<String>, color: &str, values: Vec<f64>, bold: bool) -> Self {
        Series { name: name.into(), color: color.into(), values, bold }
    }
}

const W: f64 = 900.0;
const H: f64 = 300.0;
const PL: f64 = 64.0;
const PR: f64 = 16.0;
const PT: f64 = 12.0;
const PB: f64 = 28.0;

struct Scale {
    lo: f64,
    hi: f64,
    ticks: Vec<f64>,
    n: usize,
}

impl Scale {
    fn new(mut lo: f64, mut hi: f64, n: usize) -> Self {
        if (hi - lo).abs() < 1e-9 {
            hi += 1.0;
            lo -= 1.0;
        }
        let raw = (hi - lo) / 4.0;
        let mag = 10f64.powf(raw.log10().floor());
        let step = [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|m| m * mag).find(|s| *s >= raw).unwrap_or(raw);
        lo = (lo / step).floor() * step;
        hi = (hi / step).ceil() * step;
        let mut ticks = Vec::new();
        let mut t = lo;
        while t <= hi + step * 0.01 {
            ticks.push(t);
            t += step;
        }
        Scale { lo, hi, ticks, n }
    }
    fn x(&self, i: usize) -> f64 {
        PL + (W - PL - PR) * i as f64 / (self.n - 1) as f64
    }
    fn y(&self, v: f64) -> f64 {
        PT + (H - PT - PB) * (1.0 - (v - self.lo) / (self.hi - self.lo))
    }
}

/// Grid, y ticks and x labels (thinned to about eight, always including the last).
fn axes(sc: &Scale, labels: &[String], currency: &str) -> impl IntoView + use<> {
    let n = sc.n;
    let every = n.div_ceil(8).max(1);
    let grid: Vec<_> = sc
        .ticks
        .iter()
        .map(|t| {
            let y = sc.y(*t);
            let stroke = if t.abs() < 1e-9 { "var(--muted-foreground)" } else { "var(--chart-grid)" };
            view! {
                <line x1=PL x2=W - PR y1=y y2=y stroke=stroke stroke-width="1" />
                <text x=PL - 8.0 y=y + 4.0 text-anchor="end" font-size="11" fill="var(--muted-foreground)">{money_short(*t, currency)}</text>
            }
        })
        .collect();
    let xl: Vec<_> = labels
        .iter()
        .enumerate()
        .filter(|(i, _)| (i % every == 0 && n - 1 - i >= every) || *i == n - 1)
        .map(|(i, l)| {
            let anchor = if i == n - 1 { "end" } else { "middle" };
            view! { <text x=sc.x(i) y=H - 8.0 text-anchor=anchor font-size="11" fill="var(--muted-foreground)">{l.clone()}</text> }
        })
        .collect();
    view! { {grid} {xl} }
}

/// The point under the pointer, from its x position on the drawn plot.
fn index_at(ev: &leptos::ev::PointerEvent, n: usize) -> Option<usize> {
    let el: web_sys::Element = ev.current_target()?.dyn_into().ok()?;
    let r = el.get_bounding_client_rect();
    if r.width() <= 0.0 {
        return None;
    }
    let x = (ev.client_x() as f64 - r.left()) / r.width() * W;
    let f = (x - PL) / (W - PL - PR) * (n - 1) as f64;
    Some(f.round().clamp(0.0, (n - 1) as f64) as usize)
}

fn path(sc: &Scale, values: &[f64]) -> String {
    values.iter().enumerate().map(|(i, v)| format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, sc.x(i), sc.y(*v))).collect()
}

#[component]
pub fn LineChart(series: Vec<Series>, labels: Vec<String>, #[prop(into)] currency: String) -> impl IntoView {
    let n = labels.len();
    if n < 2 || series.is_empty() {
        return view! { <div class="text-sm text-muted-foreground p-6">"Not enough data to chart yet."</div> }.into_any();
    }
    let hover = RwSignal::new(None::<usize>);
    let lo = series.iter().flat_map(|s| s.values.iter().copied()).fold(f64::INFINITY, f64::min).min(0.0);
    let hi = series.iter().flat_map(|s| s.values.iter().copied()).fold(f64::NEG_INFINITY, f64::max).max(0.0);
    let sc = Scale::new(lo, hi, n);
    let paths: Vec<_> = series
        .iter()
        .map(|s| view! { <path d=path(&sc, &s.values) fill="none" stroke=s.color.clone() stroke-width=if s.bold { "3" } else { "1.75" } stroke-linejoin="round" stroke-linecap="round" /> })
        .collect();
    let ax = axes(&sc, &labels, &currency);
    let sc_hover = Scale::new(lo, hi, n);
    let series_h = series.clone();
    let labels_h = labels.clone();
    let cur = currency.clone();
    let legend = move || {
        let i = hover.get();
        view! {
            {series_h.iter().map(|s| view! {
                <div class="flex items-center gap-1.5">
                    <span class="inline-block h-2.5 w-2.5 rounded-full" style=format!("background: {}", s.color)></span>
                    <span class="text-muted-foreground">{s.name.clone()}</span>
                    {i.map(|i| view! { <span class="font-medium tabular-nums">{money(s.values.get(i).copied().unwrap_or(0.0), &cur)}</span> })}
                </div>
            }).collect_view()}
            {i.map(|i| view! { <span class="ml-auto font-medium">{labels_h[i].clone()}</span> })}
        }
    };
    let marker = move || {
        hover.get().map(|i| {
            let x = sc_hover.x(i);
            view! {
                <line x1=x x2=x y1=PT y2=H - PB stroke="var(--muted-foreground)" stroke-dasharray="4 4" />
                {series.iter().map(|s| view! {
                    <circle cx=x cy=sc_hover.y(s.values.get(i).copied().unwrap_or(0.0)) r="4" fill=s.color.clone() stroke="var(--background)" stroke-width="2" />
                }).collect_view()}
            }
        })
    };
    view! {
        <div class="flex flex-col gap-2">
            <div class="flex flex-wrap gap-4 text-xs">{legend}</div>
            <svg class="w-full h-auto select-none touch-pan-y" viewBox=format!("0 0 {W} {H}")
                on:pointermove=move |ev| hover.set(index_at(&ev, n))
                on:pointerdown=move |ev| hover.set(index_at(&ev, n))
                on:pointerleave=move |_| hover.set(None)>
                {ax}
                {paths}
                {marker}
            </svg>
        </div>
    }
    .into_any()
}

/// Stacked areas (non-negative layers, bottom-up), a total line on top, and an optional dashed
/// reference line (e.g. a minimum balance).
#[component]
pub fn StackedChart(
    layers: Vec<Series>,
    total: Series,
    #[prop(optional)] reference: Option<(String, f64)>,
    labels: Vec<String>,
    #[prop(into)] currency: String,
) -> impl IntoView {
    let n = labels.len();
    if n < 2 {
        return view! { <div class="text-sm text-muted-foreground p-6">"Not enough data to chart yet."</div> }.into_any();
    }
    let hover = RwSignal::new(None::<usize>);
    let mut tops: Vec<Vec<f64>> = Vec::new();
    let mut acc = vec![0.0; n];
    for l in &layers {
        for (i, a) in acc.iter_mut().enumerate() {
            *a += l.values.get(i).copied().unwrap_or(0.0).max(0.0);
        }
        tops.push(acc.clone());
    }
    let ref_v = reference.as_ref().map(|r| r.1).unwrap_or(0.0);
    let lo = total.values.iter().copied().fold(0.0_f64, f64::min);
    let hi = acc.iter().chain(total.values.iter()).copied().fold(ref_v, f64::max);
    let sc = Scale::new(lo, if (hi - lo).abs() < 1e-9 { hi + 1.0 } else { hi }, n);
    let areas: Vec<_> = layers
        .iter()
        .enumerate()
        .map(|(li, l)| {
            let top = &tops[li];
            let mut d = String::new();
            for (i, v) in top.iter().enumerate() {
                d += &format!("{}{:.1},{:.1}", if i == 0 { "M" } else { "L" }, sc.x(i), sc.y(*v));
            }
            for i in (0..n).rev() {
                let bottom = if li == 0 { 0.0 } else { tops[li - 1][i] };
                d += &format!("L{:.1},{:.1}", sc.x(i), sc.y(bottom));
            }
            d.push('Z');
            view! { <path d=d fill=l.color.clone() fill-opacity="0.55" stroke=l.color.clone() stroke-width="1" /> }
        })
        .collect();
    let total_path = path(&sc, &total.values);
    let ref_line = reference.as_ref().map(|(_, v)| {
        let y = sc.y(*v);
        view! { <line x1=PL x2=W - PR y1=y y2=y stroke="var(--destructive)" stroke-width="1.5" stroke-dasharray="6 4" /> }
    });
    let ax = axes(&sc, &labels, &currency);
    let x_hover = move |i: usize| PL + (W - PL - PR) * i as f64 / (n - 1) as f64;
    let cur = currency.clone();
    let total_color = total.color.clone();
    let legend = move || {
        let i = hover.get();
        let val = |s: &Series| i.map(|i| view! { <span class="font-medium tabular-nums">{money(s.values.get(i).copied().unwrap_or(0.0), &cur)}</span> });
        view! {
            {layers.iter().rev().map(|s| view! {
                <div class="flex items-center gap-1.5">
                    <span class="inline-block h-2.5 w-2.5 rounded-sm" style=format!("background: {}", s.color)></span>
                    <span class="text-muted-foreground">{s.name.clone()}</span>
                    {val(s)}
                </div>
            }).collect_view()}
            <div class="flex items-center gap-1.5">
                <span class="inline-block h-0.5 w-3" style=format!("background: {}", total.color)></span>
                <span class="text-muted-foreground">{total.name.clone()}</span>
                {val(&total)}
            </div>
            {reference.clone().map(|(name, v)| view! {
                <div class="flex items-center gap-1.5">
                    <span class="inline-block w-3 border-t-2 border-dashed border-destructive"></span>
                    <span class="text-muted-foreground">{format!("{name} {}", money(v, &cur))}</span>
                </div>
            })}
            {i.map(|i| view! { <span class="ml-auto font-medium">{labels[i].clone()}</span> })}
        }
    };
    view! {
        <div class="flex flex-col gap-2">
            <div class="flex flex-wrap gap-x-4 gap-y-1 text-xs">{legend}</div>
            <svg class="w-full h-auto select-none touch-pan-y" viewBox=format!("0 0 {W} {H}")
                on:pointermove=move |ev| hover.set(index_at(&ev, n))
                on:pointerdown=move |ev| hover.set(index_at(&ev, n))
                on:pointerleave=move |_| hover.set(None)>
                {ax}
                {areas}
                <path d=total_path fill="none" stroke=total_color stroke-width="2.5" stroke-linejoin="round" />
                {ref_line}
                {move || hover.get().map(|i| view! {
                    <line x1=x_hover(i) x2=x_hover(i) y1=PT y2=H - PB stroke="var(--muted-foreground)" stroke-dasharray="4 4" />
                })}
            </svg>
        </div>
    }
    .into_any()
}
