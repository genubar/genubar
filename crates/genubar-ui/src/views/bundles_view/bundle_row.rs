use crate::components::controls::{build_icon_button, build_popover_menu_button};
use crate::components::dialogs::show_info_modal;
use crate::components::rows::{build_action_row, build_expander_row};
use crate::models::info_item::InfoField;
use crate::models::package_ui::{package_kind_badge, package_kind_tab_id};
use crate::services::bundle_service::{BundleOpOutcome, BundleService};
use crate::state::AppState;
use adw::prelude::*;
use genubar_packages::BundleDetailedView;
use gtk4 as gtk;
use libadwaita as adw;

fn apply_bundle_outcome(state: &AppState, outcome: BundleOpOutcome) {
    *state.config.borrow_mut() = outcome.config;
    *state.patcher.borrow_mut() = outcome.patcher;
    state.notify(&outcome.message);
    if outcome.needs_reload {
        state.save_and_reload();
    }
    state.rebuild_all_views();
}

fn spawn_bundle_op<F>(state: &AppState, menu_btn: &gtk::MenuButton, op: F)
where
    F: FnOnce(
            genubar_config::ConfigManager,
            genubar_patcher::PatchManager,
        ) -> Result<BundleOpOutcome, crate::error::UiError>
        + Send
        + 'static,
{
    let cfg = state.config.borrow().clone();
    let patcher = state.patcher.borrow().clone();
    menu_btn.set_sensitive(false);

    let st = state.clone();
    let btn = menu_btn.clone();
    gtk::glib::spawn_future_local(async move {
        let res = gtk::gio::spawn_blocking(move || op(cfg, patcher)).await;
        btn.set_sensitive(true);
        match res {
            Ok(Ok(outcome)) => apply_bundle_outcome(&st, outcome),
            Ok(Err(e)) => st.notify(&format!("Bundle operation failed: {e}")),
            Err(e) => st.notify(&format!("Background task error: {e:?}")),
        }
    });
}

pub(super) fn build_bundle_row(state: &AppState, b: &BundleDetailedView) -> adw::ExpanderRow {
    let m = &b.status.manifest;
    let subtitle = if b.status.is_compatible {
        format!(
            "v{} • by {} • {}/{} active ({}/{} patches applied)",
            m.version,
            m.author,
            b.active_components,
            b.components.len(),
            b.applied_patches,
            b.total_patches,
        )
    } else {
        format!(
            "v{} • by {} • Requires Genubar v{} • {}/{} active",
            m.version,
            m.author,
            m.min_app_version.as_deref().unwrap_or("newer"),
            b.active_components,
            b.components.len(),
        )
    };

    let expander = build_expander_row(&m.name, &subtitle, false);

    let info_btn = build_icon_button(
        "dialog-information-symbolic",
        "Bundle Description & Details",
        &[],
    );
    {
        let m_clone = m.clone();
        let total_comps = b.components.len();
        let active_comps = b.active_components;
        let applied_p = b.applied_patches;
        let total_p = b.total_patches;
        info_btn.connect_clicked(move |btn| {
            let fields = vec![
                InfoField::new("Bundle ID", m_clone.id.clone()),
                InfoField::new("Name", m_clone.name.clone()),
                InfoField::new("Version", m_clone.version.clone()),
                InfoField::new("Author", m_clone.author.clone()),
                InfoField::new("Description", m_clone.description.clone()),
                InfoField::new(
                    "Status",
                    format!(
                        "{active_comps}/{total_comps} component(s) active • {applied_p}/{total_p} patch(es) applied"
                    ),
                ),
                InfoField::new(
                    "Min App Version",
                    m_clone
                        .min_app_version
                        .clone()
                        .unwrap_or_else(|| String::from("Any")),
                ),
                InfoField::new("Patches", m_clone.patches.join(", ")),
                InfoField::new("Profiles", m_clone.profiles.join(", ")),
                InfoField::new("Styles", m_clone.styles.join(", ")),
                InfoField::new("Icon Packs", m_clone.icon_packs.join(", ")),
            ];
            show_info_modal(btn, &format!("Bundle: {}", m_clone.name), &fields);
        });
    }

    let (menu_btn, popover, popover_box) = build_popover_menu_button("Bundle Actions", 6);

    let enable_btn = gtk::Button::builder()
        .label(if b.status.is_compatible {
            "Enable All Components & Patches"
        } else {
            "Enable All (Requires Update)"
        })
        .sensitive(b.status.is_compatible)
        .css_classes(["flat"])
        .build();

    let disable_btn = gtk::Button::builder()
        .label("Disable All Components & Patches")
        .css_classes(["flat"])
        .build();

    let remove_btn = gtk::Button::builder()
        .label("Disable & Remove Bundle")
        .css_classes(["flat", "destructive-action"])
        .build();

    popover_box.append(&enable_btn);
    popover_box.append(&disable_btn);
    popover_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    popover_box.append(&remove_btn);

    {
        let st = state.clone();
        let bid = m.id.clone();
        let pop = popover.clone();
        let mb = menu_btn.clone();
        enable_btn.connect_clicked(move |_| {
            pop.popdown();
            let bid_c = bid.clone();
            spawn_bundle_op(&st, &mb, move |cfg, patcher| {
                BundleService::set_bundle_enabled_owned(cfg, patcher, bid_c, true)
            });
        });
    }
    {
        let st = state.clone();
        let bid = m.id.clone();
        let pop = popover.clone();
        let mb = menu_btn.clone();
        disable_btn.connect_clicked(move |_| {
            pop.popdown();
            let bid_c = bid.clone();
            spawn_bundle_op(&st, &mb, move |cfg, patcher| {
                BundleService::set_bundle_enabled_owned(cfg, patcher, bid_c, false)
            });
        });
    }
    {
        let st = state.clone();
        let bid = m.id.clone();
        let pop = popover;
        let mb = menu_btn.clone();
        remove_btn.connect_clicked(move |_| {
            pop.popdown();
            let bid_c = bid.clone();
            spawn_bundle_op(&st, &mb, move |cfg, patcher| {
                BundleService::remove_bundle_owned(cfg, patcher, bid_c)
            });
        });
    }

    let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    actions_box.set_valign(gtk::Align::Center);
    actions_box.append(&info_btn);
    actions_box.append(&menu_btn);
    expander.add_action(&actions_box);

    if !m.description.trim().is_empty() {
        expander.add_row(&build_action_row("About This Bundle", &m.description));
    }

    for comp in &b.components {
        let comp_row = build_action_row(
            &comp.name,
            &format!(
                "{} • {} • {}",
                package_kind_badge(comp.kind),
                comp.status_label,
                comp.file_or_ref
            ),
        );
        let open_tab_btn = gtk::Button::builder()
            .label("Open Tab ↗")
            .valign(gtk::Align::Center)
            .build();
        {
            let st = state.clone();
            let target_tab = package_kind_tab_id(comp.kind);
            open_tab_btn.connect_clicked(move |_| st.navigate_to(target_tab));
        }
        comp_row.add_suffix(&open_tab_btn);
        expander.add_row(&comp_row);
    }

    expander
}
