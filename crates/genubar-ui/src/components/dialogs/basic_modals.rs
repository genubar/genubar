use crate::components::rows::build_action_row;
use crate::models::info_item::InfoField;
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

pub fn show_info_modal(
    parent_widget: &impl IsA<gtk::Widget>,
    window_title: &str,
    fields: &[InfoField],
) {
    let win = parent_widget
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = gtk::Window::builder()
        .title(window_title)
        .modal(true)
        .default_width(540)
        .default_height(420)
        .build();
    if let Some(ref w) = win {
        dialog.set_transient_for(Some(w));
    }

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();

    let page = adw::PreferencesPage::new();
    let group = adw::PreferencesGroup::builder()
        .title(window_title)
        .build();

    for item in fields {
        group.add(&build_action_row(&item.label, &item.value));
    }
    page.add(&group);
    scrolled.set_child(Some(&page));

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
    vbox.append(&scrolled);

    let close_btn = gtk::Button::builder()
        .label("Close")
        .margin_top(6)
        .margin_bottom(12)
        .margin_start(16)
        .margin_end(16)
        .build();
    let dlg_c = dialog.clone();
    close_btn.connect_clicked(move |_| dlg_c.close());
    vbox.append(&close_btn);

    dialog.set_child(Some(&vbox));
    dialog.present();
}

pub fn show_text_prompt_modal(
    parent_widget: &impl IsA<gtk::Widget>,
    window_title: &str,
    initial_text: &str,
    placeholder: &str,
    button_label: &str,
    on_submit: impl Fn(String) -> bool + 'static,
) {
    let win = parent_widget
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = gtk::Window::builder()
        .title(window_title)
        .modal(true)
        .default_width(480)
        .default_height(150)
        .build();
    if let Some(ref w) = win {
        dialog.set_transient_for(Some(w));
    }

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
    vbox.set_margin_top(16);
    vbox.set_margin_bottom(16);
    vbox.set_margin_start(16);
    vbox.set_margin_end(16);

    let entry = gtk::Entry::builder()
        .text(initial_text)
        .placeholder_text(placeholder)
        .hexpand(true)
        .build();
    let submit_btn = gtk::Button::builder()
        .label(button_label)
        .css_classes(["suggested-action"])
        .build();

    vbox.append(&entry);
    vbox.append(&submit_btn);
    dialog.set_child(Some(&vbox));

    let dlg_c = dialog.clone();
    submit_btn.connect_clicked(move |_| {
        let val = entry.text().trim().to_string();
        if !val.is_empty() && on_submit(val) {
            dlg_c.close();
        }
    });

    dialog.present();
}