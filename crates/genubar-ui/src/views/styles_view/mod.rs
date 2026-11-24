mod metrics_group;
mod theme_group;

use crate::components::controls::{
    build_scrolled_container, build_section_top_bar, clear_box, spawn_config_reload,
};
use crate::state::AppState;
use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use metrics_group::build_metrics_group;
use std::cell::RefCell;
use std::rc::Rc;
use theme_group::build_theme_group;

pub fn build_styles_view(state: &AppState) -> gtk::Widget {
    let root_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let reload_handle: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let top_bar = build_section_top_bar(
        state,
        "Open Styles Folder",
        &reload_handle,
        |st| st.config.borrow().styles.styles_dir().to_path_buf(),
        {
            let rh = reload_handle.clone();
            move |st| spawn_config_reload(st, &rh)
        },
        |st| {
            if let Err(e) = st.config.borrow_mut().reset_styles_to_defaults() {
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

        Rc::new(move || {
            clear_box(&container);

            let page = adw::PreferencesPage::new();
            page.add(&build_theme_group(&state, &reload_handle_inner));
            page.add(&build_metrics_group(&state));
            container.append(&page);
        })
    };

    *reload_handle.borrow_mut() = Some(rebuild_ui.clone());
    state.register_tab_refresher("styles", rebuild_ui.clone());
    rebuild_ui();

    root_box.upcast()
}
