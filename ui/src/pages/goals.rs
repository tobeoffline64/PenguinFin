//! Savings goals: when each planned purchase becomes affordable, keeping minimum balances and
//! bucket money untouched. The plan runs the engine's projection here in the interface.

use crate::app::{Page, use_store};
use crate::charts::{LineChart, Series};
use crate::pages::flows::parse_date;
use crate::ui::icon::{Icon, IconName};
use crate::ui::*;
use chrono::NaiveDate;
use leptos::prelude::*;
use penguinfin_core::format::{money, num_str, parse_num};
use penguinfin_core::goals::{GoalPlan, GoalsPlan, TargetStatus, plan_goals};
use penguinfin_core::ipc::Change;
use penguinfin_core::model::{AccountGroup, FlowKind, Goal};

/// How far ahead to look for goals: at least 10 years, or the projection length if longer.
fn horizon(projection_years: u32) -> u32 {
    projection_years.max(10)
}

pub fn month_label(d: NaiveDate) -> String {
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
pub fn GoalsPage(#[prop(into)] on_nav: Callback<Page>) -> impl IntoView {
    let store = use_store();
    let today = store.today();
    let form = RwSignal::new(GoalForm::new());
    let plan = Memo::new(move |_| {
        let data = store.data();
        plan_goals(&data, today, horizon(data.settings.projection_years))
    });

    let summary = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let income = data.monthly_total(|k| k == FlowKind::Income, today);
        let outflow = data.monthly_total(|k| k != FlowKind::Income, today);
        let leftover = income - outflow - data.monthly_cash_contributions();
        let baseline = data.cash_baseline();
        let bucketed = data.cash_buckets_total();
        let reserved = baseline + bucketed;
        let cash_now = data.group_total(AccountGroup::Cash);
        let total_open: f64 = data.goals.iter().filter(|g| g.purchased_on.is_none()).map(|g| g.cost).sum();
        let cash_accounts: Vec<_> = data.accounts.iter().filter(|a| a.kind.group() == AccountGroup::Cash).cloned().collect();
        let chips = cash_accounts
            .iter()
            .filter(|a| a.minimum_balance > 0.0)
            .map(|a| {
                let purpose_text = a.purpose.clone();
                let purpose = (!purpose_text.is_empty()).then(move || view! { <Badge variant=BadgeVariant::Info>{purpose_text}</Badge> });
                view! {
                    <div class="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm">
                        <span class="font-medium">{a.name.clone()}</span>
                        {purpose}
                        <span class="tabular-nums text-muted-foreground">{format!("min {}", money(a.minimum_balance, &sym))}</span>
                    </div>
                }
            })
            .collect_view();
        let bucket_chips: Vec<_> = data
            .buckets
            .iter()
            .filter_map(|b| {
                let a = cash_accounts.iter().find(|a| a.id == b.account_id)?;
                Some(view! {
                    <div class="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm">
                        <span class="font-medium">{format!("{} › {}", a.name, b.name)}</span>
                        <span class="tabular-nums text-muted-foreground">{format!("bucket {}", money(b.balance, &sym))}</span>
                    </div>
                })
            })
            .collect();
        let reserved_view = if reserved <= 0.0 && !cash_accounts.is_empty() {
            Some(view! {
                <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-dashed p-4 text-sm">
                    <span class="text-muted-foreground">
                        "Nothing reserved yet, so every dollar in checking and savings counts as spendable. Set minimum balances or buckets (like Rainy day money) in Accounts to keep money for bills, medical, food and emergencies."
                    </span>
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm on_click=move |_| on_nav.run(Page::Accounts)>"Set minimums in Accounts"</Button>
                </div>
            }.into_any())
        } else if reserved > 0.0 {
            Some(view! {
                <Card small=true>
                    <CardHeader>
                        <CardTitle>"Reserved money"</CardTitle>
                        <CardDescription>"Minimum balances and buckets are kept in place before any goal is bought. Change these in Accounts."</CardDescription>
                    </CardHeader>
                    <CardContent class="flex flex-wrap gap-2">{chips}{bucket_chips}</CardContent>
                </Card>
            }.into_any())
        } else {
            None
        };
        view! {
            <div class="grid grid-cols-2 gap-4 @3xl:grid-cols-4">
                <Stat label="Checking + savings" value=money(cash_now, &sym) sub="Where goals are paid from" />
                <Stat label="Reserved" value=money(reserved, &sym) sub=format!("Minimums {} · Buckets {}", money(baseline, &sym), money(bucketed, &sym)) />
                <Stat label="Spendable now" value=money(cash_now - reserved, &sym) tone=if cash_now - reserved < 0.0 { "bad" } else { "good" }
                    sub=format!("Planned goals total {}", money(total_open, &sym)) />
                <Stat label="Left over / month" value=money(leftover, &sym) tone=if leftover < 0.0 { "bad" } else { "" } sub="After bills, expenses and savings transfers" />
            </div>
            {reserved_view}
        }
    };

    let lists = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let p = plan.get();
        let mut open: Vec<Goal> = data.goals.iter().filter(|g| g.purchased_on.is_none()).cloned().collect();
        open.sort_by_key(|g| (g.priority, g.id));
        let mut bought: Vec<Goal> = data.goals.iter().filter(|g| g.purchased_on.is_some()).cloned().collect();
        bought.sort_by_key(|g| std::cmp::Reverse(g.purchased_on));
        let n = open.len();
        let title = format!("Planned ({n})");
        let rows = if open.is_empty() {
            view! { <EmptyState title="No goals yet" text="Add something you'd like to buy and PenguinFin will tell you when you can afford it." /> }.into_any()
        } else {
            open.iter()
                .enumerate()
                .map(|(i, g)| view! { <GoalRow goal=g.clone() plan=p.plan_for(g.id).cloned() first=i == 0 last=i + 1 == n sym=sym.clone() form=form today=today /> })
                .collect_view()
                .into_any()
        };
        let chart = (!open.is_empty()).then(|| view! { <SpendableChart plan=p.clone() sym=sym.clone() /> });
        let bought_view = (!bought.is_empty()).then(|| view! {
            <Card small=true>
                <CardHeader><CardTitle>"Bought"</CardTitle></CardHeader>
                <CardContent class="flex flex-col gap-2">
                    {bought.into_iter().map(|g| view! { <BoughtRow goal=g sym=sym.clone() /> }).collect_view()}
                </CardContent>
            </Card>
        });
        view! {
            <Card>
                <CardHeader>
                    <CardTitle>{title}</CardTitle>
                    <CardDescription>"Bought in this order. Use the arrows to change priority."</CardDescription>
                </CardHeader>
                <CardContent class="flex flex-col gap-3">{rows}</CardContent>
            </Card>
            {chart}
            {bought_view}
        }
    };

    view! {
        <PageHeader title="Savings goals" subtitle="Things you want to buy, and when you can afford each one while keeping your minimum balances." />
        {summary}
        <div class="grid gap-6 @5xl:grid-cols-3">
            <div class="flex flex-col gap-6 @5xl:col-span-2">{lists}</div>
            <GoalFormCard form=form />
        </div>
    }
}

