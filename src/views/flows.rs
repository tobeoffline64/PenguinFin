use crate::app::{today, use_store};
use crate::ui::badge::*;
use crate::ui::button::*;
use crate::ui::card::*;
use crate::ui::input::*;
use crate::ui::switch::Switch;
use crate::ui::table::*;
use crate::views::dashboard::PageHeader;
use crate::widgets::*;
use chrono::{Datelike, NaiveDate};
use dioxus::prelude::*;
use penguinfin::model::{CashFlow, FlowKind, Frequency};

#[derive(Clone, PartialEq)]
struct FlowForm {
    id: i64,
    name: String,
    category: String,
    amount: String,
    frequency: Frequency,
    start_date: String,
    end_date: String,
    has_end: bool,
    change: String,
    autopay: bool,
    notes: String,
}

impl FlowForm {
    fn new(kind: FlowKind, inflation: f64) -> Self {
        FlowForm {
            id: 0,
            name: String::new(),
            category: String::new(),
            amount: String::new(),
            frequency: if kind == FlowKind::Income { Frequency::Biweekly } else { Frequency::Monthly },
            start_date: today().format("%Y-%m-%d").to_string(),
            end_date: String::new(),
            has_end: false,
            change: match kind {
                FlowKind::Income => "3".into(),
                FlowKind::Bill => "0".into(),
                FlowKind::Expense => num_str(inflation),
            },
            autopay: false,
            notes: String::new(),
        }
    }

    fn from_flow(f: &CashFlow) -> Self {
        FlowForm {
            id: f.id,
            name: f.name.clone(),
            category: f.category.clone(),
            amount: num_str(f.amount),
            frequency: f.frequency,
            start_date: f.start_date.format("%Y-%m-%d").to_string(),
            end_date: f.end_date.map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default(),
            has_end: f.end_date.is_some(),
            change: num_str(f.annual_change_pct),
            autopay: f.autopay,
            notes: f.notes.clone(),
        }
    }

    fn to_flow(&self, kind: FlowKind) -> Result<CashFlow, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Please enter a name.".into());
        }
        let amount = parse_num(&self.amount).filter(|v| *v >= 0.0).ok_or("Please enter a valid amount.")?;
        let start = parse_date(&self.start_date).ok_or("Please enter a start / due date (YYYY-MM-DD).")?;
        let end = if !self.has_end || self.end_date.trim().is_empty() {
            None
        } else {
            Some(parse_date(&self.end_date).ok_or("End date must look like YYYY-MM-DD.")?)
        };
        if end.is_some_and(|e| e < start) {
            return Err("End date is before the start date.".into());
        }
        Ok(CashFlow {
            id: self.id,
            kind,
            name: name.to_string(),
            category: self.category.trim().to_string(),
            amount,
            frequency: self.frequency,
            start_date: start,
            end_date: end,
            annual_change_pct: parse_num(&self.change).unwrap_or(0.0),
            autopay: self.autopay,
            notes: self.notes.trim().to_string(),
        })
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok()
}

struct KindText {
    title: &'static str,
    subtitle: &'static str,
    singular: &'static str,
    date_label: &'static str,
    change_label: &'static str,
    change_hint: &'static str,
    categories: &'static [&'static str],
}

fn text_for(kind: FlowKind) -> KindText {
    match kind {
        FlowKind::Income => KindText {
            title: "Income",
            subtitle: "Paychecks and other money coming in. Enter take-home (after tax and payroll deductions).",
            singular: "income",
            date_label: "Next / first pay date",
            change_label: "Yearly raise %",
            change_hint: "Applied once per year in projections",
            categories: &["Salary", "Bonus", "Freelance", "Rental", "Benefits", "Other"],
        },
        FlowKind::Bill => KindText {
            title: "Bills",
            subtitle: "Fixed obligations with a due date: rent, mortgage, utilities, loans, insurance.",
            singular: "bill",
            date_label: "Due date (first or next)",
            change_label: "Yearly change %",
            change_hint: "0 for fixed payments like a mortgage",
            categories: &["Housing", "Utilities", "Insurance", "Transportation", "Debt", "Medical", "Childcare", "Other"],
        },
        FlowKind::Expense => KindText {
            title: "Recurring expenses",
            subtitle: "Spending that repeats: groceries, gas, subscriptions, dining, hobbies.",
            singular: "expense",
            date_label: "Starts on",
            change_label: "Yearly change %",
            change_hint: "Defaults to your inflation setting",
            categories: &["Food", "Transportation", "Subscriptions", "Personal", "Shopping", "Entertainment", "Health", "Pets", "Other"],
        },
    }
}

