use crate::state::AppState;
use crate::utils::text::escape_markup;
use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

pub fn build_action_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let safe_title = escape_markup(title);
    let safe_sub = escape_markup(subtitle);
    let mut builder = adw::ActionRow::builder().title(safe_title);
    if !safe_sub.is_empty() {
        builder = builder.subtitle(safe_sub);
    }
    builder.build()
}

pub fn build_expander_row(title: &str, subtitle: &str, expanded: bool) -> adw::ExpanderRow {
    let safe_title = escape_markup(title);
    let safe_sub = escape_markup(subtitle);
    let mut builder = adw::ExpanderRow::builder()
        .title(safe_title)
        .expanded(expanded);
    if !safe_sub.is_empty() {
        builder = builder.subtitle(safe_sub);
    }
    builder.build()
}

pub fn build_two_row_entry(
    title: &str,
    subtitle: &str,
    initial_text: &str,
    placeholder: &str,
    btn_label: &str,
) -> (gtk::ListBoxRow, gtk::Entry, gtk::Button) {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 6);
    outer.set_margin_top(10);
    outer.set_margin_bottom(10);
    outer.set_margin_start(14);
    outer.set_margin_end(14);

    let title_lbl = gtk::Label::new(Some(title));
    title_lbl.set_halign(gtk::Align::Start);
    title_lbl.set_xalign(0.0);
    title_lbl.add_css_class("heading");
    outer.append(&title_lbl);

    if !subtitle.is_empty() {
        let sub_lbl = gtk::Label::new(Some(subtitle));
        sub_lbl.set_halign(gtk::Align::Start);
        sub_lbl.set_xalign(0.0);
        sub_lbl.set_wrap(true);
        sub_lbl.add_css_class("dim-label");
        outer.append(&sub_lbl);
    }

    let input_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let entry = gtk::Entry::builder()
        .text(initial_text)
        .placeholder_text(placeholder)
        .hexpand(true)
        .build();
    let apply_btn = gtk::Button::builder()
        .label(btn_label)
        .valign(gtk::Align::Center)
        .css_classes(["suggested-action"])
        .build();

    input_row.append(&entry);
    input_row.append(&apply_btn);
    outer.append(&input_row);

    let list_row = gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .child(&outer)
        .build();

    (list_row, entry, apply_btn)
}

pub fn build_switch_row(
    state: &AppState,
    title: &str,
    subtitle: &str,
    active: bool,
    on_toggle: impl Fn(&AppState, bool) + 'static,
) -> adw::ActionRow {
    let row = build_action_row(title, subtitle);
    let sw = gtk::Switch::builder()
        .active(active)
        .valign(gtk::Align::Center)
        .build();
    let st = state.clone();
    sw.connect_state_set(move |_, is_active| {
        on_toggle(&st, is_active);
        st.save_and_reload();
        gtk::glib::Propagation::Proceed
    });
    row.add_suffix(&sw);
    row
}

#[allow(clippy::too_many_arguments)]
pub fn build_spin_row(
    state: &AppState,
    title: &str,
    subtitle: &str,
    val: f64,
    min: f64,
    max: f64,
    step: f64,
    on_change: impl Fn(&AppState, i32) + 'static,
) -> adw::ActionRow {
    let row = build_action_row(title, subtitle);
    let spin = gtk::SpinButton::with_range(min, max, step);
    spin.set_valign(gtk::Align::Center);
    spin.set_value(val);
    let st = state.clone();
    spin.connect_value_changed(move |sp| {
        on_change(&st, sp.value_as_int());
        st.save_and_reload();
    });
    row.add_suffix(&spin);
    row
}
