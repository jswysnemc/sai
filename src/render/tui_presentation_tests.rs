use super::*;
use crate::config::TuiStyle;
use crate::render::tui_style;

/// 【终端】【语言测试】DeepSeek 使用英文，退出渲染后仍保留中文设置和原始内容。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_presentation_preserves_locale_and_source_content() {
    crate::i18n::with_locale(Locale::Zh, || {
        tui_style::set_override(Some(TuiStyle::Deepseek));
        let content = with_locale(|| {
            assert_eq!(crate::i18n::locale(), Locale::En);
            format!(
                "{} · 用户原文 · 模型原文",
                crate::i18n::text("Thinking", "思考中")
            )
        });
        assert_eq!(content, "Thinking · 用户原文 · 模型原文");
        assert_eq!(crate::i18n::locale(), Locale::Zh);
        tui_style::set_override(Some(TuiStyle::Classic));
        assert_eq!(
            with_locale(|| crate::i18n::text("Thinking", "思考中")),
            "思考中"
        );
        tui_style::set_override(None);
    });
}

/// 【终端】【语言测试】异常展开不会使后续命令或经典界面继承英文作用域。
/// 参数: 无；返回: 无。
#[test]
fn deepseek_presentation_restores_locale_after_panics() {
    crate::i18n::with_locale(Locale::Zh, || {
        tui_style::set_override(Some(TuiStyle::Deepseek));
        let result = std::panic::catch_unwind(|| with_locale(|| panic!("render fixture")));
        assert!(result.is_err());
        assert_eq!(crate::i18n::locale(), Locale::Zh);
        tui_style::set_override(None);
    });
}
