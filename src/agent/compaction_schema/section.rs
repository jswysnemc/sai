/// 压缩摘要中一个固定小节的定义。
///
/// 标题刻意固定为英文：它同时是给模型看的结构标记和给程序用的定位锚点。
/// 正文语言仍跟随对话，只有标题这一行不随语言变化，否则中英文会话
/// 会产出两套无法互相定位的摘要。
pub(crate) struct SummarySection {
    /// 小节序号，从 1 开始，与标题一同构成定位锚点
    pub(crate) ordinal: usize,
    /// 小节标题正文，不含序号与 markdown 标记
    pub(crate) title: &'static str,
    /// 该节的填写要求，逐字写进指令
    pub(crate) guidance: &'static str,
    /// 该节是否由程序填充而非模型撰写
    pub(crate) machine_filled: bool,
}

impl SummarySection {
    /// 返回该节在摘要正文中的完整标题行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 形如 `## 6. All user messages` 的标题行
    pub(crate) fn heading(&self) -> String {
        format!("## {}. {}", self.ordinal, self.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证标题行同时带上序号与标题正文。
    #[test]
    fn heading_carries_both_ordinal_and_title() {
        let section = SummarySection {
            ordinal: 6,
            title: "All user messages",
            guidance: "",
            machine_filled: true,
        };

        assert_eq!(section.heading(), "## 6. All user messages");
    }
}
