use genubar_core::MenuNode;
use std::collections::HashSet;

// Keyword dictionaries covering the GNOME Core Apps suite
// (Files, Ptyxis, Text Editor, Loupe, Papers, Calculator, Calendar, Clocks, Snapshot, System Monitor, Software)
pub const FILE_KEYWORDS: &[&str] = &[
    "new",
    "open",
    "save",
    "print",
    "export",
    "import",
    "close",
    "discard",
    "properties",
    "console",
    "terminal",
    "location",
    "bookmark",
    "star",
    "trash",
    "background",
    "wallpaper",
    "email",
    "send",
    "compress",
    "extract",
    "mount",
    "unmount",
    "eject",
    "format",
    "repositor",
];

pub const EDIT_KEYWORDS: &[&str] = &[
    "find",
    "replace",
    "search",
    "preference",
    "setting",
    "copy",
    "paste",
    "cut",
    "undo",
    "redo",
    "select",
    "spell",
    "rename",
    "rotate",
    "flip",
    "duplicate",
    "clear",
    "reorder",
    "set title",
];

pub const VIEW_KEYWORDS: &[&str] = &[
    "fullscreen",
    "zoom",
    "sidebar",
    "side pane",
    "grid",
    "list",
    "sort",
    "hidden",
    "view",
    "show-",
    "captions",
    "reload",
    "refresh",
    "stop",
    "theme",
    "appearance",
    "dark",
    "light",
    "style",
    "night",
    "continuous",
    "dual",
    "presentation",
    "line number",
    "margin",
    "wrap",
    "indent",
    "highlight",
    "overview",
    "read-only",
    "read only",
    "process",
    "dependencies",
    "gallery",
    "countdown",
    "flash",
    "guideline",
    "mirror",
    "basic",
    "advanced",
    "financial",
    "programming",
    "conversion",
    "mode",
    "result format",
    "scientific",
    "engineering",
    "odd pages",
    "right to left",
    "slideshow",
    "temperature",
    "celsius",
    "fahrenheit",
    "measurement",
    "scale",
    "marker",
    "layer",
];

pub const HELP_KEYWORDS: &[&str] = &[
    "keyboard shortcut",
    "shortcut",
    "help",
    "about",
    "inspector",
    "tour",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gtk4ItemBucket {
    ScanMode,
    DocumentType,
    File,
    Edit,
    View,
    Help,
    Skip,
}

fn matches_keywords(lbl: &str, act: &str, keywords: &[&str]) -> bool {
    keywords
        .iter()
        .any(|k| (!lbl.is_empty() && lbl.contains(k)) || (!act.is_empty() && act.contains(k)))
}

pub fn classify_gtk4_item(
    item: &MenuNode,
    expose_unknown: bool,
    seen_labels: &mut HashSet<String>,
) -> Gtk4ItemBucket {
    let lbl = item.clean_label().to_lowercase();
    let act = item.action.as_deref().unwrap_or("").to_lowercase();

    if lbl == "unknown" || lbl == "unknow" {
        return if expose_unknown {
            Gtk4ItemBucket::View
        } else {
            Gtk4ItemBucket::Skip
        };
    }

    if matches!(lbl.as_str(), "used" | "created" | "modified") || !seen_labels.insert(lbl.clone()) {
        return Gtk4ItemBucket::Skip;
    }

    if act.starts_with("app.scan_type") {
        return Gtk4ItemBucket::ScanMode;
    }
    if act.starts_with("app.document_hint") {
        return Gtk4ItemBucket::DocumentType;
    }

    // Group by item label first so actions like `view.copy-current-location` ("Copy Location")
    // or `view.empty-trash` ("Empty Trash") are not misrouted into `View` just because of the `view.` prefix
    if matches_keywords(&lbl, "", HELP_KEYWORDS) || matches_keywords("", &act, HELP_KEYWORDS) {
        Gtk4ItemBucket::Help
    } else if matches_keywords(&lbl, "", FILE_KEYWORDS) {
        Gtk4ItemBucket::File
    } else if matches_keywords(&lbl, "", EDIT_KEYWORDS) {
        Gtk4ItemBucket::Edit
    } else if matches_keywords(&lbl, &act, VIEW_KEYWORDS) {
        Gtk4ItemBucket::View
    } else if matches_keywords("", &act, EDIT_KEYWORDS) {
        Gtk4ItemBucket::Edit
    } else {
        Gtk4ItemBucket::File
    }
}
