use serde::{Deserialize, Serialize};

/// 记忆条目的类型。
///
/// 类型决定召回时的注入措辞与显著性下限：偏好和决策是长期约束，
/// 必须让模型明确遵循；事实是背景信息；往事只在明确相关时才有价值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// 用户的稳定偏好：工具链选择、代码风格、沟通方式
    Preference,
    /// 关于用户或环境的客观事实
    Fact,
    /// 已经做出的技术决策及其理由
    Decision,
    /// 发生过的事：做了什么、结果如何
    Episode,
}

impl MemoryKind {
    /// 解析抽取模型输出的类型标识。
    ///
    /// 参数:
    /// - `value`: 类型文本
    ///
    /// 返回:
    /// - 匹配的类型；无法识别时为 None
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "preference" => Some(Self::Preference),
            "fact" => Some(Self::Fact),
            "decision" => Some(Self::Decision),
            "episode" => Some(Self::Episode),
            _ => None,
        }
    }

    /// 返回存库使用的标识。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 小写类型标识
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preference => "preference",
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Episode => "episode",
        }
    }

    /// 返回该类型允许入库的最低显著性。
    ///
    /// 往事最容易退化成流水账，门槛最高；偏好一旦出现就值得长期保留，门槛最低。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 显著性下限，取值 0.0 到 1.0
    pub fn capture_floor(self) -> f64 {
        match self {
            Self::Preference => 0.35,
            Self::Decision => 0.45,
            Self::Fact => 0.5,
            Self::Episode => 0.7,
        }
    }

    /// 返回注入上下文时使用的中文小节标题。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 小节标题
    pub fn section_title(self) -> &'static str {
        match self {
            Self::Preference => "用户的既定偏好",
            Self::Fact => "已知事实",
            Self::Decision => "既往决策",
            Self::Episode => "相关往事",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证类型标识可以往返解析。
    #[test]
    fn parses_every_kind_from_its_identifier() {
        for kind in [
            MemoryKind::Preference,
            MemoryKind::Fact,
            MemoryKind::Decision,
            MemoryKind::Episode,
        ] {
            assert_eq!(MemoryKind::parse(kind.as_str()), Some(kind));
        }
    }

    /// 验证大小写与空白不影响解析。
    #[test]
    fn parsing_ignores_case_and_whitespace() {
        assert_eq!(MemoryKind::parse("  Preference "), Some(MemoryKind::Preference));
    }

    /// 验证未知标识不产生类型。
    #[test]
    fn unknown_identifier_yields_none() {
        assert_eq!(MemoryKind::parse("diary"), None);
    }

    /// 验证往事的入库门槛高于偏好。
    ///
    /// 流水账正是往事类堆积造成的，它的门槛必须最严。
    #[test]
    fn episodes_have_the_strictest_capture_floor() {
        assert!(MemoryKind::Episode.capture_floor() > MemoryKind::Fact.capture_floor());
        assert!(MemoryKind::Fact.capture_floor() > MemoryKind::Decision.capture_floor());
        assert!(MemoryKind::Decision.capture_floor() > MemoryKind::Preference.capture_floor());
    }
}
