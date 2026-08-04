use super::memory_kind::MemoryKind;
use super::memory_scope::MemoryScope;
use serde::{Deserialize, Serialize};

/// 一条记忆的完整记录。
///
/// 与旧模型的关键差别：类型与作用域是一等字段而非自由文本 source，
/// 显著性在写入时确定并只随时间衰减，不因召回次数变化。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryItem {
    /// 存库主键；未落库时为 0
    pub id: i64,
    /// 记忆类型
    pub kind: MemoryKind,
    /// 作用域
    pub scope: MemoryScope,
    /// 记忆正文，单条陈述，不含对话原文
    pub content: String,
    /// 写入时判定的显著性，取值 0.0 到 1.0
    pub salience: f64,
    /// 检索标签，用于补充正文中没有出现的同义词
    pub tags: Vec<String>,
    /// 写入时间，RFC3339
    pub created_at: String,
    /// 最近一次更新时间，RFC3339
    pub updated_at: String,
}

/// 待写入的记忆候选，尚未分配主键。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryCandidate {
    /// 记忆类型
    pub kind: MemoryKind,
    /// 作用域
    pub scope: MemoryScope,
    /// 记忆正文
    pub content: String,
    /// 抽取模型判定的显著性
    pub salience: f64,
    /// 检索标签
    pub tags: Vec<String>,
}

impl MemoryCandidate {
    /// 归一化候选内容，去掉空白与空标签。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 归一化后的候选；正文为空时为 None
    pub fn normalized(mut self) -> Option<Self> {
        self.content = self.content.trim().to_string();
        if self.content.is_empty() {
            return None;
        }
        self.salience = self.salience.clamp(0.0, 1.0);
        self.tags = self
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        self.tags.sort();
        self.tags.dedup();
        Some(self)
    }
}

/// 一次召回命中的记忆及其得分。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryHit {
    /// 命中的记忆
    pub item: MemoryItem,
    /// 检索相关度，取值 0.0 到 1.0
    pub relevance: f64,
    /// 按时间衰减后的当前强度
    pub strength: f64,
    /// 相关度与强度的综合得分，用于排序与注入门槛
    pub score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(content: &str, salience: f64, tags: &[&str]) -> MemoryCandidate {
        MemoryCandidate {
            kind: MemoryKind::Preference,
            scope: MemoryScope::Global,
            content: content.to_string(),
            salience,
            tags: tags.iter().map(|tag| tag.to_string()).collect(),
        }
    }

    /// 验证归一化去掉正文与标签的空白。
    #[test]
    fn normalization_trims_content_and_tags() {
        let normalized = candidate("  用户偏好 pnpm  ", 0.6, &["  PNPM ", "包管理"])
            .normalized()
            .expect("正文非空应当保留");
        assert_eq!(normalized.content, "用户偏好 pnpm");
        assert_eq!(normalized.tags, vec!["pnpm".to_string(), "包管理".to_string()]);
    }

    /// 验证空正文的候选被丢弃。
    #[test]
    fn blank_content_is_rejected() {
        assert!(candidate("   ", 0.9, &[]).normalized().is_none());
    }

    /// 验证显著性被钳制到有效区间。
    #[test]
    fn salience_is_clamped() {
        let normalized = candidate("内容", 5.0, &[]).normalized().unwrap();
        assert_eq!(normalized.salience, 1.0);
        let normalized = candidate("内容", -2.0, &[]).normalized().unwrap();
        assert_eq!(normalized.salience, 0.0);
    }

    /// 验证重复标签被去重。
    #[test]
    fn duplicate_tags_are_removed() {
        let normalized = candidate("内容", 0.5, &["pnpm", "PNPM", "pnpm"])
            .normalized()
            .unwrap();
        assert_eq!(normalized.tags, vec!["pnpm".to_string()]);
    }
}