#[component]
pub fn FlowsPage(kind: FlowKind) -> Element {
    let mut store = use_store();
    let data = store.data.read().clone();
    let sym = data.settings.currency_symbol.clone();
    let inflation = data.settings.inflation_pct;
    let t = text_for(kind);
    let mut form = use_signal(|| FlowForm::new(kind, inflation));
    let mut error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0u32);
    let today = today();

    let flows: Vec<CashFlow> = data.flows.iter().filter(|f| f.kind == kind).cloned().collect();
    let monthly_total: f64 = flows
        .iter()
        .filter(|f| f.end_date.is_none_or(|e| e >= today))
        .map(CashFlow::monthly_equivalent)
        .sum();

    let editing = form.read().id != 0;
    let freq_options: Vec<(String, String)> =
        Frequency::ALL.iter().map(|f| (f.key().to_string(), f.label().to_string())).collect();

    let save = move |_| {
        let res = form.read().to_flow(kind);
        match res {
            Ok(f) => {
                error.set(None);
                let msg = if f.id == 0 { "Added" } else { "Saved" };
                store.mutate(msg, |db| db.save_flow(&f).map(|_| ()));
                form.set(FlowForm::new(kind, inflation));
                generation += 1;
            }
            Err(e) => error.set(Some(e)),
        }
    };

    rsx! {
        PageHeader { title: t.title, subtitle: t.subtitle }

        div { class: "grid gap-6 xl:grid-cols-3",
            Card { class: "xl:col-span-2",
                CardHeader {
                    CardTitle { "{flows.len()} item(s)" }
                    CardDescription { "About {money(monthly_total, &sym)} per month · {money(monthly_total * 12.0, &sym)} per year" }
                }
                CardContent {
                    if flows.is_empty() {
                        EmptyState { title: format!("No {} yet", t.singular), text: "Use the form to add one." }
                    } else {
                        TableWrapper { class: "max-h-none",
                            Table {
                                TableHeader {
                                    TableRow {
                                        TableHead { "Name" }
                                        TableHead { "Amount" }
                                        TableHead { class: "text-right", "Per month" }
                                        TableHead { "Next" }
                                        TableHead { "" }
                                    }
                                }
                                TableBody {
                                    for f in flows.iter().cloned() {
                                        FlowRow {
                                            key: "{f.id}",
                                            flow: f.clone(),
                                            sym: sym.clone(),
                                            selected: form.read().id == f.id,
                                            on_edit: move |f: CashFlow| { error.set(None); form.set(FlowForm::from_flow(&f)); generation += 1; },
                                            on_delete: move |id: i64| {
                                                store.mutate("Deleted", |db| db.delete_flow(id));
                                                if form.read().id == id { form.set(FlowForm::new(kind, inflation)); generation += 1; }
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Card {
                CardHeader {
                    CardTitle { if editing { "Edit {t.singular}" } else { "Add {t.singular}" } }
                }
                for g in [generation()] {
                CardContent { key: "{g}", class: "flex flex-col gap-4",
                    Field { label: "Name",
                        Input { value: form.read().name.clone(), placeholder: "e.g. {t.categories[0]}",
                            oninput: move |e: FormEvent| form.write().name = e.value() }
                    }
                    div { class: "grid grid-cols-2 gap-3",
                        Field { label: "Amount ({sym})",
                            Input { value: form.read().amount.clone(), placeholder: "0.00",
                                oninput: move |e: FormEvent| form.write().amount = e.value() }
                        }
                        Field { label: "How often",
                            NativeSelect {
                                value: form.read().frequency.key().to_string(),
                                options: freq_options.clone(),
                                onchange: move |v: String| if let Some(f) = Frequency::from_key(&v) { form.write().frequency = f },
                            }
                        }
                    }
                    Field { label: "Category",
                        Input { value: form.read().category.clone(), placeholder: "Pick or type",
                            oninput: move |e: FormEvent| form.write().category = e.value() }
                        div { class: "flex flex-wrap gap-1",
                            for c in t.categories.iter() {
                                button {
                                    class: "cursor-pointer rounded-full border px-2 py-0.5 text-[11px] text-muted-foreground hover:bg-accent",
                                    onclick: move |_| { form.write().category = c.to_string(); generation += 1; },
                                    "{c}"
                                }
                            }
                        }
                    }
                    Field { label: t.date_label,
                        Input { r#type: InputType::Date, value: form.read().start_date.clone(),
                            oninput: move |e: FormEvent| form.write().start_date = e.value() }
                    }
                    div { class: "flex items-center justify-between",
                        span { class: "text-sm", "Has an end date" }
                        Switch { checked: form.read().has_end, aria_label: "Has an end date",
                            onchange: move |v| {
                                let mut f = form.write();
                                f.has_end = v;
                                if v && f.end_date.is_empty() { f.end_date = f.start_date.clone(); }
                            } }
                    }
                    if form.read().has_end {
                        Field { label: "Last payment on or before",
                            Input { r#type: InputType::Date, value: form.read().end_date.clone(),
                                oninput: move |e: FormEvent| form.write().end_date = e.value() }
                        }
                    }
                    Field { label: t.change_label, hint: t.change_hint,
                        Input { value: form.read().change.clone(),
                            oninput: move |e: FormEvent| form.write().change = e.value() }
                    }
                    if kind != FlowKind::Income {
                        div { class: "flex items-center justify-between",
                            span { class: "text-sm", "Paid automatically (autopay)" }
                            Switch { checked: form.read().autopay, aria_label: "Autopay",
                                onchange: move |v| form.write().autopay = v }
                        }
                    } else {
                        div { class: "flex items-center justify-between",
                            span { class: "text-sm", "Direct deposit" }
                            Switch { checked: form.read().autopay, aria_label: "Direct deposit",
                                onchange: move |v| form.write().autopay = v }
                        }
                    }
                    Field { label: "Notes",
                        Input { value: form.read().notes.clone(), oninput: move |e: FormEvent| form.write().notes = e.value() }
                    }
                    if let Some(e) = error() {
                        p { class: "text-sm text-destructive", "{e}" }
                    }
                    div { class: "flex gap-2",
                        Button { onclick: save, Icon { name: IconName::Plus } if editing { "Save changes" } else { "Add" } }
                        if editing {
                            Button { variant: ButtonVariant::Outline,
                                onclick: move |_| { error.set(None); form.set(FlowForm::new(kind, inflation)); generation += 1; }, "Cancel" }
                        }
                    }
                }
                }
            }
        }
    }
}

#[component]
fn FlowRow(flow: CashFlow, sym: String, selected: bool, on_edit: EventHandler<CashFlow>, on_delete: EventHandler<i64>) -> Element {
    let today = today();
    let next = flow.next_due(today);
    let ended = flow.end_date.is_some_and(|e| e < today);
    let mut confirm = use_signal(|| false);
    let f2 = flow.clone();
    let id = flow.id;
    rsx! {
        TableRow { data_state: if selected { "selected" } else { "" },
            TableCell {
                div { class: "flex flex-col",
                    span { class: "font-medium", "{flow.name}" }
                    span { class: "text-xs text-muted-foreground",
                        if flow.category.is_empty() { "Uncategorized" } else { "{flow.category}" }
                        if flow.annual_change_pct != 0.0 { " · {num_str(flow.annual_change_pct)}%/yr" }
                    }
                }
            }
            TableCell {
                div { class: "flex flex-col whitespace-nowrap",
                    span { class: "tabular-nums", {money_cents(flow.amount, &sym)} }
                    span { class: "text-xs text-muted-foreground", "{flow.frequency.label()}" }
                }
            }
            TableCell { class: "text-right tabular-nums", {money(flow.monthly_equivalent(), &sym)} }
            TableCell {
                div { class: "flex items-center gap-1",
                    if ended {
                        Badge { variant: BadgeVariant::Muted, size: BadgeSize::Sm, "ended" }
                    } else if let Some(n) = next {
                        span { class: "text-xs whitespace-nowrap",
                            if n.year() == today.year() { "{n.format(\"%b %-d\")}" } else { "{n.format(\"%b %-d, %Y\")}" } }
                    } else {
                        span { class: "text-xs text-muted-foreground", "—" }
                    }
                    if flow.autopay && !ended {
                        Badge { variant: BadgeVariant::Muted, size: BadgeSize::Sm,
                            if flow.kind == FlowKind::Income { "direct" } else { "auto" } }
                    }
                }
            }
            TableCell { class: "text-right whitespace-nowrap",
                if confirm() {
                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Destructive, onclick: move |_| on_delete.call(id), "Delete" }
                    Button { size: ButtonSize::Sm, variant: ButtonVariant::Ghost, onclick: move |_| confirm.set(false), "Keep" }
                } else {
                    Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Edit",
                        onclick: move |_| on_edit.call(f2.clone()), Icon { name: IconName::Pencil } }
                    Button { size: ButtonSize::IconSm, variant: ButtonVariant::Ghost, title: "Delete",
                        onclick: move |_| confirm.set(true), Icon { name: IconName::Trash } }
                }
            }
        }
    }
}
