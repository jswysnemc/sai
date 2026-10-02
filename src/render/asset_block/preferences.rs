use crate::config::DisplayConfig;
#[cfg(not(test))]
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(not(test))]
static FLAGS: AtomicU8 = AtomicU8::new(3);
#[cfg(test)]
thread_local! { static FLAGS: std::cell::Cell<u8> = const { std::cell::Cell::new(3) }; }

/// 【终端显示】【图片偏好】参数为显示配置，更新公式与 Mermaid 渲染开关，无返回值。
pub(crate) fn configure(config: &DisplayConfig) {
    let flags = u8::from(config.math_images) | (u8::from(config.mermaid_images) << 1);
    #[cfg(not(test))]
    FLAGS.store(flags, Ordering::Relaxed);
    #[cfg(test)]
    FLAGS.set(flags);
}

/// 【终端显示】【读取偏好】无参数，返回当前渲染开关位图。
fn flags() -> u8 {
    #[cfg(not(test))]
    {
        FLAGS.load(Ordering::Relaxed)
    }
    #[cfg(test)]
    {
        FLAGS.get()
    }
}

/// 【终端显示】【公式图片】无参数，返回是否将公式渲染成图片。
pub(super) fn math_images() -> bool {
    flags() & 1 != 0
}

/// 【终端显示】【图表图片】无参数，返回是否将 Mermaid 渲染成图片。
pub(super) fn mermaid_images() -> bool {
    flags() & 2 != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 【终端显示】【配置兼容】缺省开启，显式关闭经过保存读取后仍生效；无参数和返回值。
    #[test]
    fn display_preferences_default_on_and_preserve_false() {
        let defaults: DisplayConfig = serde_json::from_str("{}").unwrap();
        assert!(defaults.fullscreen && defaults.math_images && defaults.mermaid_images);
        let configured: DisplayConfig = serde_json::from_str(
            r#"{"fullscreen":false,"math_images":false,"mermaid_images":false}"#,
        )
        .unwrap();
        let restored: DisplayConfig =
            serde_json::from_value(serde_json::to_value(configured).unwrap()).unwrap();
        assert!(!restored.fullscreen && !restored.math_images && !restored.mermaid_images);
    }

    /// 【终端显示】【源码回退】两个开关互不影响，关闭公式后所有入口均不生成图片；无参数和返回值。
    #[test]
    fn disabled_images_render_sources_without_using_asset_cache() {
        let mut config = DisplayConfig::default();
        config.math_images = false;
        configure(&config);
        assert!(!super::super::is_asset_language("math"));
        assert!(super::super::is_asset_language("mermaid"));
        assert_eq!(
            super::super::render_math_block(&["x^2".into()]),
            "$$\nx^2\n$$\n"
        );
        assert_eq!(super::super::render_inline_math_at("x^2", "one"), "$x^2$");
        assert_eq!(super::super::render_inline_math_halfblock("x^2"), "$x^2$");
        config.mermaid_images = false;
        configure(&config);
        assert!(!super::super::is_asset_language("mermaid"));
        assert!(super::super::is_asset_language("svg"));
        assert_eq!(
            super::super::render_asset_block("mermaid", &["graph TD; A-->B".into()]),
            "graph TD; A-->B\n"
        );
        configure(&DisplayConfig::default());
        assert!(super::super::is_asset_language("math"));
        assert!(super::super::is_asset_language("mermaid"));
    }
}
