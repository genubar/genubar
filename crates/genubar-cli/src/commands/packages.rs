use crate::commands::notify_daemon_reload_config;
use genubar_dbus::unified::UnifiedMenuService;
use genubar_protocol::{TRACKER_BUS_NAME, TRACKER_INTERFACE, TRACKER_OBJECT_PATH};
use std::error::Error;

pub async fn simulate_focus(
    service: &UnifiedMenuService,
    pid: u32,
    xid: u32,
    title: String,
) -> Result<(), Box<dyn Error>> {
    service
        .connection()
        .call_method(
            Some(TRACKER_BUS_NAME),
            TRACKER_OBJECT_PATH,
            Some(TRACKER_INTERFACE),
            "NotifyFocusChanged",
            &(pid, xid, title, "simulated.app", "", "", "", "", ""),
        )
        .await?;
    println!("Sent focus simulation (PID={pid}, XID={xid}) to daemon.");
    Ok(())
}

pub async fn reload_config(service: &UnifiedMenuService) -> Result<(), Box<dyn Error>> {
    service
        .connection()
        .call_method(
            Some(TRACKER_BUS_NAME),
            TRACKER_OBJECT_PATH,
            Some(TRACKER_INTERFACE),
            "ReloadConfig",
            &(),
        )
        .await?;
    println!("Configuration reloaded successfully.");
    Ok(())
}

pub async fn capture_snapshot(service: &UnifiedMenuService) -> Result<(), Box<dyn Error>> {
    let reply = service
        .connection()
        .call_method(
            Some(TRACKER_BUS_NAME),
            TRACKER_OBJECT_PATH,
            Some(TRACKER_INTERFACE),
            "CaptureActiveSnapshot",
            &(),
        )
        .await?;
    let path: String = reply.body().deserialize()?;
    println!("Saved active window menu snapshot to: {path}");
    Ok(())
}

pub async fn profile_from_snapshot(
    service: &UnifiedMenuService,
    snapshot: String,
    sync_new: bool,
) -> Result<(), Box<dyn Error>> {
    let reply = service
        .connection()
        .call_method(
            Some(TRACKER_BUS_NAME),
            TRACKER_OBJECT_PATH,
            Some(TRACKER_INTERFACE),
            "CreateProfileFromSnapshot",
            &(snapshot, sync_new),
        )
        .await?;
    let profile_path: String = reply.body().deserialize()?;
    println!("Generated profile from snapshot at: {profile_path}");
    Ok(())
}

pub fn list_patches() -> Result<(), Box<dyn Error>> {
    let mgr = genubar_patcher::PatchManager::load_or_init()?;
    for card in mgr.list_patch_cards() {
        let badge = if card.status == genubar_patcher::PackageStatus::Applied {
            "[APPLIED]"
        } else if card.is_recommended {
            "[RECOMMENDED]"
        } else {
            "[AVAILABLE]"
        };
        println!(
            "{badge:<15} {} (v{}) - {}\n                tags={:?} bundles={:?} origin={:?}",
            card.manifest.meta.id,
            card.manifest.meta.version,
            card.manifest.meta.name,
            card.manifest.meta.tags,
            card.manifest.meta.part_of_bundles,
            card.manifest.meta.origin,
        );
    }
    Ok(())
}

pub fn apply_patch(id: String, skip_unsupported: bool) -> Result<(), Box<dyn Error>> {
    let mut mgr = genubar_patcher::PatchManager::load_or_init()?;
    let hint = mgr.apply_patch_ext(
        &id,
        genubar_patcher::ConflictStrategy::Overwrite,
        genubar_core::PatchErrorPolicy::Strict,
        skip_unsupported,
    )?;
    println!("Successfully applied patch '{id}'.");
    if let Some(h) = hint {
        println!("Note: {h}");
    }
    Ok(())
}

pub fn revert_patch(id: String, force: bool) -> Result<(), Box<dyn Error>> {
    let mut mgr = genubar_patcher::PatchManager::load_or_init()?;
    let warnings = mgr.revert_patch_ext(&id, force)?;
    println!("Successfully reverted patch '{id}'.");
    for w in warnings {
        println!("Warning during unsafe cleanup: {w}");
    }
    Ok(())
}

pub fn list_bundles() -> Result<(), Box<dyn Error>> {
    let cfg = genubar_config::ConfigManager::load_or_bootstrap()?;
    let patch_mgr = genubar_patcher::PatchManager::load_or_init()?;
    for b in genubar_packages::BundleOrchestrator::inspect_bundles(&cfg, &patch_mgr) {
        let m = &b.status.manifest;
        println!(
            "[BUNDLE] {} (v{}) - {}\n         active={}/{} components ({}/{} installed), patches={}/{} applied\n         {}",
            m.id,
            m.version,
            m.name,
            b.active_components,
            b.components.len(),
            b.status.installed_components,
            b.status.total_components,
            b.applied_patches,
            b.total_patches,
            m.description
        );
    }
    Ok(())
}

pub async fn set_bundle_enabled(id: String, enabled: bool) -> Result<(), Box<dyn Error>> {
    let outcome = {
        let mut cfg = genubar_config::ConfigManager::load_or_bootstrap()?;
        let mut patch_mgr = genubar_patcher::PatchManager::load_or_init()?;
        genubar_packages::BundleOrchestrator::set_bundle_enabled(
            &mut cfg,
            &mut patch_mgr,
            &id,
            enabled,
        )?
    };

    if enabled {
        if outcome.changed_patches.is_empty() {
            println!("Bundle '{id}' enabled (no pending patches to apply).");
        } else {
            for pid in outcome.changed_patches {
                println!("  -> Applied '{pid}'.");
            }
            for h in outcome.post_hints {
                println!("  -> Note: {h}");
            }
        }
    } else {
        for pid in outcome.changed_patches {
            println!("  -> Reverted '{pid}'.");
        }
    }

    if let Ok(service) = UnifiedMenuService::new().await {
        notify_daemon_reload_config(&service).await;
    } else {
        tracing::debug!("Session D-Bus unavailable; skipped daemon reload notification");
    }
    Ok(())
}
