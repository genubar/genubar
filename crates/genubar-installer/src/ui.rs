use crate::error::InstallerError;
use crate::operations::{
    InstallerStatusSnapshot, inspect_status_snapshot, install_extension_only,
    perform_install_or_update, perform_uninstall,
};
use genubar_core::{CURRENT_APP_VERSION, EXPECTED_EXTENSION_VERSION, is_version_compatible};
use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use std::rc::Rc;

#[derive(Clone)]
struct InstallerUiRefs {
    status_label: gtk::Label,
    install_btn: gtk::Button,
    update_btn: gtk::Button,
    ext_only_btn: gtk::Button,
    uninstall_btn: gtk::Button,
}

impl InstallerUiRefs {
    fn set_busy(&self, busy_message: &str) {
        self.status_label.set_text(busy_message);
        self.install_btn.set_sensitive(false);
        self.update_btn.set_sensitive(false);
        self.ext_only_btn.set_sensitive(false);
        self.uninstall_btn.set_sensitive(false);
    }

    fn apply_snapshot(&self, snap: &InstallerStatusSnapshot, header_msg: Option<&str>) {
        let inst = snap.daemon_version.is_some();
        let upd = snap
            .daemon_version
            .as_deref()
            .map(|v| !is_version_compatible(CURRENT_APP_VERSION, v))
            .unwrap_or(false);
        let ok = snap.ext_compat_ok;
        let reinstall = inst && !upd;

        if reinstall {
            self.install_btn
                .set_label(&format!("Reinstall Genubar v{CURRENT_APP_VERSION}"));
            self.install_btn.remove_css_class("suggested-action");
        } else {
            self.install_btn
                .set_label(&format!("Install Genubar v{CURRENT_APP_VERSION}"));
            if !self.install_btn.has_css_class("suggested-action") {
                self.install_btn.add_css_class("suggested-action");
            }
        }

        self.install_btn.set_sensitive((!inst || reinstall) && ok);
        self.update_btn.set_sensitive(upd && ok);
        self.ext_only_btn.set_sensitive(ok);
        self.uninstall_btn.set_sensitive(inst || snap.ext_installed);

        let full_text = match header_msg {
            Some(msg) => format!("{msg}\n\n{}", snap.summary_text),
            None => snap.summary_text.clone(),
        };
        self.status_label.set_text(&full_text);
    }
}

fn spawn_background_action<F>(
    ui: &InstallerUiRefs,
    busy_text: &'static str,
    error_prefix: &'static str,
    action: F,
) where
    F: FnOnce() -> Result<String, InstallerError> + Send + 'static,
{
    ui.set_busy(busy_text);
    let ui_c = ui.clone();
    gtk::glib::MainContext::default().spawn_local(async move {
        let outcome = gtk::gio::spawn_blocking(move || {
            let res = action();
            let snap = inspect_status_snapshot();
            (res, snap)
        })
        .await;

        match outcome {
            Ok((Ok(msg), snap)) => ui_c.apply_snapshot(&snap, Some(&msg)),
            Ok((Err(err), snap)) => {
                let err_msg = format!("{error_prefix}: {err}");
                ui_c.apply_snapshot(&snap, Some(&err_msg));
            }
            Err(join_err) => {
                let err = InstallerError::Task(format!("{join_err:?}"));
                ui_c.status_label.set_text(&format!("{error_prefix}: {err}"));
            }
        }
    });
}

fn wire_async_action<F>(
    button: &gtk::Button,
    ui: &InstallerUiRefs,
    busy_text: &'static str,
    error_prefix: &'static str,
    make_action: F,
) where
    F: Fn() -> Box<dyn FnOnce() -> Result<String, InstallerError> + Send> + 'static,
{
    let ui_c = ui.clone();
    let maker = Rc::new(make_action);
    button.connect_clicked(move |_| {
        let work = maker();
        spawn_background_action(&ui_c, busy_text, error_prefix, work);
    });
}

