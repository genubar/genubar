use crate::components::dialogs::show_confirm_modal;
use crate::services::config_service::ConfigService;
use crate::state::AppState;
use crate::utils::system::open_directory;
use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub fn build_compact_dropdown(items: &[&str], selected: u32) -> gtk::DropDown {
    let dd = gtk::DropDown::from_strings(items);
    dd.set_valign(gtk::Align::Center);
    dd.set_margin_top(6);
    dd.set_margin_bottom(6);
    dd.set_selected(selected);
    dd
}

pub fn build_toolbar_dropdown(items: &[&str], selected: u32) -> gtk::DropDown {
    let dd = gtk::DropDown::from_strings(items);
    dd.set_valign(gtk::Align::Fill);
    dd.set_selected(selected);
    dd
}

pub fn build_spin_button(val: f64, min: f64, max: f64, step: f64) -> gtk::SpinButton {
    let spin = gtk::SpinButton::with_range(min, max, step);
    spin.set_valign(gtk::Align::Center);
    spin.set_value(val);
    spin
}

pub fn build_priority_arrows(
    can_move_up: bool,
    can_move_down: bool,
    on_up: impl Fn() + 'static,
    on_down: impl Fn() + 'static,
) -> (gtk::Button, gtk::Button) {
    let up_btn = gtk::Button::builder()
        .icon_name("go-up-symbolic")
        .tooltip_text("Move Up (Higher Priority)")
        .sensitive(can_move_up)
        .valign(gtk::Align::Center)
        .build();
    let down_btn = gtk::Button::builder()
        .icon_name("go-down-symbolic")
        .tooltip_text("Move Down (Lower Priority)")
        .sensitive(can_move_down)
        .valign(gtk::Align::Center)
        .build();

    up_btn.connect_clicked(move |_| on_up());
    down_btn.connect_clicked(move |_| on_down());

    (up_btn, down_btn)
}

/// Builds a standard view toolbar with Refresh, Open Folder, Reset (with confirmation modal), and Catalog actions.
pub fn build_unified_top_bar(
    folder_label: &str,
    on_refresh: impl Fn() + 'static,
    on_open_folder: impl Fn() + 'static,
    on_reset: impl Fn() + 'static,
    on_catalog: impl Fn() + 'static,
) -> gtk::Box {
    let top_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    top_bar.set_margin_top(12);
    top_bar.set_margin_bottom(6);
    top_bar.set_margin_start(16);
    top_bar.set_margin_end(16);

    let refresh_btn = gtk::Button::builder()
        .icon_name("view-refresh-symbolic")
        .tooltip_text("Refresh from disk")
        .build();
    let folder_btn = gtk::Button::builder().label(folder_label).build();

    let spacer = gtk::Box::builder().hexpand(true).build();

    let reset_btn = gtk::Button::builder()
        .label("Reset to Defaults")
        .css_classes(["destructive-action"])
        .build();
    let catalog_btn = gtk::Button::builder()
        .label("Find in Catalog")
        .css_classes(["suggested-action"])
        .build();

    refresh_btn.connect_clicked(move |_| on_refresh());
    folder_btn.connect_clicked(move |_| on_open_folder());

    let on_reset_rc: Rc<dyn Fn()> = Rc::new(on_reset);
    reset_btn.connect_clicked(move |btn| {
        let cb = on_reset_rc.clone();
        show_confirm_modal(
            btn,
            "Reset Section to Defaults?",
            "Are you sure you want to restore the default settings for this section? Unsaved custom changes in this section will be overwritten.",
            "Reset to Defaults",
            move || cb(),
        );
    });

    catalog_btn.connect_clicked(move |_| on_catalog());

    top_bar.append(&refresh_btn);
    top_bar.append(&folder_btn);
    top_bar.append(&spacer);
    top_bar.append(&reset_btn);
    top_bar.append(&catalog_btn);

    top_bar
}

/// Shared top bar wrapper for config/patch sections (`Refresh`, `Open Folder`, `Reset`, `Catalog`).
pub fn build_section_top_bar<DirFn, ReloadFn, ResetFn>(
    state: &AppState,
    folder_label: &str,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
    resolve_dir: DirFn,
    on_reload: ReloadFn,
    on_reset: ResetFn,
) -> gtk::Box
where
    DirFn: Fn(&AppState) -> PathBuf + 'static,
    ReloadFn: Fn(&AppState) + 'static,
    ResetFn: Fn(&AppState) + 'static,
{
    let st_ref = state.clone();
    let st_dir = state.clone();
    let st_res = state.clone();
    let rh_res = reload_handle.clone();
    let st_cat = state.clone();

    build_unified_top_bar(
        folder_label,
        move || {
            on_reload(&st_ref);
        },
        move || {
            let dir = resolve_dir(&st_dir);
            open_directory(&dir);
        },
        move || {
            on_reset(&st_res);
            st_res.save_and_reload();
            if let Some(ref cb) = *rh_res.borrow() {
                cb();
            }
        },
        move || st_cat.navigate_to("marketplace"),
    )
}

/// Reloads ConfigManager off the GTK main thread and invokes `reload_handle` on completion.
pub fn spawn_config_reload(
    state: &AppState,
    reload_handle: &Rc<RefCell<Option<Rc<dyn Fn()>>>>,
) {
    let st_in = state.clone();
    let rh_in = reload_handle.clone();
    gtk::glib::spawn_future_local(async move {
        match gtk::gio::spawn_blocking(ConfigService::reload_config).await {
            Ok(Ok(reloaded)) => {
                *st_in.config.borrow_mut() = reloaded;
                if let Some(ref cb) = *rh_in.borrow() {
                    cb();
                }
            }
            Ok(Err(e)) => st_in.notify(&format!("Reload error: {e}")),
            Err(e) => st_in.notify(&format!("Task error: {e:?}")),
        }
    });
}

pub fn build_scrolled_container() -> (gtk::ScrolledWindow, gtk::Box) {
    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    scrolled.set_child(Some(&container));
    (scrolled, container)
}

pub fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

pub fn build_icon_button(icon_name: &str, tooltip: &str, css_classes: &[&str]) -> gtk::Button {
    let btn = gtk::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .build();
    for cls in css_classes {
        btn.add_css_class(cls);
    }
    btn
}

pub fn build_caption_label(text: &str, extra_css_class: Option<&str>) -> gtk::Label {
    let lbl = gtk::Label::builder()
        .label(text)
        .single_line_mode(true)
        .valign(gtk::Align::Center)
        .css_classes(["caption"])
        .build();
    if let Some(cls) = extra_css_class {
        lbl.add_css_class(cls);
    }
    lbl
}

pub fn build_compat_warning_label(text: &str) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .css_classes(["caption", "dim-label"])
        .wrap(true)
        .max_width_chars(34)
        .build()
}

pub fn build_popover_menu_button(
    tooltip: &str,
    margin: i32,
) -> (gtk::MenuButton, gtk::Popover, gtk::Box) {
    let pop_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    pop_box.set_margin_top(margin);
    pop_box.set_margin_bottom(margin);
    pop_box.set_margin_start(margin);
    pop_box.set_margin_end(margin);

    let pop = gtk::Popover::builder().child(&pop_box).build();
    let menu_btn = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .popover(&pop)
        .build();
    (menu_btn, pop, pop_box)
}