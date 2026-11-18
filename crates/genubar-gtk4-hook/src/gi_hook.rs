//! GObject Introspection (`g_module_symbol`) redirection and real symbol pointer caching.

use crate::ffi::load_next_sym;
use crate::hooks::{
    g_application_run, g_module_symbol, gtk_application_add_window,
    gtk_menu_button_set_menu_model, gtk_menu_button_set_popover, gtk_popover_bind_model,
    gtk_popover_new_from_model, gtk_widget_action_set_enabled, gtk_widget_insert_action_group,
    gtk_window_present, gtk_window_set_application,
};
use libc::{c_char, c_int, c_void};
use std::ffi::CStr;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

pub static REAL_G_APPLICATION_RUN: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_WINDOW_PRESENT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_WINDOW_SET_APP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_APP_ADD_WINDOW: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_MENU_BTN_SET_MODEL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_MENU_BTN_SET_POPOVER: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_POPOVER_NEW_FROM_MODEL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_POPOVER_BIND_MODEL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_WIDGET_INSERT_ACT_GRP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
pub static REAL_GTK_WIDGET_ACTION_SET_ENABLED: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

static REAL_G_MODULE_SYMBOL: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

pub unsafe fn resolve_real_sym(name: &[u8], cached: &AtomicPtr<c_void>) -> *mut c_void {
    let ptr = unsafe { load_next_sym(name) };
    if !ptr.is_null() {
        return ptr;
    }
    cached.load(Ordering::SeqCst)
}

/// Promotes already-loaded GTK/GIO shared libraries into `RTLD_GLOBAL` scope so
/// `dlsym(RTLD_DEFAULT, ...)` works even when `PyGObject` or `GJS` opened them via `RTLD_LOCAL`.
pub unsafe fn ensure_gtk_symbols_global() {
    for soname in [
        c"libgtk-4.so.1".as_ptr(),
        c"libgtk-3.so.0".as_ptr(),
        c"libadwaita-1.so.0".as_ptr(),
        c"libhandy-1.so.0".as_ptr(),
        c"libgio-2.0.so.0".as_ptr(),
        c"libgobject-2.0.so.0".as_ptr(),
        c"libglib-2.0.so.0".as_ptr(),
    ] {
        unsafe {
            let _ = libc::dlopen(
                soname,
                libc::RTLD_LAZY | libc::RTLD_NOLOAD | libc::RTLD_GLOBAL,
            );
        }
    }
}

unsafe fn get_real_g_module_symbol() -> *mut c_void {
    let cached = REAL_G_MODULE_SYMBOL.load(Ordering::SeqCst);
    if !cached.is_null() {
        return cached;
    }
    let mut ptr = unsafe { load_next_sym(b"g_module_symbol\0") };
    if ptr.is_null() {
        let handle = unsafe {
            libc::dlopen(
                c"libgmodule-2.0.so.0".as_ptr(),
                libc::RTLD_LAZY | libc::RTLD_GLOBAL,
            )
        };
        if !handle.is_null() {
            let sym = unsafe { libc::dlsym(handle, c"g_module_symbol".as_ptr()) };
            if !sym.is_null() && sym != g_module_symbol as *mut c_void {
                ptr = sym;
            }
        }
    }
    if !ptr.is_null() {
        REAL_G_MODULE_SYMBOL.store(ptr, Ordering::SeqCst);
    }
    ptr
}

pub unsafe fn intercept_g_module_symbol(
    module: *mut c_void,
    symbol_name: *const c_char,
    symbol: *mut *mut c_void,
) -> c_int {
    type FnGModuleSymbol =
        unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> c_int;

    let real_fn_ptr = unsafe { get_real_g_module_symbol() };
    if real_fn_ptr.is_null() {
        return 0;
    }
    let real_fn: FnGModuleSymbol = unsafe { std::mem::transmute(real_fn_ptr) };
    let ok = unsafe { real_fn(module, symbol_name, symbol) };
    if ok == 0 || symbol_name.is_null() || symbol.is_null() {
        return ok;
    }

    let orig_sym = unsafe { *symbol };
    if orig_sym.is_null() {
        return ok;
    }

    if let Ok(name) = unsafe { CStr::from_ptr(symbol_name).to_str() } {
        let replacement: Option<(*mut c_void, &AtomicPtr<c_void>)> = match name {
            "g_application_run" => Some((g_application_run as *mut c_void, &REAL_G_APPLICATION_RUN)),
            "gtk_window_present" => {
                Some((gtk_window_present as *mut c_void, &REAL_GTK_WINDOW_PRESENT))
            }
            "gtk_window_set_application" => Some((
                gtk_window_set_application as *mut c_void,
                &REAL_GTK_WINDOW_SET_APP,
            )),
            "gtk_application_add_window" => Some((
                gtk_application_add_window as *mut c_void,
                &REAL_GTK_APP_ADD_WINDOW,
            )),
            "gtk_menu_button_set_menu_model" => Some((
                gtk_menu_button_set_menu_model as *mut c_void,
                &REAL_GTK_MENU_BTN_SET_MODEL,
            )),
            "gtk_menu_button_set_popover" => Some((
                gtk_menu_button_set_popover as *mut c_void,
                &REAL_GTK_MENU_BTN_SET_POPOVER,
            )),
            "gtk_popover_new_from_model" => Some((
                gtk_popover_new_from_model as *mut c_void,
                &REAL_GTK_POPOVER_NEW_FROM_MODEL,
            )),
            "gtk_popover_bind_model" => Some((
                gtk_popover_bind_model as *mut c_void,
                &REAL_GTK_POPOVER_BIND_MODEL,
            )),
            "gtk_widget_insert_action_group" => Some((
                gtk_widget_insert_action_group as *mut c_void,
                &REAL_GTK_WIDGET_INSERT_ACT_GRP,
            )),
            "gtk_widget_action_set_enabled" => Some((
                gtk_widget_action_set_enabled as *mut c_void,
                &REAL_GTK_WIDGET_ACTION_SET_ENABLED,
            )),
            _ => None,
        };

        if let Some((hook_fn, cache_slot)) = replacement {
            if orig_sym != hook_fn {
                cache_slot.store(orig_sym, Ordering::SeqCst);
            }
            unsafe { *symbol = hook_fn };
        }
    }

    ok
}
