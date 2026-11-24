use genubar_config::{ConfigManager, DesktopMenuMode};
use genubar_core::{ActiveWindowInfo, MenuNode, MenuSourceTier};
use genubar_pipeline::{MenuPipeline, StaticMenuBuilder};

pub struct FallbackMenuBuilder;

impl FallbackMenuBuilder {
    pub fn clean_app_name(info: &ActiveWindowInfo) -> String {
        if !info.app_id.is_empty() {
            let base = info
                .app_id
                .trim_end_matches(".desktop")
                .rsplit('.')
                .next()
                .unwrap_or(&info.app_id);
            let mut chars = base.chars();
            match chars.next() {
                None => String::from("Application"),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        } else if !info.title.is_empty() {
            info.title.clone()
        } else {
            genubar_platform::detect_os_name().to_string()
        }
    }

    pub fn build_desktop_menu(cfg: &ConfigManager, info: &ActiveWindowInfo) -> MenuNode {
        let base = StaticMenuBuilder::build_static_tree(
            &cfg.desktop_menu.columns,
            None,
            MenuSourceTier::DesktopIdle,
            -100,
        );

        match cfg.desktop_menu.mode {
            DesktopMenuMode::Static => base,
            DesktopMenuMode::PythonScript => {
                if let Some(ref script) = cfg.desktop_menu.python_script
                    && let Some(out) = MenuPipeline::run_python_script_with_timeout(
                        &cfg.settings.general.python_binary,
                        cfg.base_dir(),
                        script,
                        info,
                        MenuSourceTier::DesktopIdle,
                        &base,
                        cfg.settings.general.script_timeout_ms,
                    )
                {
                    return out;
                }
                base
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_app_name_falls_back_from_app_id_to_title_to_os_name() {
        let by_id = ActiveWindowInfo {
            app_id: String::from("org.gnome.Nautilus.desktop"),
            title: String::from("Home"),
            ..Default::default()
        };
        assert_eq!(FallbackMenuBuilder::clean_app_name(&by_id), "Nautilus");

        let by_title = ActiveWindowInfo {
            title: String::from("Untitled Window"),
            ..Default::default()
        };
        assert_eq!(
            FallbackMenuBuilder::clean_app_name(&by_title),
            "Untitled Window"
        );

        let by_os = ActiveWindowInfo::default();
        assert_eq!(
            FallbackMenuBuilder::clean_app_name(&by_os),
            genubar_platform::detect_os_name()
        );
    }
}
