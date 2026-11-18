//! Active `GtkWindow` / `GApplication` tracking and per-window zoom percentages.

use super::widget_tracking::track_widget_lifetime;
use crate::ffi::{
    GApplication, GtkWidget, gtk_application_get_active_window, gtk_widget_get_mapped,
    gtk_widget_get_visible,
};
use std::collections::{HashMap, HashSet};
use std::ptr;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicPtr, Ordering},
};

static ACTIVE_GTK_WINDOW: AtomicPtr<GtkWidget> = AtomicPtr::new(ptr::null_mut());
static ACTIVE_G_APP: AtomicPtr<GApplication> = AtomicPtr::new(ptr::null_mut());

static WINDOW_ZOOM_PCT: OnceLock<Mutex<HashMap<usize, i32>>> = OnceLock::new();
static KNOWN_MENU_WINDOWS: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

fn window_zoom_map() -> &'static Mutex<HashMap<usize, i32>> {
    WINDOW_ZOOM_PCT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn menu_windows_set() -> &'static Mutex<HashSet<usize>> {
    KNOWN_MENU_WINDOWS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn set_active_context(window: *mut GtkWidget, app: *mut GApplication) {
    ACTIVE_GTK_WINDOW.store(window, Ordering::SeqCst);
    ACTIVE_G_APP.store(app, Ordering::SeqCst);
}

pub fn record_menu_window(window: *mut GtkWidget) {
    if !window.is_null() {
        track_widget_lifetime(window);
        if let Ok(mut guard) = menu_windows_set().lock() {
            guard.insert(window as usize);
        }
    }
}

pub fn remove_window_state(win_addr: usize, window_ptr: *mut GtkWidget) {
    if let Ok(mut guard) = window_zoom_map().lock() {
        guard.remove(&win_addr);
    }

    let mut fallback_win: *mut GtkWidget = ptr::null_mut();
    if let Ok(mut guard) = menu_windows_set().lock() {
        guard.remove(&win_addr);
        for &cand_addr in guard.iter() {
            let cand = cand_addr as *mut GtkWidget;
            let is_mapped = unsafe { gtk_widget_get_mapped(cand) }.unwrap_or(0) != 0;
            if is_mapped {
                fallback_win = cand;
                break;
            }
        }
    }

    if !window_ptr.is_null() {
        let _ = ACTIVE_GTK_WINDOW.compare_exchange(
            window_ptr,
            fallback_win,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
    }
}

pub fn active_window() -> *mut GtkWidget {
    let app = ACTIVE_G_APP.load(Ordering::SeqCst);
    let known_set = menu_windows_set()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();

    if !app.is_null()
        && let Some(active_win) = unsafe { gtk_application_get_active_window(app) }
            && !active_win.is_null() && known_set.contains(&(active_win as usize)) {
                let is_vis = unsafe { gtk_widget_get_visible(active_win) }.unwrap_or(1) != 0;
                let is_map = unsafe { gtk_widget_get_mapped(active_win) }.unwrap_or(1) != 0;
                if is_vis && is_map {
                    ACTIVE_GTK_WINDOW.store(active_win, Ordering::SeqCst);
                    return active_win;
                }
            }

    let cur = ACTIVE_GTK_WINDOW.load(Ordering::SeqCst);
    if !cur.is_null() && (known_set.is_empty() || known_set.contains(&(cur as usize))) {
        let is_map = unsafe { gtk_widget_get_mapped(cur) }.unwrap_or(1) != 0;
        if is_map {
            return cur;
        }
    }

    for &cand_addr in &known_set {
        let cand = cand_addr as *mut GtkWidget;
        let is_map = unsafe { gtk_widget_get_mapped(cand) }.unwrap_or(0) != 0;
        if is_map {
            ACTIVE_GTK_WINDOW.store(cand, Ordering::SeqCst);
            return cand;
        }
    }

    cur
}

pub fn active_app() -> *mut GApplication {
    ACTIVE_G_APP.load(Ordering::SeqCst)
}

pub fn zoom_percent() -> i32 {
    let win_addr = active_window() as usize;
    zoom_percent_for_addr(win_addr)
}

pub(crate) fn zoom_percent_for_addr(win_addr: usize) -> i32 {
    window_zoom_map()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&win_addr).copied())
        .unwrap_or(100)
}

pub fn set_zoom_percent(pct: i32) {
    set_zoom_percent_for_addr(active_window() as usize, pct);
}

pub(crate) fn set_zoom_percent_for_addr(win_addr: usize, pct: i32) {
    if win_addr == 0 {
        return;
    }
    if let Ok(mut guard) = window_zoom_map().lock() {
        guard.insert(win_addr, pct.clamp(20, 500));
    }
}

pub fn adjust_zoom(delta: i32) {
    adjust_zoom_for_addr(active_window() as usize, delta);
}

pub(crate) fn adjust_zoom_for_addr(win_addr: usize, delta: i32) {
    if win_addr == 0 {
        return;
    }
    if let Ok(mut guard) = window_zoom_map().lock() {
        if delta == 0 {
            guard.insert(win_addr, 100);
        } else {
            let prev = guard.get(&win_addr).copied().unwrap_or(100);
            guard.insert(win_addr, (prev + delta).clamp(20, 500));
        }
    }
}
