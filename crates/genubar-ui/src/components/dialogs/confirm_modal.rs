use gtk4 as gtk;
use gtk::prelude::*;

/// Displays a modal confirmation dialog for destructive resets and bulk disable/revert actions.
pub fn show_confirm_modal(
    parent_widget: &impl IsA<gtk::Widget>,
    window_title: &str,
    message: &str,
    confirm_label: &str,
    on_confirm: impl Fn() + 'static,
) {
    let win = parent_widget
        .root()
        .and_then(|r| r.downcast::<gtk::Window>().ok());
    let dialog = gtk::Window::builder()
        .title(window_title)
        .modal(true)
        .default_width(440)
        .default_height(175)
        .resizable(false)
        .build();
    if let Some(ref w) = win {
        dialog.set_transient_for(Some(w));
    }

    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 16);
    vbox.set_margin_top(20);
    vbox.set_margin_bottom(20);
    vbox.set_margin_start(20);
    vbox.set_margin_end(20);

    let msg_label = gtk::Label::new(Some(message));
    msg_label.set_wrap(true);
    msg_label.set_xalign(0.0);
    msg_label.set_vexpand(true);
    vbox.append(&msg_label);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    btn_box.set_halign(gtk::Align::End);

    let cancel_btn = gtk::Button::builder().label("Cancel").build();
    let confirm_btn = gtk::Button::builder()
        .label(confirm_label)
        .css_classes(["destructive-action"])
        .build();

    btn_box.append(&cancel_btn);
    btn_box.append(&confirm_btn);
    vbox.append(&btn_box);

    dialog.set_child(Some(&vbox));

    {
        let dlg_c = dialog.clone();
        cancel_btn.connect_clicked(move |_| dlg_c.close());
    }
    {
        let dlg_c = dialog.clone();
        confirm_btn.connect_clicked(move |_| {
            on_confirm();
            dlg_c.close();
        });
    }

    dialog.present();
}