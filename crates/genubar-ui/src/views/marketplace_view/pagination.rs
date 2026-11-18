use gtk4 as gtk;
use gtk::prelude::*;

pub const PAGE_SIZE: usize = 20;

pub fn build_pagination_controls(
    page_idx: usize,
    total_items: usize,
    on_page: impl Fn(usize) + Clone + 'static,
) -> gtk::Box {
    let total_pages = total_items.div_ceil(PAGE_SIZE).max(1);
    let current = page_idx.min(total_pages - 1);

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    bar.set_halign(gtk::Align::Center);

    let prev_btn = gtk::Button::builder()
        .label("← Previous")
        .sensitive(current > 0)
        .build();
    let info_lbl = gtk::Label::builder()
        .label(format!(
            "Page {} of {} ({} packages)",
            current + 1,
            total_pages,
            total_items
        ))
        .css_classes(["caption", "dim-label"])
        .single_line_mode(true)
        .build();
    let next_btn = gtk::Button::builder()
        .label("Next →")
        .sensitive(current + 1 < total_pages)
        .build();

    {
        let cb = on_page.clone();
        prev_btn.connect_clicked(move |_| cb(current.saturating_sub(1)));
    }
    {
        let cb = on_page;
        next_btn.connect_clicked(move |_| cb(current + 1));
    }

    bar.append(&prev_btn);
    bar.append(&info_lbl);
    bar.append(&next_btn);
    bar
}
