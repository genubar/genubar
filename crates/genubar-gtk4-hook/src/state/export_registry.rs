//! GDBus export IDs, primary sidebar model retention, and signature deduplication.

use crate::ffi::{GtkWidget, g_object_ref, g_object_unref};
use libc::c_void;
use std::collections::HashMap;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU32, AtomicU64, Ordering},
};

type ExportSignature = (usize, Vec<(String, i32, usize)>, u64);
type WindowPrimaryModelsMap = HashMap<usize, Vec<(String, usize)>>;

static EXPORTED_MENU_ID: AtomicU32 = AtomicU32::new(0);
static EXPORTED_APP_ACT_ID: AtomicU32 = AtomicU32::new(0);
static EXPORTED_WIN_ACT_ID: AtomicU32 = AtomicU32::new(0);
static EXPORTED_BRIDGE_ID: AtomicU32 = AtomicU32::new(0);
static ACTION_STATE_EPOCH: AtomicU64 = AtomicU64::new(1);

static WINDOW_PRIMARY_MODELS: OnceLock<Mutex<WindowPrimaryModelsMap>> = OnceLock::new();
static LAST_EXPORTED_SIGNATURE: OnceLock<Mutex<ExportSignature>> = OnceLock::new();

pub fn action_state_epoch() -> u64 {
    ACTION_STATE_EPOCH.load(Ordering::SeqCst)
}

pub fn bump_action_state_epoch() -> u64 {
    ACTION_STATE_EPOCH.fetch_add(1, Ordering::SeqCst) + 1
}

fn window_primary_models_map() -> &'static Mutex<HashMap<usize, Vec<(String, usize)>>> {
    WINDOW_PRIMARY_MODELS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn last_signature_lock() -> &'static Mutex<ExportSignature> {
    LAST_EXPORTED_SIGNATURE.get_or_init(|| Mutex::new((0, Vec::new(), 0)))
}

pub fn swap_menu_export_id(new_id: u32) -> u32 {
    EXPORTED_MENU_ID.swap(new_id, Ordering::SeqCst)
}

pub fn swap_app_action_export_id(new_id: u32) -> u32 {
    EXPORTED_APP_ACT_ID.swap(new_id, Ordering::SeqCst)
}

pub fn swap_win_action_export_id(new_id: u32) -> u32 {
    EXPORTED_WIN_ACT_ID.swap(new_id, Ordering::SeqCst)
}

pub fn bridge_export_id() -> u32 {
    EXPORTED_BRIDGE_ID.load(Ordering::SeqCst)
}

pub fn set_bridge_export_id(id: u32) {
    EXPORTED_BRIDGE_ID.store(id, Ordering::SeqCst);
}

/// Records a primary `GMenuModel` reference for `window` so headerbar state survives rebuilds.
///
/// # Safety
/// `window` and `model` must either be null or valid non-dangling GObject pointers on the GTK thread.
pub unsafe fn record_window_primary_model(window: *mut GtkWidget, title: &str, model: *mut c_void) {
    if window.is_null() || model.is_null() {
        return;
    }
    let win_addr = window as usize;
    let model_addr = model as usize;
    if let Ok(mut guard) = window_primary_models_map().lock() {
        let list = guard.entry(win_addr).or_default();
        if !list.iter().any(|(_, m)| *m == model_addr) {
            unsafe {
                let _ = g_object_ref(model);
            }
            list.push((title.to_string(), model_addr));
        }
    }
}

pub fn get_window_primary_models(window: *mut GtkWidget) -> Vec<(String, *mut c_void)> {
    if window.is_null() {
        return Vec::new();
    }
    window_primary_models_map()
        .lock()
        .ok()
        .and_then(|guard| {
            guard.get(&(window as usize)).map(|list| {
                list.iter()
                    .map(|(t, addr)| (t.clone(), *addr as *mut c_void))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Drops retained primary `GMenuModel` references for a destroyed window outside the mutex lock.
pub fn release_window_primary_models(win_addr: usize) {
    let removed = window_primary_models_map()
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(&win_addr));

    if let Some(entries) = removed {
        for (_, model_addr) in entries {
            let model_ptr = model_addr as *mut c_void;
            if !model_ptr.is_null() {
                unsafe {
                    let _ = g_object_unref(model_ptr);
                }
            }
        }
    }
}

pub fn check_and_update_export_signature(
    window: *mut GtkWidget,
    sig: &[(String, i32, usize)],
) -> bool {
    let win_addr = window as usize;
    let epoch = action_state_epoch();
    if let Ok(mut guard) = last_signature_lock().lock() {
        if guard.0 == win_addr && guard.1.as_slice() == sig && guard.2 == epoch {
            return false;
        }
        guard.0 = win_addr;
        guard.1 = sig.to_vec();
        guard.2 = epoch;
        return true;
    }
    true
}

pub fn invalidate_export_signature() {
    if let Ok(mut guard) = last_signature_lock().lock() {
        guard.0 = 0;
        guard.1.clear();
        guard.2 = 0;
    }
}
