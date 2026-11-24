use super::StatefulStepApplier;
use crate::utils::paths::{expand_tokens, resolve_file_pattern};
use genubar_core::MenuError;
use genubar_platform::fs_utils::{read_text_file, write_text_file};

pub(crate) struct TextLineApplier<'a> {
    pub file_pattern: &'a str,
    pub line: &'a str,
}

impl StatefulStepApplier for TextLineApplier<'_> {
    fn resource_key(&self) -> (String, String) {
        (
            format!("line:{}:{}", self.file_pattern, self.line),
            self.line.to_string(),
        )
    }

    fn apply_change(&self, _patch_id: &str) -> Result<Option<String>, MenuError> {
        apply(self.file_pattern, self.line)
    }

    fn revert_change(
        &self,
        _patch_id: &str,
        _orig_opt: Option<String>,
        _force_unsafe: bool,
    ) -> Result<(), MenuError> {
        revert(self.file_pattern, self.line)
    }
}

pub fn apply(file_pattern: &str, line: &str) -> Result<Option<String>, MenuError> {
    let targets = resolve_file_pattern(file_pattern);
    if targets.is_empty() {
        return Err(MenuError::NotFound(format!(
            "No files matched pattern '{file_pattern}'"
        )));
    }

    let expanded_line = expand_tokens(line);
    let trimmed_needle = expanded_line.trim();
    let mut first_original = None;

    for target in targets {
        let existing = if target.is_file() {
            read_text_file(&target)?
        } else {
            String::new()
        };

        if first_original.is_none() {
            first_original = Some(existing.clone());
        }

        if !existing.lines().any(|l| l.trim() == trimmed_needle) {
            let mut updated = existing;
            if !updated.is_empty() && !updated.ends_with('\n') {
                updated.push('\n');
            }
            updated.push_str(trimmed_needle);
            updated.push('\n');
            write_text_file(&target, &updated)?;
        }
    }

    Ok(first_original)
}

pub fn revert(file_pattern: &str, line: &str) -> Result<(), MenuError> {
    let expanded_line = expand_tokens(line);
    let trimmed_needle = expanded_line.trim();

    for target in resolve_file_pattern(file_pattern) {
        if target.is_file() {
            let existing = read_text_file(&target)?;
            let filtered: Vec<&str> = existing
                .lines()
                .filter(|l| l.trim() != trimmed_needle)
                .collect();
            let new_body = if filtered.is_empty() {
                String::new()
            } else {
                filtered.join("\n") + "\n"
            };
            write_text_file(&target, &new_body)?;
        }
    }
    Ok(())
}
