//! Income, Bills and Recurring expenses: one page for all three, as in 1.x.

use crate::app::use_store;
use crate::ui::icon::{Icon, IconName};
use crate::ui::*;
use chrono::{Datelike, NaiveDate};
use leptos::prelude::*;
use penguinfin_core::format::{money, money_cents, num_str, parse_num};
use penguinfin_core::ipc::Change;
use penguinfin_core::model::{AppData, CashFlow, FlowKind, Frequency};

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
    /// "" = default account, "a:<id>" = account, "b:<id>" = bucket.
    source: String,
}

impl FlowForm {
    fn new(kind: FlowKind, inflation: f64, today: NaiveDate) -> Self {
        FlowForm {
            id: 0,
            name: String::new(),
            category: String::new(),
            amount: String::new(),
            frequency: if kind == FlowKind::Income { Frequency::Biweekly } else { Frequency::Monthly },
            start_date: today.format("%Y-%m-%d").to_string(),
            end_date: String::new(),
            has_end: false,
            change: match kind {
                FlowKind::Income => "3".into(),
                FlowKind::Bill => "0".into(),
                FlowKind::Expense => num_str(inflation),
            },
            autopay: false,
            notes: String::new(),
            source: String::new(),
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
            source: match (f.bucket_id, f.account_id) {
                (Some(b), _) => format!("b:{b}"),
                (None, Some(a)) => format!("a:{a}"),
                _ => String::new(),
            },
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
            account_id: self.source.strip_prefix("a:").and_then(|v| v.parse().ok()),
            bucket_id: if kind.is_outflow() { self.source.strip_prefix("b:").and_then(|v| v.parse().ok()) } else { None },
        })
    }
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
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
pub fn FlowsPage(kind: FlowKind) -> impl IntoView {
    let store = use_store();
    let t = text_for(kind);
    let today = store.today();
    let inflation = move || store.snap.with_untracked(|s| s.as_ref().map(|s| s.data.settings.inflation_pct).unwrap_or(3.0));
    let form = RwSignal::new(FlowForm::new(kind, inflation(), today));
    let editing_id = Memo::new(move |_| form.with(|f| f.id));

    let list = move || {
        let data = store.data();
        let sym = data.settings.currency_symbol.clone();
        let flows: Vec<CashFlow> = data.flows.iter().filter(|f| f.kind == kind).cloned().collect();
        let monthly: f64 = flows.iter().filter(|f| f.end_date.is_none_or(|e| e >= today)).map(CashFlow::monthly_equivalent).sum();
        let title = format!("{} item(s)", flows.len());
        let description = format!("About {} per month · {} per year", money(monthly, &sym), money(monthly * 12.0, &sym));
        let rows: Vec<_> = flows.iter().map(|f| (f.clone(), data.flow_source_label(f))).collect();
        let body = if flows.is_empty() {
            view! { <EmptyState title=format!("No {} yet", t.singular) text="Use the form to add one." /> }.into_any()
        } else {
            view! {
                <TableWrapper class="max-h-none">
                    <Table>
                        <TableHeader>
                            <TableRow>
                                <TableHead>"Name"</TableHead>
                                <TableHead>"Amount"</TableHead>
                                <TableHead class="text-right">"Per month"</TableHead>
                                <TableHead>"Next"</TableHead>
                                <TableHead>""</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {rows.into_iter().map(|(f, source)| {
                                view! { <FlowRow flow=f source=source sym=sym.clone() today=today form=form editing_id=editing_id kind=kind /> }
                            }).collect_view()}
                        </TableBody>
                    </Table>
                </TableWrapper>
            }.into_any()
        };
        view! {
            <Card class="@5xl:col-span-2">
                <CardHeader>
                    <CardTitle>{title}</CardTitle>
                    <CardDescription>{description}</CardDescription>
                </CardHeader>
                <CardContent>{body}</CardContent>
            </Card>
        }
    };

    view! {
        <PageHeader title=t.title subtitle=t.subtitle />
        <div class="grid gap-6 @5xl:grid-cols-3">
            {list}
            <FlowFormCard kind=kind form=form />
        </div>
    }
}

