use crate::models::style::StyleMetadata;
use genubar_core::MenuError;
use genubar_platform::fs_utils::{
    copy_file, ensure_dir, read_text_file as read_string, write_text_file as write_string,
};
use genubar_platform::parse_metadata_block;
use std::fs;
use std::path::{Path, PathBuf};

const STYLE_HEADER_START: &str = "/* ==GenubarStyle==";
const STYLE_HEADER_END: &str = "==/GenubarStyle== */";

#[derive(Debug, Clone)]
pub struct StylesManager {
    styles_dir: PathBuf,
}

impl StylesManager {
    pub fn new(styles_dir: PathBuf) -> Self {
        let _ = ensure_dir(&styles_dir);
        Self { styles_dir }
    }

    pub fn styles_dir(&self) -> &Path {
        &self.styles_dir
    }

    pub fn list_styles(&self) -> Vec<StyleMetadata> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.styles_dir) else {
            return out;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("css")
                && let Some(fname) = path.file_name().and_then(|n| n.to_str()) {
                    let raw = read_string(&path).unwrap_or_default();
                    out.push(Self::parse_header(fname, &raw));
                }
        }

        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn parse_header(file_name: &str, css_content: &str) -> StyleMetadata {
        let stem = file_name.trim_end_matches(".css");
        let mut meta = StyleMetadata {
            file_name: file_name.to_string(),
            id: stem.to_string(),
            name: if stem == "custom" {
                String::from("Custom (User Modified)")
            } else {
                stem.replace('_', " ")
            },
            version: String::from("1.0.0"),
            author: String::from("local"),
            description: String::new(),
            min_app_version: None,
        };

        if let Some(map) = parse_metadata_block(css_content, STYLE_HEADER_START, STYLE_HEADER_END) {
            if let Some(v) = map.get("id") {
                meta.id = v.clone();
            }
            if let Some(v) = map.get("name") {
                meta.name = v.clone();
            }
            if let Some(v) = map.get("version") {
                meta.version = v.clone();
            }
            if let Some(v) = map.get("author") {
                meta.author = v.clone();
            }
            if let Some(v) = map.get("description") {
                meta.description = v.clone();
            }
            if let Some(v) = map.get("min_app_version") {
                meta.min_app_version = Some(v.clone());
            }
        }

        meta
    }

    pub fn save_custom_modified_css(&self, modified_css: &str) -> Result<String, MenuError> {
        let custom_file = self.styles_dir.join("custom.css");
        let content = if modified_css.contains(STYLE_HEADER_START) {
            modified_css.to_string()
        } else {
            format!(
                "{STYLE_HEADER_START}\n * @id custom\n * @name Custom (User Modified)\n * @version 1.0.0\n * @author user\n * @description User-customized stylesheet\n {STYLE_HEADER_END}\n\n{modified_css}"
            )
        };
        write_string(&custom_file, &content)?;
        Ok(String::from("custom.css"))
    }

    pub fn import_style_file(&self, src_path: &Path) -> Result<String, MenuError> {
        let file_name = src_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| MenuError::Config("Invalid style file name".into()))?
            .to_string();
        let dst_path = self.styles_dir.join(&file_name);
        copy_file(src_path, &dst_path)?;
        Ok(file_name)
    }
}