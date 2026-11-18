use crate::components::controls::build_compact_dropdown;
use crate::components::dialogs::{show_code_editor_modal, show_confirm_modal};
use crate::components::rows::{build_action_row, build_script_selector_row};
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use genubar_config::{AppTemplateMode, DesktopMenuMode};
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

fn build_template_mode_row<F>(
    title: &str,
    subtitle: &str,
    is_python: bool,
    script_row: &adw::ActionRow,
    on_change: F,
) -> adw::ActionRow
where
    F: Fn(bool) + 'static,
{
    script_row.set_visible(is_python);
    let row = build_action_row(title, subtitle);
    let dd = build_compact_dropdown(
        &["Static TOML", "Python Script"],
        u32::from(is_python),
    );
    let script_row_c = script_row.clone();
    dd.connect_selected_notify(move |drop| {
        let python_selected = drop.selected() == 1;
        script_row_c.set_visible(python_selected);
        on_change(python_selected);
    });
    row.add_suffix(&dd);
    row
}

pub(super) fn build_templates_group(state: &AppState) -> adw::PreferencesGroup {
    let sys_group = adw::PreferencesGroup::builder()
        .title("System & Mandatory Menu Templates")
        .description("Edit mandatory merged columns ({app_name}, Edit, Window, Help) and the Desktop Idle menu")
        .build();

    let app_row = build_action_row(
        "Mandatory Application Template (app_menu.toml)",
        "Defines {app_name}, Edit, Window (Always on Top + WindowList), and Help (SearchBar)",
    );
    let edit_app_btn = gtk::Button::builder()
        .label("Edit app_menu.toml")
        .valign(gtk::Align::Center)
        .build();
    {
        let st = state.clone();
        edit_app_btn.connect_clicked(move |btn| {
            let initial = ConfigService::serialize_app_menu(&st.config.borrow());
            let st_in = st.clone();
            show_code_editor_modal(
                btn,
                "Edit Mandatory Application Menu (app_menu.toml)",
                &initial,
                "Validate & Save app_menu.toml",
                None,
                move |raw| {
                    match ConfigService::validate_and_save_app_menu(
                        &mut st_in.config.borrow_mut(),
                        &raw,
                    ) {
                        Ok(()) => {
                            st_in.save_and_reload();
                            true
                        }
                        Err(e) => {
                            st_in.notify(&e.to_string());
                            false
                        }
                    }
                },
                None,
            );
        });
    }
    app_row.add_suffix(&edit_app_btn);
    sys_group.add(&app_row);

    let cur_app_mode = state.config.borrow().app_menu.mode;
    let cur_app_script = state.config.borrow().app_menu.python_script.clone();
    let app_script_row = build_script_selector_row(
        state,
        "Application Template Python Script",
        "Executes when Application Template Generation Mode is Python Script",
        cur_app_script.as_ref(),
        |st, chosen| {
            st.config.borrow_mut().app_menu.python_script = chosen;
            if let Err(e) = st.config.borrow().save_app_menu() {
                st.notify(&format!("Failed saving app_menu.toml: {e}"));
            } else {
                st.save_and_reload();
            }
        },
    );
    let app_mode_row = build_template_mode_row(
        "Application Template Generation Mode",
        "Use Static TOML columns or transform the final menu via a Python script",
        cur_app_mode == AppTemplateMode::PythonScript,
        &app_script_row,
        {
            let st = state.clone();
            move |is_python| {
                st.config.borrow_mut().app_menu.mode = if is_python {
                    AppTemplateMode::PythonScript
                } else {
                    AppTemplateMode::Static
                };
                if let Err(e) = st.config.borrow().save_app_menu() {
                    st.notify(&format!("Failed saving app_menu.toml: {e}"));
                } else {
                    st.save_and_reload();
                }
            }
        },
    );
    sys_group.add(&app_mode_row);
    sys_group.add(&app_script_row);

    let desk_row = build_action_row(
        "Desktop Idle Menu (desktop_menu.toml)",
        &format!(
            "Displayed when no application window is focused ({} | Places | Tools)",
            genubar_platform::detect_os_name()
        ),
    );
    let edit_desk_btn = gtk::Button::builder()
        .label("Edit desktop_menu.toml")
        .valign(gtk::Align::Center)
        .build();
    {
        let st = state.clone();
        edit_desk_btn.connect_clicked(move |btn| {
            let initial = ConfigService::serialize_desktop_menu(&st.config.borrow());
            let st_in = st.clone();
            show_code_editor_modal(
                btn,
                "Edit Desktop Idle Menu (desktop_menu.toml)",
                &initial,
                "Validate & Save desktop_menu.toml",
                None,
                move |raw| {
                    match ConfigService::validate_and_save_desktop_menu(
                        &mut st_in.config.borrow_mut(),
                        &raw,
                    ) {
                        Ok(()) => {
                            st_in.save_and_reload();
                            true
                        }
                        Err(e) => {
                            st_in.notify(&e.to_string());
                            false
                        }
                    }
                },
                None,
            );
        });
    }
    desk_row.add_suffix(&edit_desk_btn);
    sys_group.add(&desk_row);

    let cur_desk_mode = state.config.borrow().desktop_menu.mode;
    let cur_desk_script = state.config.borrow().desktop_menu.python_script.clone();
    let desk_script_row = build_script_selector_row(
        state,
        "Desktop Idle Python Script",
        "Executes when Desktop Idle Generation Mode is Python Script",
        cur_desk_script.as_ref(),
        |st, chosen| {
            st.config.borrow_mut().desktop_menu.python_script = chosen;
            if let Err(e) = st.config.borrow().save_desktop_menu() {
                st.notify(&format!("Failed saving desktop_menu.toml: {e}"));
            } else {
                st.save_and_reload();
            }
        },
    );
    let desk_mode_row = build_template_mode_row(
        "Desktop Idle Generation Mode",
        "Build idle desktop menu from Static TOML or dynamically via a Python script",
        cur_desk_mode == DesktopMenuMode::PythonScript,
        &desk_script_row,
        {
            let st = state.clone();
            move |is_python| {
                st.config.borrow_mut().desktop_menu.mode = if is_python {
                    DesktopMenuMode::PythonScript
                } else {
                    DesktopMenuMode::Static
                };
                if let Err(e) = st.config.borrow().save_desktop_menu() {
                    st.notify(&format!("Failed saving desktop_menu.toml: {e}"));
                } else {
                    st.save_and_reload();
                }
            }
        },
    );
    sys_group.add(&desk_mode_row);
    sys_group.add(&desk_script_row);

    let reset_row = build_action_row(
        "Restore Default System Menus & Sorter",
        "Resets app_menu.toml, desktop_menu.toml, and root sorting rules to factory defaults",
    );
    let reset_btn = gtk::Button::builder()
        .label("Reset to Defaults")
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    {
        let st = state.clone();
        reset_btn.connect_clicked(move |btn| {
            let st_in = st.clone();
            show_confirm_modal(
                btn,
                "Reset System Menus & Sorter?",
                "Are you sure you want to restore app_menu.toml, desktop_menu.toml, and root sorting rules to factory defaults?",
                "Reset to Defaults",
                move || {
                    if let Err(e) = st_in.config.borrow_mut().reset_system_menus_to_defaults() {
                        st_in.notify(&format!("Reset error: {e}"));
                    }
                    st_in.rebuild_all_views();
                    st_in.flush_now(true);
                },
            );
        });
    }
    reset_row.add_suffix(&reset_btn);
    sys_group.add(&reset_row);

    sys_group
}
