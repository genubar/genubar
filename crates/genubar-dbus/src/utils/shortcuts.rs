/// Converts GTK accelerator strings such as `"<Primary><Shift>s"` into `["Ctrl", "Shift", "S"]`.
pub fn parse_gtk_accel(accel: &str) -> Option<Vec<String>> {
    if accel.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    let mut rest = accel;

    while let Some(start) = rest.find('<') {
        if let Some(end) = rest[start..].find('>') {
            let modifier = &rest[start + 1..start + end];
            parts.push(normalize_modifier(modifier));
            rest = &rest[start + end + 1..];
        } else {
            break;
        }
    }

    if !rest.is_empty() {
        parts.push(rest.to_uppercase());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

/// Parses AT-SPI2 keybinding strings such as `";;<Control>t"` or `"Ctrl+Shift+N"`.
pub fn parse_atspi_shortcut(raw: &str) -> Option<Vec<String>> {
    let candidate = raw
        .split(';')
        .map(str::trim).rfind(|s| !s.is_empty())?;

    if candidate.starts_with('<') && !candidate.contains('+') {
        return parse_gtk_accel(candidate);
    }

    let parts: Vec<String> = candidate
        .split('+')
        .map(|p| {
            let trimmed = p.trim_matches(|c| c == '<' || c == '>');
            normalize_modifier(trimmed)
        })
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

fn normalize_modifier(raw: &str) -> String {
    match raw.to_lowercase().as_str() {
        "primary" | "control" | "ctrl" => String::from("Ctrl"),
        "shift" => String::from("Shift"),
        "alt" | "mod1" => String::from("Alt"),
        "super" | "meta" => String::from("Super"),
        _ => raw.to_string(),
    }
}