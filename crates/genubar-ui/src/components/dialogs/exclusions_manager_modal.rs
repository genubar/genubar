use crate::components::dialogs::show_text_prompt_modal;
use crate::components::rows::build_action_row;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub fn show_exclusions_manager_modal(
    parent_widget: &impl IsA<gtk::Widget>,
    state: &AppState,
    on_changed: impl Fn() + 'static,
) {
    let win = parent_widget
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = gtk::Window::builder()
        .title("Excluded Applications (Root Sorter)")
        .modal(true)
        .default_width(560)
        .default_height(480)
        .build();
    if let Some(ref w) = win {
        dialog.set_transient_for(Some(w));
    }

    let root_vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    root_vbox.set_margin_top(12);
    root_vbox.set_margin_bottom(12);
    root_vbox.set_margin_start(14);
    root_vbox.set_margin_end(14);

    let search_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let search_entry = gtk::SearchEntry::builder()
        .placeholder_text("Search excluded app_ids or window titles...")
        .hexpand(true)
        .build();
    let sort_btn = gtk::Button::builder()
        .icon_name("view-sort-ascending-symbolic")
        .label("Sort A–Z")
        .build();
    search_bar.append(&search_entry);
    search_bar.append(&sort_btn);
    root_vbox.append(&search_bar);

    let add_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let add_entry = gtk::Entry::builder()
        .placeholder_text("Quick add pattern (e.g. org.gnome.Terminal or *spotify*)...")
        .hexpand(true)
        .build();
    let add_btn = gtk::Button::builder()
        .label("Add Exclusion")
        .css_classes(["suggested-action"])
        .build();
    add_bar.append(&add_entry);
    add_bar.append(&add_btn);
    root_vbox.append(&add_bar);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let list_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    scrolled.set_child(Some(&list_container));
    root_vbox.append(&scrolled);

    dialog.set_child(Some(&root_vbox));

    let on_changed_rc: Rc<dyn Fn()> = Rc::new(on_changed);
    let reload_list: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    let populate: Rc<dyn Fn()> = {
        let state = state.clone();
        let list_container = list_container.clone();
        let search_entry = search_entry.clone();
        let reload_list_inner = reload_list.clone();
        let on_changed_c = on_changed_rc.clone();

        Rc::new(move || {
            while let Some(c) = list_container.first_child() {
                list_container.remove(&c);
            }

            let page = adw::PreferencesPage::new();
            let group = adw::PreferencesGroup::builder()
                .title("Excluded Patterns")
                .description("Matching applications will keep their original tab order")
                .build();

            let q = search_entry.text().trim().to_lowercase();
            let items = state
                .config
                .borrow()
                .settings
                .root_sorting
                .exclude_apps
                .clone();

            if items.is_empty() {
                group.add(&build_action_row(
                    "No excluded applications",
                    "Use the field above to add an app_id or window title pattern",
                ));
            } else {
                for pattern in items {
                    if !q.is_empty() && !pattern.to_lowercase().contains(&q) {
                        continue;
                    }
                    let row = build_action_row(&pattern, "");

                    let edit_btn = gtk::Button::builder()
                        .icon_name("document-edit-symbolic")
                        .tooltip_text("Edit Pattern")
                        .valign(gtk::Align::Center)
                        .build();
                    let del_btn = gtk::Button::builder()
                        .icon_name("user-trash-symbolic")
                        .tooltip_text("Remove Pattern")
                        .valign(gtk::Align::Center)
                        .css_classes(["destructive-action"])
                        .build();

                    {
                        let st = state.clone();
                        let old_pat = pattern.clone();
                        let rl = reload_list_inner.clone();
                        let oc = on_changed_c.clone();
                        edit_btn.connect_clicked(move |btn| {
                            let st_in = st.clone();
                            let old_in = old_pat.clone();
                            let rl_in = rl.clone();
                            let oc_in = oc.clone();
                            show_text_prompt_modal(
                                btn,
                                "Edit Exclusion Pattern",
                                &old_pat,
                                "org.gnome.Terminal",
                                "Save",
                                move |new_val| {
                                    ConfigService::update_exclusion_pattern(
                                        &mut st_in.config.borrow_mut(),
                                        &old_in,
                                        &new_val,
                                    );
                                    st_in.save_and_reload();
                                    oc_in();
                                    if let Some(ref cb) = *rl_in.borrow() {
                                        cb();
                                    }
                                    true
                                },
                            );
                        });
                    }
                    {
                        let st = state.clone();
                        let del_pat = pattern.clone();
                        let rl = reload_list_inner.clone();
                        let oc = on_changed_c.clone();
                        del_btn.connect_clicked(move |_| {
                            ConfigService::remove_exclusion_pattern(
                                &mut st.config.borrow_mut(),
                                &del_pat,
                            );
                            st.save_and_reload();
                            oc();
                            if let Some(ref cb) = *rl.borrow() {
                                cb();
                            }
                        });
                    }

                    row.add_suffix(&edit_btn);
                    row.add_suffix(&del_btn);
                    group.add(&row);
                }
            }

            page.add(&group);
            list_container.append(&page);
        })
    };

    *reload_list.borrow_mut() = Some(populate.clone());
    populate();

    {
        let pop = populate.clone();
        search_entry.connect_search_changed(move |_| pop());
    }
    {
        let st = state.clone();
        let pop = populate.clone();
        let oc = on_changed_rc.clone();
        sort_btn.connect_clicked(move |_| {
            ConfigService::sort_exclusion_patterns(&mut st.config.borrow_mut());
            st.save_and_reload();
            oc();
            pop();
        });
    }

    let do_add: Rc<dyn Fn()> = {
        let st = state.clone();
        let ae = add_entry.clone();
        let pop = populate.clone();
        let oc = on_changed_rc;
        Rc::new(move || {
            let added =
                ConfigService::add_exclusion_pattern(&mut st.config.borrow_mut(), &ae.text());
            if added {
                ae.set_text("");
                st.save_and_reload();
                oc();
                pop();
            } else if !ae.text().trim().is_empty() {
                ae.set_text("");
            }
        })
    };

    {
        let da = do_add.clone();
        add_btn.connect_clicked(move |_| da());
    }
    {
        let da = do_add;
        add_entry.connect_activate(move |_| da());
    }

    dialog.present();
}