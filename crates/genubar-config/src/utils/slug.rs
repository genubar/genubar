/// Converts an application ID or window title into a safe lowercase filename slug.
pub fn sanitize_slug(primary: &str, fallback: &str) -> String {
    let base = if !primary.trim().is_empty() {
        primary.trim()
    } else {
        fallback.trim()
    };

    let cleaned: String = base
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();

    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        String::from("unnamed")
    } else {
        trimmed.to_string()
    }
}