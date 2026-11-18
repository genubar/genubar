mod layout_group;
mod packs_group;

use crate::components::controls::{
    build_scrolled_container, build_section_top_bar, clear_box, spawn_config_reload,
};
use crate::state::AppState;
use gtk4 as gtk;
use gtk::prelude::*;
use layout_group::build_icons_layout_group;
use libadwaita as adw;
use adw::prelude::*;
use packs_group::build_icon_packs_group;
use std::cell::RefCell;
use std::rc::Rc;

pub fn build_icons_view(state: &AppState) -> gtk::Widget {
    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let reload_handle: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let pack_query_state = Rc::new(RefCell::new(String::new()));

    let top_bar = build_section_top_bar(
        state,
        "Open Icons Folder",
        &reload_handle,
        |st| st.config.borrow().base_dir().join("icons"),
        {
            let rh = reload_handle.clone();
            move |st| spawn_config_reload(st, &rh)
        },
        |st| {
            if let Err(e) = st.config.borrow_mut().reset_icons_to_defaults() {
                st.notify(&format!("Reset error: {e}"));
            }
        },
    );
    root_box.append(&top_bar);

    let (scrolled, container) = build_scrolled_container();
    root_box.append(&scrolled);

    let rebuild_ui: Rc<dyn Fn()> = {
        let state = state.clone();
        let container = container.clone();
        let reload_handle_inner = reload_handle.clone();
        let pack_query_state = pack_query_state.clone();

        Rc::new(move || {
            clear_box(&container);

            let page = adw::PreferencesPage::new();
            page.add(&build_icons_layout_group(&state));
            page.add(&build_icon_packs_group(
                &state,
                &pack_query_state,
                &reload_handle_inner,
            ));
            container.append(&page);
        })
    };

    *reload_handle.borrow_mut() = Some(rebuild_ui.clone());
    state.register_tab_refresher("icons", rebuild_ui.clone());
    rebuild_ui();

    root_box.upcast()
}
