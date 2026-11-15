use crate::app::{Page, today, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::progress::Progress;
use crate::ui::switch::Switch;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use chrono::NaiveDate;
use dioxus::prelude::*;
use penguinfin::goals::{GoalPlan, GoalsPlan, TargetStatus, plan_goals};
use penguinfin::model::{AccountGroup, FlowKind, Goal};

/// How far ahead to look for goals: at least 10 years, or the projection length if longer.
fn horizon(projection_years: u32) -> u32 {
    projection_years.max(10)
}

fn month_label(d: NaiveDate) -> String {
    d.format("%b %Y").to_string()
}

fn when_text(plan: &GoalPlan, today: NaiveDate) -> String {
    match (plan.affordable_on, plan.months_away) {
        (Some(d), _) if d == today => "Affordable now".into(),
        (Some(d), Some(0)) => format!("Affordable by end of {}", d.format("%B")),
        (Some(d), Some(1)) => format!("{} · in 1 month", month_label(d)),
        (Some(d), Some(m)) if m < 24 => format!("{} · in {m} months", month_label(d)),
        (Some(d), Some(m)) => format!("{} · in {:.1} years", month_label(d), m as f64 / 12.0),
        _ => "Not within the projection".into(),
    }
}

#[component]
pub fn GoalsPage(on_nav: EventHandler<Page>) -> Element {
    let store = use_store();
    let data = store.data.read().clone();
    let sym = data.settings.currency_symbol.clone();
    let today = today();
    let mut form = use_signal(GoalForm::new);
    let mut generation = use_signal(|| 0u32);
    let editing_id = use_memo(move || form.read().id);

    // Plan on a worker thread (it runs a full projection); skeletons show meanwhile.
    let plan = use_resource(move || {
        let data = store.data.read().clone();
        async move {
            let years = horizon(data.settings.projection_years);
            crate::app::off_thread(move || plan_goals(&data, today, years)).await
        }
    });
    let plan_value = plan.read().clone();
    let loading = plan_value.is_none();
    let plan_value = plan_value.unwrap_or_default();

    let income = data.monthly_total(|k| k == FlowKind::Income, today);
    let outflow = data.monthly_total(|k| k != FlowKind::Income, today);
    let leftover = income - outflow - data.monthly_cash_contributions();
    let baseline = data.cash_baseline();
    let bucketed = data.cash_buckets_total();
    let reserved = baseline + bucketed;
    let cash_now = data.group_total(AccountGroup::Cash);
    let cash_buckets: Vec<(String, penguinfin::model::Bucket)> = data
        .buckets
        .iter()
        .filter_map(|b| {
            let a = data.accounts.iter().find(|a| a.id == b.account_id && a.kind.group() == AccountGroup::Cash)?;
            Some((a.name.clone(), b.clone()))
        })
        .collect();

    let mut open: Vec<Goal> = data.goals.iter().filter(|g| g.purchased_on.is_none()).cloned().collect();
    open.sort_by_key(|g| (g.priority, g.id));
    let mut bought: Vec<Goal> = data.goals.iter().filter(|g| g.purchased_on.is_some()).cloned().collect();
    bought.sort_by_key(|g| std::cmp::Reverse(g.purchased_on));
    let open_count = open.len();
    let total_open: f64 = open.iter().map(|g| g.cost).sum();
    let cash_accounts: Vec<_> = data.accounts.iter().filter(|a| a.kind.group() == AccountGroup::Cash).cloned().collect();

    rsx! {
        PageHeader { title: "Savings goals",
            subtitle: "Things you want to buy, and when you can afford each one while keeping your minimum balances." }

        div { class: "grid grid-cols-2 gap-4 @3xl:grid-cols-4",
            Stat { label: "Checking + savings", value: money(cash_now, &sym), sub: "Where goals are paid from" }
            Stat { label: "Reserved", value: money(reserved, &sym),
                sub: format!("Minimums {} · Buckets {}", money(baseline, &sym), money(bucketed, &sym)) }
            Stat { label: "Spendable now", value: money(cash_now - reserved, &sym),
                tone: if cash_now - reserved < 0.0 { "bad" } else { "good" },
                sub: format!("Planned goals total {}", money(total_open, &sym)) }
            Stat { label: "Left over / month", value: money(leftover, &sym),
                tone: if leftover < 0.0 { "bad" } else { "" }, sub: "After bills, expenses and savings transfers" }
        }

        if reserved <= 0.0 && !cash_accounts.is_empty() {
            div { class: "flex flex-wrap items-center justify-between gap-3 rounded-xl border border-dashed p-4 text-sm",
                span { class: "text-muted-foreground",
                    "Nothing reserved yet, so every dollar in checking and savings counts as spendable. Set minimum balances or buckets (like Rainy day money) in Accounts to keep money for bills, medical, food and emergencies." }
                Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, onclick: move |_| on_nav.call(Page::Accounts), "Set minimums in Accounts" }
            }
        } else if reserved > 0.0 {
            Card { size: CardSize::Sm,
                CardHeader {
                    CardTitle { "Reserved money" }
                    CardDescription { "Minimum balances and buckets are kept in place before any goal is bought. Change these in Accounts." }
                }
                CardContent { class: "flex flex-wrap gap-2",
                    for a in cash_accounts.iter().filter(|a| a.minimum_balance > 0.0) {
                        div { key: "{a.id}", class: "flex items-center gap-2 rounded-lg border px-3 py-2 text-sm",
                            span { class: "font-medium", "{a.name}" }
                            if !a.purpose.is_empty() {
                                Badge { variant: BadgeVariant::Info, size: BadgeSize::Sm, "{a.purpose}" }
                            }
                            span { class: "tabular-nums text-muted-foreground", "min {money(a.minimum_balance, &sym)}" }
                        }
                    }
                    for (acct, b) in cash_buckets.iter() {
                        div { key: "b{b.id}", class: "flex items-center gap-2 rounded-lg border px-3 py-2 text-sm",
                            span { class: "font-medium", "{acct} › {b.name}" }
                            span { class: "tabular-nums text-muted-foreground", "bucket {money(b.balance, &sym)}" }
                        }
                    }
                }
            }
        }

        div { class: "grid gap-6 @5xl:grid-cols-3",
            div { class: "flex flex-col gap-6 @5xl:col-span-2",
                Card {
                    CardHeader {
                        CardTitle { "Planned ({open_count})" }
                        CardDescription { "Bought in this order. Use the arrows to change priority." }
                    }
                    CardContent { class: "flex flex-col gap-3",
                        if open.is_empty() {
                            EmptyState { title: "No goals yet", text: "Add something you'd like to buy and PenguinFin will tell you when you can afford it." }
                        }
                        for (i, g) in open.iter().cloned().enumerate() {
                            GoalRow {
                                key: "{g.id}",
                                goal: g.clone(),
                                plan: plan_value.plan_for(g.id).cloned(),
                                loading,
                                first: i == 0,
                                last: i + 1 == open_count,
                                selected: editing_id() == g.id,
                                sym: sym.clone(),
                                on_edit: move |g: Goal| { form.set(GoalForm::from_goal(&g)); generation += 1; },
                            }
                        }
                    }
                }

                if !open.is_empty() {
                    SpendableChart { plan: plan_value.clone(), loading, sym: sym.clone() }
                }

                if !bought.is_empty() {
                    Card { size: CardSize::Sm,
                        CardHeader { CardTitle { "Bought" } }
                        CardContent { class: "flex flex-col gap-2",
                            for g in bought.iter().cloned() {
                                BoughtRow { key: "{g.id}", goal: g, sym: sym.clone() }
                            }
                        }
                    }
                }
            }
            GoalFormCard { form, generation, sym: sym.clone() }
        }
    }
}

