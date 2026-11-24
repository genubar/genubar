use crate::models::package_ui::RegistryUiCache;
use crate::services::daemon_ipc::DaemonIpcService;
use genubar_config::ConfigManager;
use genubar_patcher::PatchManager;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub config: Rc<RefCell<ConfigManager>>,
    pub patcher: Rc<RefCell<PatchManager>>,
    pub toast_overlay: adw::ToastOverlay,
    pub view_stack: gtk::Stack,
    pub debounce_timer: Rc<RefCell<Option<gtk::glib::SourceId>>>,
    pub status_spinner: gtk::Spinner,
    pub status_icon: gtk::Image,
    pub status_label: gtk::Label,
    pub tab_refreshers: Rc<RefCell<HashMap<String, Rc<dyn Fn()>>>>,
    pub dirty_tabs: Rc<RefCell<HashSet<String>>>,
    pub registry_cache: Rc<RefCell<RegistryUiCache>>,
}

impl AppState {
    pub fn notify(&self, message: &str) {
        let toast = adw::Toast::new(message);
        toast.set_timeout(3);
        self.toast_overlay.add_toast(toast);
    }

    pub fn register_tab_refresher(&self, tab_id: &str, callback: Rc<dyn Fn()>) {
        self.tab_refreshers
            .borrow_mut()
            .insert(tab_id.to_string(), callback);
    }

    /// Marks every registered tab dirty, immediately refreshing only the currently visible tab.
    pub fn rebuild_all_views(&self) {
        let all_ids: Vec<String> = self.tab_refreshers.borrow().keys().cloned().collect();
        {
            let mut dirty = self.dirty_tabs.borrow_mut();
            for id in all_ids {
                dirty.insert(id);
            }
        }
        if let Some(active) = self.view_stack.visible_child_name() {
            self.refresh_tab_if_dirty(active.as_str());
        }
    }

    /// Runs a tab's refresher only when that tab is currently stale.
    pub fn refresh_tab_if_dirty(&self, tab_id: &str) {
        if !self.dirty_tabs.borrow_mut().remove(tab_id) {
            return;
        }
        let cb_opt = self.tab_refreshers.borrow().get(tab_id).cloned();
        if let Some(cb) = cb_opt {
            cb();
        }
    }

    pub fn save_and_reload(&self) {
        if let Some(old_id) = self.debounce_timer.borrow_mut().take() {
            old_id.remove();
        }

        self.status_icon.set_visible(false);
        self.status_spinner.set_visible(true);
        self.status_spinner.start();
        self.status_label.set_text("Saving changes...");

        let st = self.clone();
        let timer_id = gtk::glib::timeout_add_local_once(Duration::from_millis(650), move || {
            *st.debounce_timer.borrow_mut() = None;
            st.execute_async_flush(false);
        });

        *self.debounce_timer.borrow_mut() = Some(timer_id);
    }

    pub fn flush_now(&self, show_toast_on_finish: bool) {
        if let Some(old_id) = self.debounce_timer.borrow_mut().take() {
            old_id.remove();
        }
        self.execute_async_flush(show_toast_on_finish);
    }

    fn execute_async_flush(&self, show_toast_on_finish: bool) {
        self.status_icon.set_visible(false);
        self.status_spinner.set_visible(true);
        self.status_spinner.start();
        self.status_label.set_text("Applying to daemon...");

        if let Err(e) = self.config.borrow().save_settings() {
            self.status_spinner.stop();
            self.status_spinner.set_visible(false);
            self.status_icon
                .set_icon_name(Some("dialog-error-symbolic"));
            self.status_icon.set_visible(true);
            self.status_label.set_text("Save error");
            self.notify(&format!("Failed to save settings: {e}"));
            return;
        }
        if let Err(e) = self.config.borrow().save_icons() {
            tracing::warn!(error = %e, "Failed saving icons.toml during flush");
        }
        self.config.borrow().sync_platform_env();

        let st = self.clone();
        gtk::glib::spawn_future_local(async move {
            let ok = gtk::gio::spawn_blocking(DaemonIpcService::reload_daemon)
                .await
                .unwrap_or(false);

            st.status_spinner.stop();
            st.status_spinner.set_visible(false);
            st.status_icon.set_icon_name(Some("object-select-symbolic"));
            st.status_icon.set_visible(true);
            st.status_label.set_text("All changes applied");

            if show_toast_on_finish {
                if ok {
                    st.notify("Configuration saved and live-reloaded in Genubar");
                } else {
                    st.notify("Configuration saved to disk (daemon not running)");
                }
            }
        });
    }

    pub fn navigate_to(&self, tab_name: &str) {
        self.view_stack.set_visible_child_name(tab_name);
    }
}
