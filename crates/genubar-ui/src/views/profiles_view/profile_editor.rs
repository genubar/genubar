use crate::components::controls::build_compact_dropdown;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use genubar_config::AppProfile;
use gtk::prelude::*;
use gtk4 as gtk;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub fn open_profile_dual_editor(
    parent_btn: &gtk::Button,
    state: &AppState,
    profile: AppProfile,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) {
    let win = parent_btn
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = gtk::Window::builder()
        .title(format!("Edit Profile: {}", profile.name))
        .modal(true)
        .default_width(720)
        .default_height(580)
        .build();
    if let Some(ref w) = win {
        dialog.set_transient_for(Some(w));
    }

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let grid = gtk::Grid::builder()
        .column_spacing(12)
        .row_spacing(8)
        .build();

    let name_entry = gtk::Entry::builder()
        .text(&profile.name)
        .hexpand(true)
        .build();
    let author_entry = gtk::Entry::builder()
        .text(&profile.author)
        .hexpand(true)
        .build();

    let mut script_opts = vec![String::from("None (Disabled)")];
    script_opts.extend(ConfigService::list_valid_scripts(&state.config.borrow()));
    let cur_script_str = profile
        .python_script
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let sel_script_idx = if cur_script_str.is_empty() {
        0
    } else {
        script_opts
            .iter()
            .position(|s| *s == cur_script_str)
            .unwrap_or_else(|| {
                script_opts.push(cur_script_str.clone());
                script_opts.len() - 1
            })
    } as u32;
    let opt_refs: Vec<&str> = script_opts.iter().map(String::as_str).collect();
    let script_drop = build_compact_dropdown(&opt_refs, sel_script_idx);

    let sync_chk = gtk::CheckButton::with_label("Sync new live items (sync_new_items)");
    sync_chk.set_active(profile.sync_new_items);
    let sys_col_chk = gtk::CheckButton::with_label("Include {app_name} system column");
    sys_col_chk.set_active(profile.include_app_system_column);
    let no_sort_chk = gtk::CheckButton::with_label("Exclude from Root Sorter");
    no_sort_chk.set_active(profile.disable_root_sort);

    grid.attach(&gtk::Label::new(Some("Display Name:")), 0, 0, 1, 1);
    grid.attach(&name_entry, 1, 0, 1, 1);
    grid.attach(&gtk::Label::new(Some("Author:")), 2, 0, 1, 1);
    grid.attach(&author_entry, 3, 0, 1, 1);
    grid.attach(&gtk::Label::new(Some("Python Script:")), 0, 1, 1, 1);
    grid.attach(&script_drop, 1, 1, 3, 1);
    grid.attach(&sync_chk, 0, 2, 2, 1);
    grid.attach(&sys_col_chk, 2, 2, 2, 1);
    grid.attach(&no_sort_chk, 0, 3, 2, 1);
    vbox.append(&grid);

    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hexpand(true)
        .build();
    let text_view = gtk::TextView::builder().monospace(true).build();
    let initial_toml = toml::to_string_pretty(&profile).unwrap_or_default();
    text_view.buffer().set_text(&initial_toml);
    scroll.set_child(Some(&text_view));
    vbox.append(&scroll);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let save_btn = gtk::Button::builder()
        .label("Validate & Save Profile")
        .css_classes(["suggested-action"])
        .hexpand(true)
        .build();
    let del_btn = gtk::Button::builder()
        .label("Delete Profile")
        .css_classes(["destructive-action"])
        .build();
    btn_box.append(&save_btn);
    btn_box.append(&del_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    {
        let st = state.clone();
        let dlg_c = dialog.clone();
        let tv = text_view;
        let rh = reload_handle.clone();
        let orig_file_path = profile.file_path.clone();
        save_btn.connect_clicked(move |_| {
            let buf = tv.buffer();
            let raw = buf
                .text(&buf.start_iter(), &buf.end_iter(), false)
                .to_string();
            match toml::from_str::<AppProfile>(&raw) {
                Ok(mut parsed) => {
                    parsed.file_path = orig_file_path.clone();
                    parsed.name = name_entry.text().to_string();
                    parsed.author = author_entry.text().to_string();
                    parsed.sync_new_items = sync_chk.is_active();
                    parsed.include_app_system_column = sys_col_chk.is_active();
                    parsed.disable_root_sort = no_sort_chk.is_active();
                    let s_idx = script_drop.selected() as usize;
                    parsed.python_script = if s_idx == 0 {
                        None
                    } else {
                        script_opts.get(s_idx).map(PathBuf::from)
                    };

                    if let Err(e) = st.config.borrow_mut().save_profile(parsed) {
                        st.notify(&format!("Save error: {e}"));
                    } else {
                        st.save_and_reload();
                        if let Some(ref cb) = *rh.borrow() {
                            cb();
                        }
                        dlg_c.close();
                    }
                }
                Err(e) => st.notify(&format!("TOML syntax error: {e}")),
            }
        });
    }
    {
        let st = state.clone();
        let dlg_c = dialog.clone();
        let pid = profile.id;
        let rh = reload_handle.clone();
        del_btn.connect_clicked(move |_| {
            let _ = st.config.borrow_mut().delete_profile(&pid);
            st.save_and_reload();
            if let Some(ref cb) = *rh.borrow() {
                cb();
            }
            dlg_c.close();
        });
    }

    dialog.present();
}
