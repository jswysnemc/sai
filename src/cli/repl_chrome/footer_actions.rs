//! 【终端】【底栏入口】按默认状态栏实际显示文字建立动作范围。

use super::ReplChrome;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::cli) enum FooterAction {
    Mode,
    Model,
    Thinking,
    Context,
}

impl FooterAction {
    /// 【终端】【底栏命令】无参数；返回对应斜杠命令，模式使用现有快捷键流程。
    pub(in crate::cli) fn command(self) -> Option<&'static str> {
        match self {
            Self::Mode => None,
            Self::Model => Some("/model"),
            Self::Thinking => Some("/thinking"),
            Self::Context => Some("/context"),
        }
    }
}

impl ReplChrome {
    /// 【终端】【底栏命中】参数为实际绘制行与列数；返回未裁断的默认状态字段，插件自行布局时不猜测入口。
    pub(in crate::cli) fn footer_actions(
        &self,
        rendered: &str,
        cols: usize,
    ) -> Vec<(Range<usize>, FooterAction)> {
        if self
            .status_plugin
            .as_ref()
            .is_some_and(|plugin| plugin.active())
        {
            return Vec::new();
        }
        let plain = crate::render::ansi_clip::strip_ansi(rendered);
        let pad = super::CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
        let mut prefix = " ".repeat(pad);
        let mut hits = Vec::new();
        for (text, action) in [
            (self.mode_plain().to_string(), FooterAction::Mode),
            (self.context_status(), FooterAction::Context),
            (self.model.clone(), FooterAction::Model),
            (self.thinking.clone(), FooterAction::Thinking),
        ] {
            let start = super::visible_width(&prefix);
            prefix.push_str(&text);
            let end = super::visible_width(&prefix);
            if text.is_empty() || !plain.starts_with(&prefix) || end > cols {
                break;
            }
            hits.push((start..end, action));
            prefix.push_str("  ");
        }
        hits
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【底栏样本】无参数；返回中文模型及缓存占用状态，避免按字节计算命中范围。
    fn chrome() -> ReplChrome {
        ReplChrome {
            mode: crate::agent::AgentMode::Yolo,
            context_ratio: 0.2,
            context_window_tokens: 200_000,
            model: "中文模型".into(),
            thinking: "high".into(),
            directory: "/workspace".into(),
            branch: None,
            cache_hit_ratio: Some(0.5),
            status_plugin: None,
        }
    }

    /// 【终端】【底栏裁剪】无参数和返回值；中文按列命中，截断字段和空白不产生入口。
    #[test]
    fn footer_targets_follow_visible_fields() {
        let chrome = chrome();
        let rendered = chrome.footer_line(120);
        let hits = chrome.footer_actions(&rendered, 120);
        let model = hits
            .iter()
            .find(|(_, action)| *action == FooterAction::Model)
            .unwrap();
        assert_eq!(model.0.len(), 8);
        let thinking = hits
            .iter()
            .find(|(_, action)| *action == FooterAction::Thinking)
            .unwrap();
        assert_eq!(thinking.0.start, model.0.end + 2);
        assert_eq!(thinking.0.len(), 4);
        let clipped = "   yolo  20.0%/200k cache 50%  中文...";
        let hits = chrome.footer_actions(clipped, 40);
        assert!(hits
            .iter()
            .all(|(_, action)| !matches!(action, FooterAction::Model | FooterAction::Thinking)));
        assert!(chrome.footer_actions("custom status", 40).is_empty());
    }
}
