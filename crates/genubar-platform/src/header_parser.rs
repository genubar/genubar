use std::collections::HashMap;

/// Extracts `@key value` metadata pairs enclosed between `start_marker` and `end_marker`.
pub fn parse_metadata_block(
    content: &str,
    start_marker: &str,
    end_marker: &str,
) -> Option<HashMap<String, String>> {
    let start_idx = content.find(start_marker)?;
    let end_idx = content.find(end_marker)?;
    if end_idx <= start_idx {
        return None;
    }

    let mut pairs = HashMap::new();
    for line in content[start_idx..end_idx].lines() {
        let cleaned = line
            .trim()
            .trim_start_matches("/*")
            .trim_start_matches('*')
            .trim_start_matches('#')
            .trim();

        if let Some(rest) = cleaned.strip_prefix('@')
            && let Some((key, val)) = rest.split_once(char::is_whitespace) {
                pairs.insert(key.trim().to_string(), val.trim().to_string());
            }
    }

    Some(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_python_and_css_header_metadata_blocks() {
        let py = "# ==GenubarScript==\n# @id my_script\n# @name My Script\n# ==/GenubarScript==\n";
        let map = parse_metadata_block(py, "# ==GenubarScript==", "# ==/GenubarScript==").unwrap();
        assert_eq!(map.get("id").map(String::as_str), Some("my_script"));
        assert_eq!(map.get("name").map(String::as_str), Some("My Script"));

        let css = "/* ==GenubarStyle==\n * @id dark_glass\n * @version 1.2.0\n ==/GenubarStyle== */\n";
        let css_map =
            parse_metadata_block(css, "/* ==GenubarStyle==", "==/GenubarStyle== */").unwrap();
        assert_eq!(css_map.get("id").map(String::as_str), Some("dark_glass"));
        assert_eq!(css_map.get("version").map(String::as_str), Some("1.2.0"));
    }
}
