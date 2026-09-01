use serde::{Deserialize, Serialize};

/// 记忆条目的类型。
///
/// 分类按"这条信息从哪来、要拿它做什么"划分，而不是按主题：召回时
/// 决定注入措辞的是它的约束力强弱，主题反而无关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// 用户是谁：角色、专长领域、长期偏好
    User,
    /// 用户给出的工作方式指导，包含纠正与确认过的做法
    Feedback,
    /// 进行中的工作、目标与约束，且无法从代码或提交历史看出
    Project,
    /// 外部资源指针：网址、看板、工单
    Reference,
}

impl MemoryType {
    /// 解析类型标识。
    ///
    /// 参数:
    /// - `value`: 类型文本
    ///
    /// 返回:
    /// - 匹配的类型；无法识别时为 None
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "user" => Some(Self::User),
            "feedback" => Some(Self::Feedback),
            "project" => Some(Self::Project),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }

    /// 返回写入文件时使用的标识。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 小写类型标识
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }

    /// 返回该类型是否要求写明理由与应用方式。
    ///
    /// 工作方式指导和项目约束缺了理由就退化成孤立命令，下一轮无从判断
    /// 它在新情境下还适不适用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 需要理由时为真
    pub fn requires_rationale(self) -> bool {
        matches!(self, Self::Feedback | Self::Project)
    }

    /// 检查需要理由的类型是否写全了理由与应用方式。
    ///
    /// 工具与网页接口共用这一份判定：两边各写一遍，模型和用户拿到的
    /// 提示迟早会不一样。
    ///
    /// 参数:
    /// - `body`: 正文
    ///
    /// 返回:
    /// - 缺失提示；无需理由或已写全时为 None
    pub fn missing_rationale(self, body: &str) -> Option<String> {
        if !self.requires_rationale() {
            return None;
        }
        let missing: Vec<&str> = RATIONALE_MARKERS
            .iter()
            .filter(|marker| !body.contains(**marker))
            .copied()
            .collect();
        if missing.is_empty() {
            return None;
        }
        Some(format!(
            "{} 类记忆建议在正文补上 {}：缺了理由，下一轮无法判断它在新情境下还适不适用。",
            self.as_str(),
            missing.join(" 与 ")
        ))
    }
}

/// 需要说明理由的类型必须出现的两个小标题。
const RATIONALE_MARKERS: [&str; 2] = ["Why:", "How to apply:"];

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证类型标识可以往返解析。
    #[test]
    fn parses_every_type_from_its_identifier() {
        for kind in [
            MemoryType::User,
            MemoryType::Feedback,
            MemoryType::Project,
            MemoryType::Reference,
        ] {
            assert_eq!(MemoryType::parse(kind.as_str()), Some(kind));
        }
    }

    /// 验证大小写与空白不影响解析。
    ///
    /// 记忆文件可以被用户手改，宽容一点比报错更合适。
    #[test]
    fn parsing_ignores_case_and_whitespace() {
        assert_eq!(MemoryType::parse("  Feedback "), Some(MemoryType::Feedback));
    }

    /// 验证未知标识不产生类型。
    #[test]
    fn unknown_identifier_yields_none() {
        assert_eq!(MemoryType::parse("episode"), None);
    }

    /// 验证只有需要说明理由的类型被标记。
    #[test]
    fn only_constraint_types_require_a_rationale() {
        assert!(MemoryType::Feedback.requires_rationale());
        assert!(MemoryType::Project.requires_rationale());
        assert!(!MemoryType::User.requires_rationale());
        assert!(!MemoryType::Reference.requires_rationale());
    }

    /// 验证需要理由的类型缺小标题时给出提示。
    #[test]
    fn a_feedback_without_rationale_is_flagged() {
        let note = MemoryType::Feedback
            .missing_rationale("一律使用 pnpm")
            .unwrap();

        assert!(note.contains("Why:"));
        assert!(note.contains("How to apply:"));
    }

    /// 验证写全理由后不再提示。
    #[test]
    fn a_complete_feedback_passes() {
        let body =
            "一律使用 pnpm\n\n**Why:** 锁文件不能混用\n**How to apply:** 装依赖时用 pnpm add";

        assert!(MemoryType::Feedback.missing_rationale(body).is_none());
    }

    /// 验证不需要理由的类型从不提示。
    #[test]
    fn types_without_a_rationale_requirement_are_never_flagged() {
        assert!(MemoryType::User.missing_rationale("用户是 Rust 开发者").is_none());
        assert!(MemoryType::Reference.missing_rationale("看板：http://x").is_none());
    }

    /// 验证只缺一个小标题时只提示那一个。
    #[test]
    fn only_the_missing_marker_is_reported() {
        let note = MemoryType::Project
            .missing_rationale("目标\n\n**Why:** 因为")
            .unwrap();

        assert!(note.contains("How to apply:"));
        assert!(!note.contains("Why: 与"));
    }
}
