//! GTK4 + Libadwaita desktop settings application (`genubar-settings`) for managing Genubar configuration.
#![allow(clippy::type_complexity)]

mod components;
mod error;
mod models;
mod services;
mod state;
mod utils;
mod views;
mod window;

use gtk4::prelude::*;
use libadwaita as adw;
use std::env;
use tracing_subscriber::EnvFilter;

fn build_startup_error_window(app: &adw::Application, err_msg: &str) -> adw::ApplicationWindow {
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
    vbox.set_margin_top(24);
    vbox.set_margin_bottom(24);
    vbox.set_margin_start(24);
    vbox.set_margin_end(24);

    let title = gtk4::Label::builder()
        .label("Failed to Initialize Genubar Settings")
        .css_classes(["title-2"])
        .xalign(0.0)
        .build();
    let details = gtk4::Label::builder()
        .label(err_msg)
        .wrap(true)
        .selectable(true)
        .xalign(0.0)
        .build();
    let close_btn = gtk4::Button::builder()
        .label("Close")
        .halign(gtk4::Align::End)
        .build();
    let app_c = app.clone();
    close_btn.connect_clicked(move |_| app_c.quit());

    vbox.append(&title);
    vbox.append(&details);
    vbox.append(&close_btn);

    let root = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    root.append(&adw::HeaderBar::new());
    root.append(&vbox);

    adw::ApplicationWindow::builder()
        .application(app)
        .title("Genubar Settings — Startup Error")
        .default_width(520)
        .default_height(240)
        .content(&root)
        .build()
}

fn main() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();

    let args: Vec<String> = env::args().collect();
    let mut open_recommend: Option<String> = None;
    for i in 0..args.len() {
        if args[i] == "--open-recommend" {
            open_recommend = args.get(i + 1).cloned();
        }
    }

    let app = adw::Application::builder()
        .application_id("org.genubar.Settings")
        .build();

    app.connect_activate(move |app| {
        gtk4::Window::set_default_icon_name(genubar_platform::APP_ICON_NAME);
        let win = match window::build_main_window(app, open_recommend.clone()) {
            Ok(w) => w,
            Err(err) => {
                tracing::error!(error = %err, "Failed to initialize Genubar Settings window");
                build_startup_error_window(app, &err.to_string())
            }
        };
        win.present();
    });

    let empty_args: Vec<String> = Vec::new();
    app.run_with_args(&empty_args);
}
