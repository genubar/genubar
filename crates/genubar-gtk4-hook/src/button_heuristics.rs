//! Ignored widget classes and `GtkMenuButton` title classification heuristics.

use crate::ffi::{
    GtkWidget, gtk_button_get_label, gtk_menu_button_get_icon_name, gtk_menu_button_get_label,
    gtk_menu_button_get_popover, gtk_widget_get_tooltip_text, widget_type_name,
};
use std::ffi::CStr;

/// Returns `true` for virtualized canvases or auxiliary popovers that should be skipped during scans.
pub fn is_ignored_widget_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "GtkGridView"
            | "GtkListView"
            | "GtkColumnView"
            | "GtkTreeView"
            | "GtkTextView"
            | "VteTerminal"
            | "WebKitWebView"
            | "NautilusNetworkAddressBar"
            | "NautilusQueryEditor"
            | "NautilusSearchPopover"
            | "NautilusProgressIndicator"
            | "NautilusHistoryControls"
            | "EditorSearchBar"
    )
}

pub(crate) fn classify_primary_icon_name(icon_name: &str) -> Option<&'static str> {
    if icon_name == "open-menu-symbolic" || icon_name == "application-menu-symbolic" {
        Some("Options")
    } else {
        None
    }
}

pub(crate) fn classify_secondary_icon_name(icon_name: &str) -> Option<&'static str> {
    if icon_name == "view-more-symbolic" || icon_name == "view-more-horizontal-symbolic" {
        Some("Options")
    } else if icon_name.contains("view") || icon_name.contains("sort") {
        Some("View")
    } else {
        None
    }
}

pub(crate) fn classify_tooltip_title(tooltip: &str) -> Option<String> {
    let trimmed = tooltip.trim();
    if trimmed.is_empty() {
        return None;
    }
    let low = trimmed.to_lowercase();
    if matches!(
        low.as_str(),
        "main menu"
            | "primary menu"
            | "application menu"
            | "document menu"
            | "secondary menu"
            | "drive options"
            | "menu"
    ) {
        return Some(String::from("Options"));
    }
    if low == "mode selection" {
        return Some(String::from("View"));
    }
    Some(trimmed.to_string())
}

pub(crate) fn classify_popover_class(popover_class: &str) -> Option<&'static str> {
    (popover_class == "BoxesKeysInputPopover").then_some("Send Key")
}

/// Infers a clean top-level column label (`Options`, `View`, `File`, etc.) for a `GtkMenuButton`.
pub unsafe fn infer_button_title(widget: *mut GtkWidget) -> String {
    for lbl_opt in [
        unsafe { gtk_menu_button_get_label(widget) },
        unsafe { gtk_button_get_label(widget) },
    ] {
        if let Some(lbl_ptr) = lbl_opt
            && !lbl_ptr.is_null()
                && let Ok(s) = unsafe { CStr::from_ptr(lbl_ptr).to_str() }
                    && !s.trim().is_empty() {
                        return s.trim().to_string();
                    }
    }

    let icon_name_opt = unsafe { gtk_menu_button_get_icon_name(widget) }.and_then(|icon_ptr| {
        if icon_ptr.is_null() {
            None
        } else {
            unsafe { CStr::from_ptr(icon_ptr).to_str().ok().map(str::to_string) }
        }
    });

    if let Some(ref icon) = icon_name_opt
        && let Some(title) = classify_primary_icon_name(icon) {
            return String::from(title);
        }

    if let Some(tip_ptr) = unsafe { gtk_widget_get_tooltip_text(widget) }
        && !tip_ptr.is_null()
            && let Ok(s) = unsafe { CStr::from_ptr(tip_ptr).to_str() }
                && let Some(title) = classify_tooltip_title(s) {
                    return title;
                }

    if let Some(ref icon) = icon_name_opt
        && let Some(title) = classify_secondary_icon_name(icon) {
            return String::from(title);
        }

    if let Some(popover) = unsafe { gtk_menu_button_get_popover(widget) }
        && !popover.is_null() {
            let pop_class = unsafe { widget_type_name(popover) };
            if let Some(title) = classify_popover_class(&pop_class) {
                return String::from(title);
            }
        }

    String::from("File")
}

pub fn should_hide_original_button() -> bool {
    unsafe {
        let val_ptr = libc::getenv(genubar_protocol::HIDDEN_BUTTONS_ENV_C.as_ptr());
        if val_ptr.is_null() {
            return true;
        }
        let val_str = CStr::from_ptr(val_ptr).to_str().unwrap_or("1");
        val_str != "0" && !val_str.eq_ignore_ascii_case("false")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_title_and_ignored_class_heuristics_classify_expected_inputs() {
        assert!(is_ignored_widget_class("GtkListView"));
        assert!(is_ignored_widget_class("NautilusQueryEditor"));
        assert!(!is_ignored_widget_class("AdwHeaderBar"));

        assert_eq!(classify_primary_icon_name("open-menu-symbolic"), Some("Options"));
        assert_eq!(classify_primary_icon_name("view-grid-symbolic"), None);
        assert_eq!(classify_secondary_icon_name("view-sort-ascending-symbolic"), Some("View"));

        assert_eq!(classify_tooltip_title("Main Menu").as_deref(), Some("Options"));
        assert_eq!(classify_tooltip_title("Mode Selection").as_deref(), Some("View"));
        assert_eq!(classify_tooltip_title("Bookmarks").as_deref(), Some("Bookmarks"));
        assert_eq!(classify_tooltip_title("   "), None);
        assert_eq!(classify_popover_class("BoxesKeysInputPopover"), Some("Send Key"));
    }
}