#[component]
fn FlowFormCard(kind: FlowKind, form: RwSignal<FlowForm>) -> impl IntoView {
    let store = use_store();
    let t = text_for(kind);
    let error = RwSignal::new(None::<String>);
    let editing = move || form.with(|f| f.id != 0);
    let reset = move || {
        let inflation = store.snap.with_untracked(|s| s.as_ref().map(|s| s.data.settings.inflation_pct).unwrap_or(3.0));
        form.set(FlowForm::new(kind, inflation, store.today()));
    };
    let freq_options: Vec<(String, String)> = Frequency::ALL.iter().map(|f| (f.key().to_string(), f.label().to_string())).collect();
    let source_label = if kind == FlowKind::Income { "Deposited to" } else { "Paid from" };
    let source_hint = if kind == FlowKind::Income { "Which account this money lands in." } else { "Pick a bucket to pay it from money set aside for it." };
    let save = move |_| match form.with_untracked(|f| f.to_flow(kind)) {
        Ok(f) => {
            error.set(None);
            store.apply(Change::SaveFlow(f.clone()), if f.id == 0 { "Added" } else { "Saved" });
            reset();
        }
        Err(e) => error.set(Some(e)),
    };

    view! {
        <Card>
            <CardHeader>
                <CardTitle>{move || if editing() { format!("Edit {}", t.singular) } else { format!("Add {}", t.singular) }}</CardTitle>
            </CardHeader>
            <CardContent class="flex flex-col gap-4">
                <Field label="Name">
                    <FormInput form=form get=|f: &FlowForm| f.name.clone() set=|f: &mut FlowForm, v| f.name = v placeholder=format!("e.g. {}", t.categories[0]) />
                </Field>
                <div class="grid grid-cols-2 gap-3">
                    <Field label=format!("Amount ({})", store.currency())>
                        <FormInput form=form get=|f: &FlowForm| f.amount.clone() set=|f: &mut FlowForm, v| f.amount = v placeholder="0.00" inputmode="decimal" />
                    </Field>
                    <Field label="How often">
                        <NativeSelect label="How often" value=Signal::derive(move || form.with(|f| f.frequency.key().to_string())) options=freq_options
                            on_change=move |v: String| if let Some(fr) = Frequency::from_key(&v) { form.update(|f| f.frequency = fr) } />
                    </Field>
                </div>
                {move || {
                    let opts = store.snap.with(|s| s.as_ref().map(|s| source_options(&s.data, kind)).unwrap_or_default());
                    (opts.len() > 1).then(|| view! {
                        <Field label=source_label hint=source_hint>
                            <NativeSelect label=source_label value=Signal::derive(move || form.with(|f| f.source.clone())) options=opts
                                on_change=move |v: String| form.update(|f| f.source = v) />
                        </Field>
                    })
                }}
                <Field label="Category">
                    <FormInput form=form get=|f: &FlowForm| f.category.clone() set=|f: &mut FlowForm, v| f.category = v placeholder="Pick or type" />
                    <div class="flex flex-wrap gap-1">
                        {t.categories.iter().map(|c| view! {
                            <button type="button" class="cursor-pointer rounded-full border px-2 py-0.5 text-[0.6875rem] text-muted-foreground hover:bg-accent"
                                on:click=move |_| form.update(|f| f.category = c.to_string())>{*c}</button>
                        }).collect_view()}
                    </div>
                </Field>
                <Field label=t.date_label>
                    <FormInput form=form kind="date" get=|f: &FlowForm| f.start_date.clone() set=|f: &mut FlowForm, v| f.start_date = v />
                </Field>
                <SettingRow title="Has an end date">
                    <Switch checked=Signal::derive(move || form.with(|f| f.has_end)) label="Has an end date"
                        on_change=move |v| form.update(|f| { f.has_end = v; if v && f.end_date.is_empty() { f.end_date = f.start_date.clone(); } }) />
                </SettingRow>
                {move || form.with(|f| f.has_end).then(|| view! {
                    <Field label="Last payment on or before">
                        <FormInput form=form kind="date" get=|f: &FlowForm| f.end_date.clone() set=|f: &mut FlowForm, v| f.end_date = v />
                    </Field>
                })}
                <Field label=t.change_label hint=t.change_hint>
                    <FormInput form=form get=|f: &FlowForm| f.change.clone() set=|f: &mut FlowForm, v| f.change = v inputmode="decimal" />
                </Field>
                <SettingRow title=if kind == FlowKind::Income { "Direct deposit" } else { "Paid automatically (autopay)" }>
                    <FormSwitch form=form get=|f: &FlowForm| f.autopay set=|f: &mut FlowForm, v| f.autopay = v
                        label=if kind == FlowKind::Income { "Direct deposit" } else { "Autopay" } />
                </SettingRow>
                <Field label="Notes">
                    <FormInput form=form get=|f: &FlowForm| f.notes.clone() set=|f: &mut FlowForm, v| f.notes = v />
                </Field>
                {move || error.get().map(|e| view! { <p class="text-sm text-destructive">{e}</p> })}
                <div class="flex gap-2">
                    <Button on_click=save><Icon name=IconName::Plus />{move || if editing() { "Save changes" } else { "Add" }}</Button>
                    {move || editing().then(|| view! {
                        <Button variant=ButtonVariant::Outline on_click=move |_| { error.set(None); reset(); }>"Cancel"</Button>
                    })}
                </div>
            </CardContent>
        </Card>
    }
}

