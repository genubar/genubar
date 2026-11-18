mod root_sorter_group;
mod templates_group;

use crate::state::AppState;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use root_sorter_group::build_root_sorter_group;
use templates_group::build_templates_group;

pub fn build_system_menus_view(state: &AppState) -> gtk::Widget {
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let page = adw::PreferencesPage::new();
    page.add(&build_root_sorter_group(state));
    page.add(&build_templates_group(state));

    scrolled.set_child(Some(&page));
    scrolled.upcast()
}