#[component]
fn GoalRow(goal: Goal, plan: Option<GoalPlan>, first: bool, last: bool, sym: String, form: RwSignal<GoalForm>, today: NaiveDate) -> impl IntoView {
    let store = use_store();
    let id = goal.id;
    let affordable_now = plan.as_ref().and_then(|p| p.affordable_on) == Some(today);
    let g_edit = goal.clone();
    let g_bought = goal.clone();
    let cost = money(goal.cost, &sym);
    let status = plan.as_ref().map(|p| {
        let class = if p.affordable_on.is_some() { "font-medium" } else { "font-medium text-destructive" };
        let badge = match (&p.target, goal.target_date) {
            (Some(TargetStatus::OnTrack), Some(t)) => Some(view! { <Badge variant=BadgeVariant::Success>{format!("On track for {}", month_label(t))}</Badge> }.into_any()),
            (Some(TargetStatus::Behind { extra_per_month }), Some(t)) => Some(
                view! { <Badge variant=BadgeVariant::Warning>{format!("Wanted by {}: save {}/mo more", month_label(t), money(extra_per_month.ceil(), &sym))}</Badge> }.into_any(),
            ),
            _ => None,
        };
        view! { <div class="flex flex-wrap items-center gap-2 text-sm"><span class=class>{when_text(p, today)}</span>{badge}</div> }
    });
    let progress = plan.as_ref().map(|p| {
        let pct = p.progress * 100.0;
        view! {
            <div class="flex items-center gap-3">
                <Progress value=pct class="flex-1" />
                <span class="w-24 text-right text-xs tabular-nums text-muted-foreground">{format!("{pct:.0}% saved")}</span>
            </div>
        }
    });
    let notes = (!goal.notes.is_empty()).then(|| view! { <span class="text-xs text-muted-foreground">{goal.notes.clone()}</span> });
    let arrow = "cursor-pointer rounded px-1 text-muted-foreground hover:bg-accent disabled:opacity-30 disabled:cursor-default";
    view! {
        <div class=move || if form.with(|f| f.id) == id { "rounded-lg border border-ring p-4 flex flex-col gap-3" } else { "rounded-lg border p-4 flex flex-col gap-3" }>
            <div class="flex items-start gap-3">
                <div class="flex flex-col">
                    <button type="button" class=arrow title="Higher priority" aria-label="Higher priority" disabled=first
                        on:click=move |_| store.apply(Change::MoveGoal { id, direction: -1 }, "")>"▲"</button>
                    <button type="button" class=arrow title="Lower priority" aria-label="Lower priority" disabled=last
                        on:click=move |_| store.apply(Change::MoveGoal { id, direction: 1 }, "")>"▼"</button>
                </div>
                <div class="flex min-w-0 flex-1 flex-col gap-1">
                    <div class="flex flex-wrap items-center gap-2">
                        <span class="font-medium">{goal.name.clone()}</span>
                        <span class="tabular-nums text-muted-foreground">{cost}</span>
                    </div>
                    {status}
                    {notes}
                </div>
                <div class="flex shrink-0 items-center gap-1">
                    <Button size=ButtonSize::Sm variant=if affordable_now { ButtonVariant::Default } else { ButtonVariant::Outline } title="Mark as bought"
                        on_click=move |_| store.apply(Change::SaveGoal(Goal { purchased_on: Some(today), ..g_bought.clone() }), "Marked as bought. Remember to update your account balances.")>
                        "Bought"
                    </Button>
                    <RowActions on_edit=move |_| form.set(GoalForm::from_goal(&g_edit)) on_delete=move |_| store.apply(Change::DeleteGoal(id), "Goal deleted") />
                </div>
            </div>
            {progress}
        </div>
    }
}

