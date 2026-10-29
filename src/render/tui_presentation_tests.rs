use super::*;
use crate::config::TuiMode;
use crate::render::tui_mode;

/// 【终端】【语言测试】全屏界面使用英文，退出渲染后仍保留中文设置和原始内容。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_presentation_preserves_locale_and_source_content() {
    crate::i18n::with_locale(Locale::Zh, || {
        tui_mode::set_override(Some(TuiMode::Fullscreen));
        let content = with_locale(|| {
            assert_eq!(crate::i18n::locale(), Locale::En);
            format!(
                "{} · 用户原文 · 模型原文",
                crate::i18n::text("Thinking", "思考中")
            )
        });
        assert_eq!(content, "Thinking · 用户原文 · 模型原文");
        assert_eq!(crate::i18n::locale(), Locale::Zh);
        tui_mode::set_override(Some(TuiMode::Inline));
        assert_eq!(
            with_locale(|| crate::i18n::text("Thinking", "思考中")),
            "思考中"
        );
        tui_mode::set_override(None);
    });
}

/// 【终端】【语言测试】异常展开不会使后续命令或内联界面继承英文作用域。
/// 参数: 无；返回: 无。
#[test]
fn fullscreen_presentation_restores_locale_after_panics() {
    crate::i18n::with_locale(Locale::Zh, || {
        tui_mode::set_override(Some(TuiMode::Fullscreen));
        let result = std::panic::catch_unwind(|| with_locale(|| panic!("render fixture")));
        assert!(result.is_err());
        assert_eq!(crate::i18n::locale(), Locale::Zh);
        tui_mode::set_override(None);
    });
}
