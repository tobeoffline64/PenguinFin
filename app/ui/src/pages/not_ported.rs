use crate::ui::*;
use leptos::prelude::*;

/// A page that has not been moved to PenguinFin 2 yet (phase 3 of the migration).
#[component]
pub fn NotPorted(title: &'static str) -> impl IntoView {
    view! {
        <PageHeader title=title />
        <EmptyState
            title="This page is moving to PenguinFin 2"
            text="Your data for this page is here and safe. Until the page is finished, use PenguinFin 1 to view and edit it."
        />
    }
}