#[component]
fn BoughtRow(goal: Goal, sym: String) -> impl IntoView {
    let store = use_store();
    let id = goal.id;
    let when = goal.purchased_on.map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default();
    let cost = money(goal.cost, &sym);
    let g = goal.clone();
    view! {
        <div class="flex items-center gap-3 text-sm">
            <span class="flex-1 text-muted-foreground line-through">{goal.name.clone()}</span>
            <span class="tabular-nums text-muted-foreground">{cost}</span>
            <span class="w-28 text-right text-xs text-muted-foreground">{when}</span>
            <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost title="Move back to planned"
                on_click=move |_| store.apply(Change::SaveGoal(Goal { purchased_on: None, ..g.clone() }), "Moved back to planned")>"Undo"</Button>
            <Button size=ButtonSize::IconSm variant=ButtonVariant::Ghost title="Remove" label="Remove" on_click=move |_| store.apply(Change::DeleteGoal(id), "")>
                <Icon name=IconName::Trash />
            </Button>
        </div>
    }
}

/// Spendable cash month by month, with the planned purchases taken out.
#[component]
fn SpendableChart(plan: GoalsPlan, sym: String) -> impl IntoView {
    // Up to a few months past the last affordable goal (at least 2 years, at most 10).
    let last = plan.plans.iter().filter_map(|p| p.months_away).max().unwrap_or(0) as usize;
    let months = (last + 6).clamp(24, 120);
    let pts: Vec<_> = plan.spendable_after_goals.iter().take(months + 1).cloned().collect();
    let labels: Vec<String> = pts.iter().map(|(d, _)| d.format("%b %y").to_string()).collect();
    let values: Vec<f64> = pts.iter().map(|(_, v)| *v).collect();
    view! {
        <Card>
            <CardHeader>
                <CardTitle>"Spendable cash after planned purchases"</CardTitle>
                <CardDescription>"Checking + savings minus minimum balances and buckets, after buying each goal when it becomes affordable."</CardDescription>
            </CardHeader>
            <CardContent>
                <LineChart labels=labels currency=sym series=vec![Series::new("Spendable", "var(--chart-3)", values, true)] />
            </CardContent>
        </Card>
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
        let target_date = if self.has_target { Some(parse_date(&self.target).ok_or("Pick a date you want it by.")?) } else { None };
        Ok(Goal { id: self.id, name: name.into(), cost, target_date, priority: self.priority, notes: self.notes.trim().into(), purchased_on: None })
    }
}