#[component]
fn GoalRow(
    goal: Goal,
    plan: Option<GoalPlan>,
    loading: bool,
    first: bool,
    last: bool,
    selected: bool,
    sym: String,
    on_edit: EventHandler<Goal>,
) -> Element {
    let mut store = use_store();
    let mut confirm = use_signal(|| false);
    let today = today();
    let id = goal.id;
    let g2 = goal.clone();
    let g3 = goal.clone();
    let border = if selected { "rounded-lg border border-ring p-4" } else { "rounded-lg border p-4" };
    let affordable_now = plan.as_ref().and_then(|p| p.affordable_on) == Some(today);
    rsx! {
        div { class: "{border} flex flex-col gap-3",
            div { class: "flex items-start gap-3",
                div { class: "flex flex-col",
                    button { class: "cursor-pointer rounded px-1 text-muted-foreground hover:bg-accent disabled:opacity-30 disabled:cursor-default",
                        title: "Higher priority", disabled: first,
                        onclick: move |_| store.mutate("", |db| db.move_goal(id, -1)), "▲" }
                    button { class: "cursor-pointer rounded px-1 text-muted-foreground hover:bg-accent disabled:opacity-30 disabled:cursor-default",
                        title: "Lower priority", disabled: last,
                        onclick: move |_| store.mutate("", |db| db.move_goal(id, 1)), "▼" }
                }
                div { class: "flex min-w-0 flex-1 flex-col gap-1",
                    div { class: "flex flex-wrap items-center gap-2",
                        span { class: "font-medium", "{goal.name}" }
                        span { class: "tabular-nums text-muted-foreground", {money(goal.cost, &sym)} }
                    }
                    if loading {
                        LoadingBlock { class: "h-4 w-48" }
                    } else if let Some(p) = &plan {
                        div { class: "flex flex-wrap items-center gap-2 text-sm",
                            span { class: if p.affordable_on.is_some() { "font-medium" } else { "font-medium text-destructive" },
                                {when_text(p, today)} }
                            match (&p.target, goal.target_date) {
                                (Some(TargetStatus::OnTrack), Some(t)) => rsx! {
                                    Badge { variant: BadgeVariant::Success, size: BadgeSize::Sm, "On track for {month_label(t)}" }
                                },
                                (Some(TargetStatus::Behind { extra_per_month }), Some(t)) => rsx! {
                                    Badge { variant: BadgeVariant::Warning, size: BadgeSize::Sm,
                                        "Wanted by {month_label(t)}: save {money(extra_per_month.ceil(), &sym)}/mo more" }
                                },
                                _ => rsx! {},
                            }
                        }
                    }
                    if !goal.notes.is_empty() {
                        span { class: "text-xs text-muted-foreground", "{goal.notes}" }
                    }
                }
                div { class: "flex shrink-0 items-center gap-1",
                    if confirm() {
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive,
                            onclick: move |_| store.mutate("Goal deleted", |db| db.delete_goal(id)), "Delete" }
                        Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| confirm.set(false), "Keep" }
                    } else {
                        Button { size: ButtonSize::Sm, variant: if affordable_now { ButtonVariant::Default } else { ButtonVariant::Outline },
                            title: "Mark as bought",
                            onclick: move |_| {
                                let g = Goal { purchased_on: Some(today), ..g3.clone() };
                                store.mutate("Marked as bought. Remember to update your account balances.", |db| db.save_goal(&g).map(|_| ()));
                            },
                            "Bought" }
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Edit",
                            onclick: move |_| on_edit.call(g2.clone()), Icon { name: IconName::Pencil } }
                        Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Delete",
                            onclick: move |_| confirm.set(true), Icon { name: IconName::Trash } }
                    }
                }
            }
            if let Some(p) = &plan {
                div { class: "flex items-center gap-3",
                    Progress { value: p.progress * 100.0, max: 100.0, class: "flex-1" }
                    span { class: "w-24 text-right text-xs tabular-nums text-muted-foreground",
                        {format!("{:.0}% saved", p.progress * 100.0)} }
                }
            }
        }
    }
}

