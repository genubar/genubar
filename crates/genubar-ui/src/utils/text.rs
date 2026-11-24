/// Escapes XML/Pango characters so `&`, `<`, and `>` never break Libadwaita row markup titles.
pub fn escape_markup(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Splits a comma-separated string into trimmed non-empty items.
pub fn parse_csv_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn format_csv_list(items: &[String]) -> String {
    items.join(", ")
}
