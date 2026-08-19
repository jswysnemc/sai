//! 供应商 `/models` 响应中 `reasoning` 字段的兼容解析。
//!
//! 该字段有两种写法：早期兼容接口用布尔开关表示"是否支持推理"，
//! OpenRouter 一类则给出对象，额外列出可选的推理强度档位。
//! 只按布尔解析会让整份响应反序列化失败，供应商的模型一个也取不到。

use crate::config::map_catalog_thinking_level;
use crate::config::THINKING_LEVELS;
use serde::Deserialize;

/// `reasoning` 字段的两种形态。
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ReasoningField {
    /// 布尔开关，只表明是否支持推理
    Flag(bool),
    /// 能力对象，可能带有支持的强度档位
    Detail(ReasoningDetail),
}

/// `reasoning` 对象形态下关心的字段。
#[derive(Deserialize)]
pub(super) struct ReasoningDetail {
    #[serde(default)]
    supported_efforts: Vec<String>,
}

impl ReasoningField {
    /// 判断该模型是否支持推理。
    ///
    /// 返回:
    /// - 布尔形态取其值；对象形态视为支持
    pub(super) fn supports_thinking(&self) -> bool {
        match self {
            Self::Flag(enabled) => *enabled,
            Self::Detail(_) => true,
        }
    }

    /// 取出该模型支持的思考等级。
    ///
    /// 返回:
    /// - 按强度升序排列的 Sai 等级；无法判定时返回空，表示全部可用
    pub(super) fn thinking_levels(&self) -> Vec<String> {
        let Self::Detail(detail) = self else {
            return Vec::new();
        };
        let mut levels: Vec<&'static str> = Vec::new();
        // 1. 逐项映射为 Sai 等级，认不出的档位直接丢弃
        for effort in &detail.supported_efforts {
            let Some(level) = map_catalog_thinking_level(effort) else {
                continue;
            };
            if !levels.contains(&level) {
                levels.push(level);
            }
        }
        // 2. 按 Sai 的强度顺序排列，保证界面档位从弱到强
        levels.sort_by_key(|level| {
            THINKING_LEVELS
                .iter()
                .position(|item| item == level)
                .unwrap_or(usize::MAX)
        });
        levels.into_iter().map(str::to_string).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::ReasoningField;

    /// 按 JSON 解析出 reasoning 字段。
    fn parse(raw: &str) -> ReasoningField {
        serde_json::from_str(raw).expect("reasoning 字段应当可解析")
    }

    /// 验证布尔写法仍然可用。
    #[test]
    fn parses_boolean_form() {
        assert!(parse("true").supports_thinking());
        assert!(!parse("false").supports_thinking());
        assert!(parse("true").thinking_levels().is_empty());
    }

    /// 验证 OpenRouter 的对象写法不再让解析失败，并给出思考等级。
    #[test]
    fn parses_openrouter_object_form() {
        let field = parse(
            r#"{"mandatory":true,"default_enabled":true,"supported_efforts":["max","high","low"],"default_effort":"max"}"#,
        );

        assert!(field.supports_thinking());
        assert_eq!(field.thinking_levels(), ["low", "high", "max"]);
    }

    /// 验证目录里的 minimal 归到 Sai 的 none 档并去重。
    #[test]
    fn maps_unknown_and_aliased_efforts() {
        let field = parse(r#"{"supported_efforts":["minimal","none","medium","bogus"]}"#);

        assert_eq!(field.thinking_levels(), ["none", "medium"]);
    }

    /// 验证对象缺少档位时退回"全部可用"而不是"全部不可用"。
    #[test]
    fn object_without_efforts_stays_unrestricted() {
        let field = parse(r#"{"mandatory":false}"#);

        assert!(field.supports_thinking());
        assert!(field.thinking_levels().is_empty());
    }
}
