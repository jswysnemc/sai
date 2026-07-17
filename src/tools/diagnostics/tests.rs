#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_issue_infers_input_method_area() {
        let args = parse_args(json!({"query": "QQ 打不了中文", "target": "qq"})).unwrap();
        assert!(matches!(args.area, Area::InputMethod));
    }

    #[test]
    fn input_method_path_needs_runtime_evidence() {
        let status = input_method_path_status(
            InputToolkit::Unknown,
            DisplayMode::Unknown,
            false,
            None,
            &[],
            &[],
            &[],
            &WaylandProtocolInfo {
                compositor_supports_text_input_v3: false,
                fcitx5_wayland_frontend_loaded: false,
                wayland_info_available: false,
            },
            &LocaleInfo {
                target_lang: None,
                target_lc_ctype: None,
                available_locales: vec![],
                locale_valid: false,
            },
        );
        assert_eq!(status.overall, "path_evidence_incomplete");
    }
}