pub fn run_gui() {
    let app = adw::Application::builder()
        .application_id("org.genubar.Installer")
        .build();

    app.connect_activate(|app| {
        gtk::Window::set_default_icon_name(genubar_platform::INSTALLER_ICON_NAME);

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
        vbox.set_margin_top(20);
        vbox.set_margin_bottom(20);
        vbox.set_margin_start(24);
        vbox.set_margin_end(24);

        let brand_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let brand_icon = if let Ok(pb) = gtk::gdk::gdk_pixbuf::Pixbuf::from_read(
            std::io::Cursor::new(genubar_platform::EMBEDDED_APP_ICON_SVG.as_bytes()),
        ) {
            gtk::Image::from_paintable(Some(&gtk::gdk::Texture::for_pixbuf(&pb)))
        } else {
            gtk::Image::from_icon_name(genubar_platform::INSTALLER_ICON_NAME)
        };
        brand_icon.set_pixel_size(48);
        let brand_title = gtk::Label::builder()
            .label("Genubar Universal Global Menubar")
            .css_classes(["title-2"])
            .xalign(0.0)
            .build();
        brand_row.append(&brand_icon);
        brand_row.append(&brand_title);
        vbox.append(&brand_row);

        let status_label = gtk::Label::new(Some("Checking current installation status..."));
        status_label.set_wrap(true);
        status_label.set_xalign(0.0);
        vbox.append(&status_label);

        let install_btn = gtk::Button::builder()
            .label(format!("Install Genubar v{CURRENT_APP_VERSION}"))
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();

        let update_btn = gtk::Button::builder()
            .label(format!("Update to v{CURRENT_APP_VERSION}"))
            .sensitive(false)
            .css_classes(["suggested-action"])
            .build();

        let ext_only_btn = gtk::Button::builder()
            .label(format!(
                "Install / Update GNOME Extension Only (v{EXPECTED_EXTENSION_VERSION})"
            ))
            .sensitive(false)
            .build();

        let purge_chk = gtk::CheckButton::with_label(
            "Also remove user profiles, settings, and caches (~/.config/genubar, ~/.cache/genubar); active patches are always reverted",
        );

        let uninstall_btn = gtk::Button::builder()
            .label("Uninstall Genubar")
            .sensitive(false)
            .css_classes(["destructive-action"])
            .build();

        vbox.append(&install_btn);
        vbox.append(&update_btn);
        vbox.append(&ext_only_btn);
        vbox.append(&purge_chk);
        vbox.append(&uninstall_btn);

        let ui = InstallerUiRefs {
            status_label,
            install_btn: install_btn.clone(),
            update_btn: update_btn.clone(),
            ext_only_btn: ext_only_btn.clone(),
            uninstall_btn: uninstall_btn.clone(),
        };

        // Initial probe runs asynchronously so the window presents immediately.
        {
            let ui_init = ui.clone();
            gtk::glib::MainContext::default().spawn_local(async move {
                if let Ok(snap) = gtk::gio::spawn_blocking(inspect_status_snapshot).await {
                    ui_init.apply_snapshot(&snap, None);
                }
            });
        }

        wire_async_action(
            &install_btn,
            &ui,
            "Installing Genubar binaries, hook, and GNOME extension...",
            "Install failed",
            || Box::new(perform_install_or_update),
        );
        wire_async_action(
            &update_btn,
            &ui,
            "Updating Genubar binaries, hook, and GNOME extension...",
            "Update failed",
            || Box::new(perform_install_or_update),
        );
        wire_async_action(
            &ext_only_btn,
            &ui,
            "Installing embedded GNOME Shell extension...",
            "Extension install failed",
            || {
                Box::new(|| {
                    install_extension_only().map(|ver| {
                        format!(
                            "Installed GNOME Shell extension v{ver}. Log out and back in to reload GNOME Shell."
                        )
                    })
                })
            },
        );
        {
            let pc = purge_chk.clone();
            wire_async_action(
                &uninstall_btn,
                &ui,
                "Uninstalling Genubar...",
                "Uninstall failed",
                move || {
                    let purge = pc.is_active();
                    Box::new(move || perform_uninstall(purge))
                },
            );
        }

        let header = adw::HeaderBar::new();
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&header);
        root.append(&vbox);

        let win = adw::ApplicationWindow::builder()
            .application(app)
            .title("Genubar Installer & Updater")
            .default_width(540)
            .default_height(380)
            .content(&root)
            .build();
        win.present();
    });

    app.run();
}
