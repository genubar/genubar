mod bundle_row;
mod import_group;

use crate::components::controls::{build_scrolled_container, clear_box};
use crate::components::rows::build_action_row;
use crate::services::bundle_service::BundleService;
use crate::state::AppState;
use bundle_row::build_bundle_row;
use gtk4 as gtk;
use gtk::prelude::*;
use import_group::build_import_bundle_group;
use libadwaita as adw;
use adw::prelude::*;
use std::rc::Rc;

pub fn build_bundles_view(state: &AppState) -> gtk::Widget {
    let (scrolled, container) = build_scrolled_container();

    let rebuild_page: Rc<dyn Fn()> = {
        let state = state.clone();
        let container = container.clone();

        Rc::new(move || {
            clear_box(&container);

            let page = adw::PreferencesPage::new();
            let bundle_group = adw::PreferencesGroup::builder()
                .title("Installed Feature & Theme Bundles")
                .description(
                    "Expand a bundle to view description and component statuses, or use the menu (⋮) to enable, disable, or remove it",
                )
                .build();

            let bundles = {
                let cfg = state.config.borrow();
                let patcher = state.patcher.borrow();
                BundleService::inspect_bundles(&cfg, &patcher)
            };

            if bundles.is_empty() {
                bundle_group.add(&build_action_row(
                    "No bundles installed yet",
                    "Import a local bundle folder below or install one from Marketplace",
                ));
            } else {
                for b in bundles {
                    bundle_group.add(&build_bundle_row(&state, &b));
                }
            }

            page.add(&bundle_group);
            page.add(&build_import_bundle_group(&state));
            container.append(&page);
        })
    };

    state.register_tab_refresher("bundles", rebuild_page.clone());
    rebuild_page();

    scrolled.upcast()
}
