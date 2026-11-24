mod atspi;
mod cache_group;
mod canonical;
mod gtk4_hook;
mod gtk_native;

use crate::state::AppState;
use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

pub fn build_protocols_view(state: &AppState) -> gtk::Widget {
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let page = adw::PreferencesPage::new();
    page.add(&gtk4_hook::build_gtk4_hook_group(state));
    page.add(&gtk_native::build_gtk_native_group(state));
    page.add(&canonical::build_canonical_group(state));
    page.add(&atspi::build_atspi_group(state));
    page.add(&cache_group::build_cache_group(state));

    scrolled.set_child(Some(&page));
    scrolled.upcast()
}