#[component]
fn GoalFormCard(form: RwSignal<GoalForm>) -> impl IntoView {
    let store = use_store();
    let error = RwSignal::new(None::<String>);
    let editing = move || form.with(|f| f.id != 0);
    let save = move |_| match form.with_untracked(GoalForm::to_goal) {
        Ok(g) => {
            error.set(None);
            store.apply(Change::SaveGoal(g.clone()), if g.id == 0 { "Goal added" } else { "Goal saved" });
            form.set(GoalForm::new());
        }
        Err(e) => error.set(Some(e)),
    };
    view! {
        <Card>
            <CardHeader><CardTitle>{move || if editing() { "Edit goal" } else { "Add a goal" }}</CardTitle></CardHeader>
            <CardContent class="flex flex-col gap-4">
                <Field label="What do you want to buy?">
                    <FormInput form=form get=|f: &GoalForm| f.name.clone() set=|f: &mut GoalForm, v| f.name = v placeholder="e.g. New laptop" />
                </Field>
                <Field label=format!("Price ({})", store.currency())>
                    <FormInput form=form get=|f: &GoalForm| f.cost.clone() set=|f: &mut GoalForm, v| f.cost = v placeholder="0" inputmode="decimal" />
                </Field>
                <SettingRow title="I want it by a date">
                    <Switch checked=Signal::derive(move || form.with(|f| f.has_target)) label="Has a target date"
                        on_change=move |v| {
                            let six_months = (store.today() + chrono::Duration::days(180)).format("%Y-%m-%d").to_string();
                            form.update(|f| { f.has_target = v; if v && f.target.is_empty() { f.target = six_months; } })
                        } />
                </SettingRow>
                {move || form.with(|f| f.has_target).then(|| view! {
                    <Field label="Want it by">
                        <FormInput form=form kind="date" get=|f: &GoalForm| f.target.clone() set=|f: &mut GoalForm, v| f.target = v />
                    </Field>
                })}
                <Field label="Notes">
                    <FormInput form=form get=|f: &GoalForm| f.notes.clone() set=|f: &mut GoalForm, v| f.notes = v placeholder="Optional" />
                </Field>
                {move || error.get().map(|e| view! { <p class="text-sm text-destructive">{e}</p> })}
                <div class="flex gap-2">
                    <Button on_click=save><Icon name=IconName::Plus />{move || if editing() { "Save changes" } else { "Add goal" }}</Button>
                    {move || editing().then(|| view! { <Button variant=ButtonVariant::Outline on_click=move |_| { error.set(None); form.set(GoalForm::new()) }>"Cancel"</Button> })}
                </div>
                <p class="text-xs text-muted-foreground">
                    "Goals are paid from checking and savings, never touching minimum balances or money in buckets. Marking a goal as bought doesn't change balances, so update the account afterwards."
                </p>
            </CardContent>
        </Card>
    }
}
