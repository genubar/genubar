//! Safe GVariant unboxing and detailed-action-name parsing helpers for `Gtk4Bridge`.

use crate::ffi::{
    GVariant, g_action_parse_detailed_name, g_free, g_variant_classify, g_variant_get_child_value,
    g_variant_get_string, g_variant_get_variant, g_variant_n_children, g_variant_ref_sink,
    g_variant_unref,
};
use libc::{c_char, c_int, c_void};
use std::ffi::{CStr, CString};
use std::ptr;

const G_VARIANT_CLASS_VARIANT: c_int = b'v' as c_int;

/// Owned arguments extracted from a `Gtk4Bridge.Activate(action, params)` call.
pub(crate) struct ActivateCallArgs {
    child0: *mut GVariant,
    child1: *mut GVariant,
    pub(crate) action_name: Option<String>,
    pub(crate) target_param: *mut GVariant,
}

impl Drop for ActivateCallArgs {
    fn drop(&mut self) {
        unsafe {
            if !self.target_param.is_null() {
                let _ = g_variant_unref(self.target_param);
            }
            if !self.child1.is_null() {
                let _ = g_variant_unref(self.child1);
            }
            if !self.child0.is_null() {
                let _ = g_variant_unref(self.child0);
            }
        }
    }
}

/// Unboxes nested `v`-type `GVariant` wrappers and sinks a floating result if non-null.
pub(crate) unsafe fn unbox_variant_recursively(mut var: *mut GVariant) -> *mut GVariant {
    while !var.is_null() {
        let class_code = unsafe { g_variant_classify(var) }.unwrap_or(0);
        if class_code != G_VARIANT_CLASS_VARIANT {
            break;
        }
        let inner = unsafe { g_variant_get_variant(var) }.unwrap_or(ptr::null_mut());
        unsafe {
            let _ = g_variant_unref(var);
        }
        var = inner;
    }
    if !var.is_null() {
        let _ = unsafe { g_variant_ref_sink(var) };
    }
    var
}

/// Extracts `(action_name, optional_unboxed_target)` from an `Activate` method tuple.
pub(crate) unsafe fn extract_activate_args(parameters: *mut GVariant) -> Option<ActivateCallArgs> {
    let child0 = unsafe { g_variant_get_child_value(parameters, 0) }?;
    let Some(child1) = (unsafe { g_variant_get_child_value(parameters, 1) }) else {
        unsafe {
            let _ = g_variant_unref(child0);
        }
        return None;
    };

    let action_ptr =
        unsafe { g_variant_get_string(child0, ptr::null_mut()) }.unwrap_or(ptr::null());
    let mut target_param: *mut GVariant = ptr::null_mut();

    if unsafe { g_variant_n_children(child1) }.unwrap_or(0) > 0
        && let Some(boxed) = unsafe { g_variant_get_child_value(child1, 0) } {
            target_param = unsafe { unbox_variant_recursively(boxed) };
        }

    let action_name = (!action_ptr.is_null())
        .then(|| unsafe { CStr::from_ptr(action_ptr).to_str().unwrap_or("") }.to_string());

    Some(ActivateCallArgs {
        child0,
        child1,
        action_name,
        target_param,
    })
}

/// Parsed candidate action name + optional target parameter with RAII GLib cleanup.
pub(crate) struct DetailedActionName {
    candidate_cstr: CString,
    parsed_name_ptr: *mut c_char,
    parsed_target: *mut GVariant,
    fallback_target: *mut GVariant,
}

impl DetailedActionName {
    /// Parses `candidate` into `(name, target)` using `g_action_parse_detailed_name` when needed.
    pub(crate) unsafe fn parse(candidate: &str, target_param: *mut GVariant) -> Option<Self> {
        let candidate_cstr = CString::new(candidate).ok()?;
        let mut parsed_name_ptr: *mut c_char = ptr::null_mut();
        let mut parsed_target: *mut GVariant = ptr::null_mut();

        if target_param.is_null() && candidate.contains('(') {
            let ok = unsafe {
                g_action_parse_detailed_name(
                    candidate_cstr.as_ptr(),
                    &mut parsed_name_ptr,
                    &mut parsed_target,
                    ptr::null_mut(),
                )
            }
            .unwrap_or(0);

            if ok != 0 && !parsed_name_ptr.is_null() {
                if !parsed_target.is_null() {
                    let _ = unsafe { g_variant_ref_sink(parsed_target) };
                }
            } else {
                // Defensive cleanup in case GLib populated an output on failure.
                if !parsed_target.is_null() {
                    unsafe {
                        let _ = g_variant_unref(parsed_target);
                    }
                    parsed_target = ptr::null_mut();
                }
                if !parsed_name_ptr.is_null() {
                    unsafe {
                        let _ = g_free(parsed_name_ptr as *mut c_void);
                    }
                    parsed_name_ptr = ptr::null_mut();
                }
            }
        }

        Some(Self {
            candidate_cstr,
            parsed_name_ptr,
            parsed_target,
            fallback_target: target_param,
        })
    }

    pub(crate) fn name_ptr(&self) -> *const c_char {
        if !self.parsed_name_ptr.is_null() {
            self.parsed_name_ptr as *const c_char
        } else {
            self.candidate_cstr.as_ptr()
        }
    }

    pub(crate) fn target_ptr(&self) -> *mut GVariant {
        if !self.parsed_name_ptr.is_null() {
            self.parsed_target
        } else {
            self.fallback_target
        }
    }

    pub(crate) unsafe fn full_name(&self) -> &str {
        unsafe { CStr::from_ptr(self.name_ptr()).to_str().unwrap_or("") }
    }
}

impl Drop for DetailedActionName {
    fn drop(&mut self) {
        unsafe {
            if !self.parsed_target.is_null() {
                let _ = g_variant_unref(self.parsed_target);
            }
            if !self.parsed_name_ptr.is_null() {
                let _ = g_free(self.parsed_name_ptr as *mut c_void);
            }
        }
    }
}
