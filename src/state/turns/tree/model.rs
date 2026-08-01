use serde::{Deserialize, Serialize};

/// 会话树中的一个轮次节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnTreeNode {
    /// 轮次标识
    pub turn_id: String,
    /// 父轮次标识；根轮次为空
    pub parent_turn_id: Option<String>,
    /// 轮次序号，用于同级排序
    pub seq: i64,
    /// 用户输入摘要
    pub user_summary: String,
    /// 助手回复摘要
    pub assistant_summary: String,
    /// 轮次状态文本
    pub status: String,
    /// 用户提问时间
    pub timestamp: String,
    /// 子节点，按 seq 升序
    pub children: Vec<TurnTreeNode>,
}

/// 会话树整体视图。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTree {
    /// 根节点，通常只有一个
    pub roots: Vec<TurnTreeNode>,
    /// 当前活动叶子轮次；空表示会话尚无轮次
    pub active_leaf_id: Option<String>,
    /// 树中的轮次总数
    pub total_turns: usize,
    /// 分叉点数量：拥有多个子节点的轮次
    pub branch_points: usize,
}

/// 摘要文本的最大字符数。
const SUMMARY_CHARS: usize = 80;

/// 把长文本压成单行摘要。
///
/// 树视图每个节点只占一行，换行与多余空白都会破坏对齐。
///
/// 参数:
/// - `text`: 原始文本
///
/// 返回:
/// - 单行摘要，超长时截断并加省略号
pub(super) fn summarize(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= SUMMARY_CHARS {
        return flat;
    }
    format!(
        "{}…",
        flat.chars().take(SUMMARY_CHARS - 1).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 短文本原样保留。
    #[test]
    fn keeps_short_text_intact() {
        assert_eq!(summarize("查一下这个包的版本"), "查一下这个包的版本");
    }

    /// 换行与连续空白压成单个空格。
    #[test]
    fn flattens_whitespace() {
        assert_eq!(summarize("第一行\n\n  第二行"), "第一行 第二行");
    }

    /// 超长文本截断并加省略号。
    #[test]
    fn truncates_long_text() {
        let summary = summarize(&"字".repeat(200));

        assert_eq!(summary.chars().count(), SUMMARY_CHARS);
        assert!(summary.ends_with('…'));
    }
}
