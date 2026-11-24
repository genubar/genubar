use crate::components::dialogs::show_confirm_modal;
use gtk::prelude::*;
use gtk4 as gtk;
use std::rc::Rc;

pub fn show_code_editor_modal(
    parent_widget: &impl IsA<gtk::Widget>,
    window_title: &str,
    initial_code: &str,
    save_label: &str,
    secondary_btn_label: Option<&str>,
    on_save: impl Fn(String) -> bool + 'static,
    on_secondary: Option<Box<dyn Fn() + 'static>>,
) {
    let win = parent_widget
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = gtk::Window::builder()
        .title(window_title)
        .modal(true)
        .default_width(740)
        .default_height(560)
        .build();
    if let Some(ref w) = win {
        dialog.set_transient_for(Some(w));
    }

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_top(12);
    vbox.set_margin_bottom(12);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
    let tv = gtk::TextView::builder().monospace(true).build();
    tv.buffer().set_text(initial_code);
    scroll.set_child(Some(&tv));
    vbox.append(&scroll);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let save_btn = gtk::Button::builder()
        .label(save_label)
        .css_classes(["suggested-action"])
        .hexpand(true)
        .build();
    btn_box.append(&save_btn);

    if let (Some(sec_lbl), Some(sec_cb)) = (secondary_btn_label, on_secondary) {
        let sec_btn = gtk::Button::builder()
            .label(sec_lbl)
            .css_classes(["destructive-action"])
            .build();
        let dlg_sec = dialog.clone();
        let sec_cb_rc: Rc<dyn Fn()> = Rc::from(sec_cb);
        let lbl_owned = sec_lbl.to_string();
        sec_btn.connect_clicked(move |btn| {
            let cb = sec_cb_rc.clone();
            let d = dlg_sec.clone();
            show_confirm_modal(
                btn,
                &format!("{lbl_owned}?"),
                "Are you sure you want to discard current custom edits and switch back?",
                &lbl_owned,
                move || {
                    cb();
                    d.close();
                },
            );
        });
        btn_box.append(&sec_btn);
    }

    vbox.append(&btn_box);
    dialog.set_child(Some(&vbox));

    let dlg_c = dialog.clone();
    save_btn.connect_clicked(move |_| {
        let buf = tv.buffer();
        let raw = buf
            .text(&buf.start_iter(), &buf.end_iter(), false)
            .to_string();
        if on_save(raw) {
            dlg_c.close();
        }
    });

    dialog.present();
}
