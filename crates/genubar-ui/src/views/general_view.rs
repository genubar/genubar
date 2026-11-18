use crate::components::controls::build_spin_button;
use crate::components::rows::{build_action_row, build_two_row_entry};
use crate::state::AppState;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub fn build_general_view(state: &AppState) -> gtk::Widget {
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let page = adw::PreferencesPage::new();

    // 1. Top Panel Overflow ("More") & Core Behavior (bound to exact GeneralConfig fields)
    let bar_group = adw::PreferencesGroup::builder()
        .title("Menubar Layout & Overflow")
        .description("Controls how top-level menu columns fit on the GNOME top panel")
        .build();

    // Overflow toggle (`GeneralConfig::enable_more_overflow`)
    let overflow_row = build_action_row(
        "Enable 'More' Column Overflow",
        "Groups extra top-level tabs into a single overflow dropdown when column limit is exceeded",
    );
    let overflow_sw = gtk::Switch::builder()
        .active(state.config.borrow().settings.general.enable_more_overflow)
        .valign(gtk::Align::Center)
        .build();
    {
        let st = state.clone();
        overflow_sw.connect_state_set(move |_, active| {
            st.config.borrow_mut().settings.general.enable_more_overflow = active;
            st.save_and_reload();
            gtk::glib::Propagation::Proceed
        });
    }
    overflow_row.add_suffix(&overflow_sw);
    bar_group.add(&overflow_row);

    // Column cap (`GeneralConfig::max_top_columns`)
    let max_col_row = build_action_row(
        "Maximum Top-Level Columns",
        "Number of visible top-level columns before remaining tabs move into the overflow menu",
    );
    let max_spin = build_spin_button(
        state.config.borrow().settings.general.max_top_columns as f64,
        2.0,
        25.0,
        1.0,
    );
    {
        let st = state.clone();
        max_spin.connect_value_changed(move |sp| {
            st.config.borrow_mut().settings.general.max_top_columns = sp.value_as_int() as usize;
            st.save_and_reload();
        });
    }
    max_col_row.add_suffix(&max_spin);
    bar_group.add(&max_col_row);

    let (more_row, more_entry, more_btn) = build_two_row_entry(
        "Overflow Column Label",
        "Title of the dropdown menu that holds overflow tabs",
        &state.config.borrow().settings.general.more_column_label,
        "More",
        "Apply",
    );
    {
        let st = state.clone();
        let me = more_entry.clone();
        more_btn.connect_clicked(move |_| {
            let txt = me.text().trim().to_string();
            if !txt.is_empty() {
                st.config.borrow_mut().settings.general.more_column_label = txt;
                st.save_and_reload();
            }
        });
    }
    bar_group.add(&more_row);
    page.add(&bar_group);

    // 2. Python Scripting Engine Settings (bound to `python_binary` and `script_timeout_ms`)
    let eng_group = adw::PreferencesGroup::builder()
        .title("Python Scripting Engine")
        .description("Configure the interpreter and execution timeout for # ==GenubarScript== plugins")
        .build();

    // Interpreter binary path (`GeneralConfig::python_binary`)
    let (py_row, py_entry, py_btn) = build_two_row_entry(
        "Python Interpreter Binary",
        "Executable name or full path to a virtual environment python3 binary",
        &state.config.borrow().settings.general.python_binary,
        "python3",
        "Apply",
    );
    {
        let st = state.clone();
        let pe = py_entry.clone();
        py_btn.connect_clicked(move |_| {
            let txt = pe.text().trim().to_string();
            if !txt.is_empty() {
                st.config.borrow_mut().settings.general.python_binary = txt;
                st.save_and_reload();
            }
        });
    }
    eng_group.add(&py_row);

    let timeout_row = build_action_row(
        "Script Execution Timeout (ms)",
        "Maximum time allowed for a custom Python menu script before falling back",
    );
    let timeout_spin = build_spin_button(
        state.config.borrow().settings.general.script_timeout_ms as f64,
        100.0,
        5000.0,
        50.0,
    );
    {
        let st = state.clone();
        timeout_spin.connect_value_changed(move |sp| {
            st.config.borrow_mut().settings.general.script_timeout_ms = sp.value_as_int() as u64;
            st.save_and_reload();
        });
    }
    timeout_row.add_suffix(&timeout_spin);
    eng_group.add(&timeout_row);
    page.add(&eng_group);

    // 3. Factory Reset Actions (General settings reset or full configuration reset)
    let reset_group = adw::PreferencesGroup::builder()
        .title("Reset Configuration")
        .description("Restore either General settings or overwrite all Genubar configuration files to factory state")
        .build();

    let reset_gen_row = build_action_row(
        "Restore Default General Settings",
        "Resets only general settings in settings.toml back to factory defaults",
    );
    let reset_gen_btn = gtk::Button::builder()
        .label("Reset General")
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    {
        let st = state.clone();
        reset_gen_btn.connect_clicked(move |btn| {
            let st_in = st.clone();
            crate::components::dialogs::show_confirm_modal(
                btn,
                "Reset General Settings?",
                "Are you sure you want to restore General settings to their factory defaults?",
                "Reset General",
                move || {
                    let _ = st_in.config.borrow_mut().reset_settings_to_defaults();
                    st_in.rebuild_all_views();
                    st_in.flush_now(true);
                },
            );
        });
    }
    reset_gen_row.add_suffix(&reset_gen_btn);
    reset_group.add(&reset_gen_row);

    let reset_all_row = build_action_row(
        "Factory Reset All Configurations",
        "Overwrites settings.toml, icons.toml, app_menu.toml, desktop_menu.toml, style.css, and default profiles",
    );
    let reset_all_btn = gtk::Button::builder()
        .label("Factory Reset All")
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    {
        let st = state.clone();
        reset_all_btn.connect_clicked(move |btn| {
            let st_in = st.clone();
            crate::components::dialogs::show_confirm_modal(
                btn,
                "Factory Reset All Configurations?",
                "This will overwrite all Genubar configuration files, styles, and default profiles back to factory state. Are you sure?",
                "Factory Reset All",
                move || {
                    if st_in
                        .config
                        .borrow_mut()
                        .reset_all_to_factory_defaults()
                        .is_ok()
                    {
                        st_in.rebuild_all_views();
                        st_in.flush_now(true);
                        st_in.notify("All configurations restored to factory defaults");
                    }
                },
            );
        });
    }
    reset_all_row.add_suffix(&reset_all_btn);
    reset_group.add(&reset_all_row);
    page.add(&reset_group);

    scrolled.set_child(Some(&page));
    scrolled.upcast()
}