#[component]
fn FlowRow(flow: CashFlow, source: String, sym: String, today: NaiveDate, form: RwSignal<FlowForm>, editing_id: Memo<i64>, kind: FlowKind) -> impl IntoView {
    let store = use_store();
    let next = flow.next_due(today);
    let ended = flow.end_date.is_some_and(|e| e < today);
    let id = flow.id;
    let f2 = flow.clone();
    let subtitle = format!(
        "{}{}",
        if flow.category.is_empty() { "Uncategorized".to_string() } else { flow.category.clone() },
        if flow.annual_change_pct != 0.0 { format!(" · {}%/yr", num_str(flow.annual_change_pct)) } else { String::new() }
    );
    let source_line = (!source.is_empty()).then(|| if flow.kind == FlowKind::Income { format!("into {source}") } else { format!("from {source}") });
    let next_view = if ended {
        view! { <Badge variant=BadgeVariant::Muted>"ended"</Badge> }.into_any()
    } else if let Some(n) = next {
        let s = if n.year() == today.year() { n.format("%b %-d").to_string() } else { n.format("%b %-d, %Y").to_string() };
        view! { <span class="text-xs whitespace-nowrap">{s}</span> }.into_any()
    } else {
        view! { <span class="text-xs text-muted-foreground">"—"</span> }.into_any()
    };
    let (name, amount, freq, monthly) =
        (flow.name.clone(), money_cents(flow.amount, &sym), flow.frequency.label(), money(flow.monthly_equivalent(), &sym));
    let auto = (flow.autopay && !ended).then(|| view! { <Badge variant=BadgeVariant::Muted>{if flow.kind == FlowKind::Income { "direct" } else { "auto" }}</Badge> });
    view! {
        <tr class=move || format!("border-b transition-colors hover:bg-muted/50 {}", if editing_id.get() == id { "bg-muted" } else { "" })>
            <TableCell>
                <div class="flex flex-col">
                    <span class="font-medium">{name}</span>
                    <span class="text-xs text-muted-foreground">{subtitle}</span>
                    {source_line.map(|s| view! { <span class="text-xs text-muted-foreground">{s}</span> })}
                </div>
            </TableCell>
            <TableCell>
                <div class="flex flex-col whitespace-nowrap">
                    <span class="tabular-nums">{amount}</span>
                    <span class="text-xs text-muted-foreground">{freq}</span>
                </div>
            </TableCell>
            <TableCell class="text-right tabular-nums">{monthly}</TableCell>
            <TableCell><div class="flex items-center gap-1">{next_view}{auto}</div></TableCell>
            <TableCell class="text-right whitespace-nowrap">
                <RowActions
                    on_edit=move |_| form.set(FlowForm::from_flow(&f2))
                    on_delete=move |_| {
                        store.apply(Change::DeleteFlow(id), "Deleted");
                        if form.with_untracked(|f| f.id == id) {
                            let inflation = store.snap.with_untracked(|s| s.as_ref().map(|s| s.data.settings.inflation_pct).unwrap_or(3.0));
                            form.set(FlowForm::new(kind, inflation, store.today()));
                        }
                    }
                />
            </TableCell>
        </tr>
    }
}

/// Choices for where a flow's money goes to / comes from: default, each account, and
/// (for bills/expenses) each bucket as "Account › Bucket".
fn source_options(data: &AppData, kind: FlowKind) -> Vec<(String, String)> {
    let default = match data.primary_cash_account() {
        Some(a) => format!("Default ({})", a.name),
        None => "Default".to_string(),
    };
    let mut opts = vec![(String::new(), default)];
    for a in &data.accounts {
        opts.push((format!("a:{}", a.id), a.name.clone()));
        if kind.is_outflow() {
            for b in data.buckets_for(a.id).filter(|b| b.debt_id.is_none()) {
                opts.push((format!("b:{}", b.id), format!("{} › {}", a.name, b.name)));
            }
        }
    }
    opts
}
