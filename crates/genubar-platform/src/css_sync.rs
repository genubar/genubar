use crate::fs_utils::{
    copy_file, read_text_file, replace_with_atomic_copy_or_symlink, write_text_file,
};
use crate::paths::resolve_user_path;
use genubar_core::MenuError;
use std::path::Path;

const METRICS_START_MARKER: &str = "/* GENUBAR_METRICS_START */";
const METRICS_END_MARKER: &str = "/* GENUBAR_METRICS_END */";
const JS_MAX_SAFE_INT_MASK: u64 = (1u64 << 53) - 1;

/// Computes a deterministic, non-zero, JS-safe (`<= 2^53 - 1`) FNV-1a content fingerprint for CSS bytes.
pub fn css_content_fingerprint(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    let safe = hash & JS_MAX_SAFE_INT_MASK;
    if safe == 0 { 1 } else { safe }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssSyncModeKind {
    SymlinkToExtension,
    CopyToExtension,
    DirectExtensionFile,
}

#[derive(Debug, Clone, Copy)]
pub struct CssMetrics {
    pub panel_button_hpadding_px: u32,
    pub popup_border_radius_px: u32,
    pub max_column_height_px: u32,
    pub min_column_width_px: u32,
    pub item_vpadding_px: u32,
    pub item_hpadding_px: u32,
    pub item_border_radius_px: u32,
    pub font_size_px: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct CssSyncOptions<'a> {
    pub sync_mode: CssSyncModeKind,
    pub active_style: &'a str,
    pub custom_css_path: &'a Path,
    pub extension_css_path: &'a Path,
    pub apply_metrics_to_css: bool,
    pub metrics: CssMetrics,
}

pub struct CssSynchronizer;

impl CssSynchronizer {
    pub fn sync(base_dir: &Path, opts: &CssSyncOptions<'_>) {
        if let Err(err) = Self::try_sync(base_dir, opts) {
            tracing::warn!(error = %err, "Failed syncing GNOME Shell stylesheet");
        }
    }

    pub fn try_sync(base_dir: &Path, opts: &CssSyncOptions<'_>) -> Result<(), MenuError> {
        let preset_path = base_dir.join("styles").join(opts.active_style);
        let compiled_src = resolve_user_path(base_dir, opts.custom_css_path);
        let dst = resolve_user_path(base_dir, opts.extension_css_path);

        let base_css = if preset_path.is_file() {
            read_text_file(&preset_path)?
        } else if compiled_src.is_file() {
            read_text_file(&compiled_src)?
        } else {
            String::new()
        };

        if !base_css.is_empty() {
            let final_css = if opts.apply_metrics_to_css {
                Self::inject_metrics_block(&base_css, &opts.metrics)
            } else {
                base_css
            };
            write_text_file(&compiled_src, &final_css)?;
        }

        match opts.sync_mode {
            CssSyncModeKind::SymlinkToExtension => {
                replace_with_atomic_copy_or_symlink(&compiled_src, &dst)?
            }
            CssSyncModeKind::CopyToExtension => copy_file(&compiled_src, &dst)?,
            CssSyncModeKind::DirectExtensionFile => {}
        }

        Ok(())
    }

    /// Computes a stable content fingerprint of the active extension stylesheet (falling back
    /// to the compiled custom stylesheet if the extension target file is absent).
    pub fn fingerprint(base_dir: &Path, opts: &CssSyncOptions<'_>) -> u64 {
        let dst = resolve_user_path(base_dir, opts.extension_css_path);
        let compiled_src = resolve_user_path(base_dir, opts.custom_css_path);

        for candidate in [&dst, &compiled_src] {
            if let Ok(bytes) = std::fs::read(candidate)
                && !bytes.is_empty()
            {
                return css_content_fingerprint(&bytes);
            }
        }
        1
    }

    pub fn inject_metrics_block(raw_css: &str, m: &CssMetrics) -> String {
        let metrics_block = format!(
            "{METRICS_START_MARKER}\n\
             .panel-button.genubar-panel-button {{ -natural-hpadding: {}px; }}\n\
             .popup-menu-boxpointer .popup-menu-content {{ border-radius: {}px; }}\n\
             .genubar-menu-scroll, .genubar-submenu-scroll {{ max-height: {}px; min-width: {}px; }}\n\
             .genubar-item-row, .genubar-search-entry {{ padding: {}px {}px; font-size: {}px; border-radius: {}px; }}\n\
             .genubar-item-label, .genubar-item-label-disabled {{ font-size: {}px; }}\n\
             {METRICS_END_MARKER}",
            m.panel_button_hpadding_px,
            m.popup_border_radius_px,
            m.max_column_height_px,
            m.min_column_width_px,
            m.item_vpadding_px,
            m.item_hpadding_px,
            m.font_size_px,
            m.item_border_radius_px,
            m.font_size_px
        );

        if let (Some(start), Some(end)) = (
            raw_css.find(METRICS_START_MARKER),
            raw_css.find(METRICS_END_MARKER),
        ) && end > start
        {
            let end_full = end + METRICS_END_MARKER.len();
            return format!(
                "{}{}{}",
                &raw_css[..start],
                metrics_block,
                &raw_css[end_full..]
            );
        }

        format!("{raw_css}\n\n{metrics_block}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_and_updates_metrics_block_idempotently() {
        let m1 = CssMetrics {
            panel_button_hpadding_px: 6,
            popup_border_radius_px: 14,
            max_column_height_px: 620,
            min_column_width_px: 215,
            item_vpadding_px: 5,
            item_hpadding_px: 10,
            item_border_radius_px: 9,
            font_size_px: 13,
        };
        let once = CssSynchronizer::inject_metrics_block(".base { color: red; }", &m1);
        assert!(once.contains("font-size: 13px;"));

        let mut m2 = m1;
        m2.font_size_px = 16;
        let twice = CssSynchronizer::inject_metrics_block(&once, &m2);
        assert_eq!(twice.matches(METRICS_START_MARKER).count(), 1);
        assert!(twice.contains("font-size: 16px;"));
        assert!(!twice.contains("font-size: 13px;"));
    }

    #[test]
    fn css_fingerprint_is_deterministic_js_safe_and_detects_edits() {
        let fp1 = css_content_fingerprint(b".genubar-panel-button { padding: 6px; }");
        let fp2 = css_content_fingerprint(b".genubar-panel-button { padding: 6px; }");
        let fp3 = css_content_fingerprint(b".genubar-panel-button { padding: 8px; }");

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
        assert!(fp1 > 0 && fp1 <= JS_MAX_SAFE_INT_MASK);
        assert!(fp3 > 0 && fp3 <= JS_MAX_SAFE_INT_MASK);
    }
}