#[component]
fn BoughtRow(goal: Goal, sym: String) -> Element {
    let mut store = use_store();
    let id = goal.id;
    let g = goal.clone();
    let when = goal.purchased_on.map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default();
    rsx! {
        div { class: "flex items-center gap-3 text-sm",
            span { class: "flex-1 text-muted-foreground line-through", "{goal.name}" }
            span { class: "tabular-nums text-muted-foreground", {money(goal.cost, &sym)} }
            span { class: "w-28 text-right text-xs text-muted-foreground", "{when}" }
            Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, title: "Move back to planned",
                onclick: move |_| {
                    let g = Goal { purchased_on: None, ..g.clone() };
                    store.mutate("Moved back to planned", |db| db.save_goal(&g).map(|_| ()));
                },
                "Undo" }
            Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Remove",
                onclick: move |_| store.mutate("", |db| db.delete_goal(id)), Icon { name: IconName::Trash } }
        }
    }
}

/// Spendable cash month by month, with the planned purchases taken out.
#[component]
fn SpendableChart(plan: GoalsPlan, loading: bool, sym: String) -> Element {
    if loading {
        return rsx! { Card { CardContent { ChartSkeleton {} } } };
    }
    // Show up to a few months past the last affordable goal (at least 2 years, at most 10).
    let last = plan.plans.iter().filter_map(|p| p.months_away).max().unwrap_or(0) as usize;
    let months = (last + 6).clamp(24, 120);
    let pts: Vec<_> = plan.spendable_after_goals.iter().take(months + 1).cloned().collect();
    let labels: Vec<String> = pts.iter().map(|(d, _)| d.format("%b %y").to_string()).collect();
    let values: Vec<f64> = pts.iter().map(|(_, v)| *v).collect();
    rsx! {
        Card {
            CardHeader {
                CardTitle { "Spendable cash after planned purchases" }
                CardDescription { "Checking + savings minus minimum balances and buckets, after buying each goal when it becomes affordable." }
            }
            CardContent {
                LineChart {
                    labels,
                    currency: sym,
                    series: vec![Series { name: "Spendable".into(), color: "var(--chart-3)".into(), values, bold: true }],
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct GoalForm {
    id: i64,
    name: String,
    cost: String,
    has_target: bool,
    target: String,
    notes: String,
    priority: i64,
}

impl GoalForm {
    fn new() -> Self {
        GoalForm { id: 0, name: String::new(), cost: String::new(), has_target: false, target: String::new(), notes: String::new(), priority: 0 }
    }
    fn from_goal(g: &Goal) -> Self {
        GoalForm {
            id: g.id,
            name: g.name.clone(),
            cost: num_str(g.cost),
            has_target: g.target_date.is_some(),
            target: g.target_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
            notes: g.notes.clone(),
            priority: g.priority,
        }
    }
    fn to_goal(&self) -> Result<Goal, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("What do you want to buy?".into());
        }
        let cost = parse_num(&self.cost).filter(|v| *v > 0.0).ok_or("Please enter the price.")?;
        let target_date = if self.has_target {
            Some(NaiveDate::parse_from_str(self.target.trim(), "%Y-%m-%d").map_err(|_| "Pick a date you want it by.")?)
        } else {
            None
        };
        Ok(Goal { id: self.id, name: name.into(), cost, target_date, priority: self.priority, notes: self.notes.trim().into(), purchased_on: None })
    }
}

#[component]
fn GoalFormCard(form: Signal<GoalForm>, generation: Signal<u32>, sym: String) -> Element {
    let mut store = use_store();
    let (mut form, mut generation) = (form, generation);
    let mut error = use_signal(|| None::<String>);
    let f = form.read().clone();
    let editing = f.id != 0;
    let save = move |_| {
        let res = form.read().to_goal();
        match res {
            Ok(g) => {
                error.set(None);
                store.mutate(if g.id == 0 { "Goal added" } else { "Goal saved" }, |db| db.save_goal(&g).map(|_| ()));
                form.set(GoalForm::new());
                generation += 1;
            }
            Err(e) => error.set(Some(e)),
        }
    };
    rsx! {
        Card {
            CardHeader { CardTitle { if editing { "Edit goal" } else { "Add a goal" } } }
            for g in [generation()] {
            CardContent { key: "{g}", class: "flex flex-col gap-4",
                Field { label: "What do you want to buy?",
                    Input { value: f.name.clone(), placeholder: "e.g. New laptop",
                        oninput: move |e: FormEvent| form.write().name = e.value() }
                }
                Field { label: "Price ({sym})",
                    Input { value: f.cost.clone(), placeholder: "0",
                        oninput: move |e: FormEvent| form.write().cost = e.value() }
                }
                div { class: "flex items-center justify-between",
                    span { class: "text-sm", "I want it by a date" }
                    Switch { checked: f.has_target, aria_label: "Has a target date",
                        onchange: move |v| {
                            let mut f = form.write();
                            f.has_target = v;
                            if v && f.target.is_empty() {
                                f.target = (today() + chrono::Duration::days(180)).format("%Y-%m-%d").to_string();
                            }
                        } }
                }
                if f.has_target {
                    Field { label: "Want it by",
                        for t in [f.target.clone()] {
                            Input { key: "{t}", r#type: InputType::Date, value: t.clone(),
                                oninput: move |e: FormEvent| form.write().target = e.value() }
                        }
                    }
                }
                Field { label: "Notes",
                    Input { value: f.notes.clone(), placeholder: "Optional", oninput: move |e: FormEvent| form.write().notes = e.value() }
                }
                if let Some(e) = error() {
                    p { class: "text-sm text-destructive", "{e}" }
                }
                div { class: "flex gap-2",
                    Button { onclick: save, Icon { name: IconName::Plus } if editing { "Save changes" } else { "Add goal" } }
                    if editing {
                        Button { variant: ButtonVariant::Outline,
                            onclick: move |_| { error.set(None); form.set(GoalForm::new()); generation += 1; }, "Cancel" }
                    }
                }
                p { class: "text-xs text-muted-foreground",
                    "Goals are paid from checking and savings, never touching minimum balances or money in buckets. Marking a goal as bought doesn't change balances, so update the account afterwards." }
            }
            }
        }
    }
}
