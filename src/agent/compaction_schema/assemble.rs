use super::catalog::{machine_filled_section, section_after_machine_filled};

/// 把机器填充节与回读指引合进模型产出的摘要。
///
/// 指令已要求模型跳过第 6 节，但模型未必照办。这里先移除它可能自行写下的
/// 那一节再插入真本，否则同一节会出现两份、且模型那份是转述过的。
///
/// 参数:
/// - `summary`: 模型产出的摘要正文
/// - `user_section`: 程序生成的用户原话节，含标题
/// - `pointer`: 回读指引；不可用时为 None
///
/// 返回:
/// - 合成后的完整摘要
pub(crate) fn assemble(summary: &str, user_section: &str, pointer: Option<&str>) -> String {
    // 1. 去掉模型自行撰写的第 6 节，只保留程序生成的那份
    let stripped = remove_section(
        summary,
        machine_filled_section().ordinal,
        machine_filled_section().title,
    );
    // 2. 按后继节的标题定位插入点，定位不到则并到末尾
    let merged = insert_before_section(
        &stripped,
        section_after_machine_filled().ordinal,
        section_after_machine_filled().title,
        user_section,
    );
    match pointer {
        Some(pointer) => format!("{}\n\n{pointer}", merged.trim_end()),
        None => merged,
    }
}

/// 移除摘要中指定序号的小节。
///
/// 从命中的标题行起，删到下一个小节标题行为止。
///
/// 参数:
/// - `summary`: 摘要正文
/// - `ordinal`: 小节序号
/// - `title`: 小节标题正文
///
/// 返回:
/// - 移除该节后的正文；未命中时原样返回
fn remove_section(summary: &str, ordinal: usize, title: &str) -> String {
    let lines: Vec<&str> = summary.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|line| is_heading_of(line, ordinal, title))
    else {
        return summary.to_string();
    };
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| is_any_heading(line))
        .map(|(index, _)| index)
        .unwrap_or(lines.len());
    let mut kept: Vec<&str> = lines[..start].to_vec();
    kept.extend_from_slice(&lines[end..]);
    kept.join("\n").trim_end().to_string()
}

/// 在指定小节之前插入一段文本。
///
/// 参数:
/// - `summary`: 摘要正文
/// - `ordinal`: 锚点小节序号
/// - `title`: 锚点小节标题正文
/// - `block`: 待插入文本
///
/// 返回:
/// - 插入后的正文；锚点缺失时把文本并到末尾
fn insert_before_section(summary: &str, ordinal: usize, title: &str, block: &str) -> String {
    let lines: Vec<&str> = summary.lines().collect();
    let Some(anchor) = lines
        .iter()
        .position(|line| is_heading_of(line, ordinal, title))
    else {
        return format!("{}\n\n{block}", summary.trim_end());
    };
    let head = lines[..anchor].join("\n");
    let tail = lines[anchor..].join("\n");
    format!("{}\n\n{block}\n\n{tail}", head.trim_end())
}

/// 判断一行是否为指定小节的标题。
///
/// 模型可能把标题写成 `## 6. Title`、`**6. Title**` 或裸文本，
/// 这里只要求该行去掉修饰后以"序号加标题"开头。
///
/// 参数:
/// - `line`: 待判定行
/// - `ordinal`: 小节序号
/// - `title`: 小节标题正文
///
/// 返回:
/// - 命中时为真
fn is_heading_of(line: &str, ordinal: usize, title: &str) -> bool {
    let bare = strip_decorations(line);
    let expected = format!("{ordinal}. {title}");
    bare.eq_ignore_ascii_case(&expected)
}

/// 判断一行是否为任意小节标题。
///
/// 参数:
/// - `line`: 待判定行
///
/// 返回:
/// - 该行以 markdown 标题标记开头时为真
fn is_any_heading(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// 去掉标题行的 markdown 修饰。
///
/// 参数:
/// - `line`: 原始行
///
/// 返回:
/// - 去掉井号、星号与首尾空白后的文本
fn strip_decorations(line: &str) -> &str {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一份写齐了除第 6 节外全部小节的摘要。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 摘要正文
    fn model_summary() -> String {
        "## 5. Problem Solving\n已解决\n\n## 7. Pending Tasks\n待办\n\n## 8. Current Work\n在做"
            .to_string()
    }

    /// 验证用户原话节被插到第 7 节之前。
    #[test]
    fn user_section_lands_before_pending_tasks() {
        let merged = assemble(&model_summary(), "## 6. All user messages\n- 原话", None);

        let user_at = merged.find("## 6. All user messages").unwrap();
        let pending_at = merged.find("## 7. Pending Tasks").unwrap();
        let solving_at = merged.find("## 5. Problem Solving").unwrap();
        assert!(solving_at < user_at);
        assert!(user_at < pending_at);
    }

    /// 验证模型自行写下的第 6 节被替换而非叠加。
    ///
    /// 模型那份是转述过的，留着会与逐字原话互相矛盾。
    #[test]
    fn a_model_written_section_six_is_replaced() {
        let summary = "## 6. All user messages\n用户大意是想改压缩\n\n## 7. Pending Tasks\n待办";

        let merged = assemble(summary, "## 6. All user messages\n- 请把压缩改成九节", None);

        assert!(!merged.contains("用户大意是想改压缩"));
        assert!(merged.contains("- 请把压缩改成九节"));
        assert_eq!(merged.matches("## 6. All user messages").count(), 1);
    }

    /// 验证锚点缺失时用户原话仍被保留。
    ///
    /// 宁可位置不对也不能丢：这一节是唯一的零失真记录。
    #[test]
    fn user_section_survives_a_missing_anchor() {
        let merged = assemble(
            "## 1. Primary Request and Intent\n改压缩",
            "## 6. All user messages\n- 原话",
            None,
        );

        assert!(merged.contains("- 原话"));
    }

    /// 验证模型把标题写成加粗形式时仍能定位。
    #[test]
    fn bold_headings_are_still_recognized() {
        let summary = "**5. Problem Solving**\n已解决\n\n**7. Pending Tasks**\n待办";

        let merged = assemble(summary, "## 6. All user messages\n- 原话", None);

        let user_at = merged.find("## 6. All user messages").unwrap();
        let pending_at = merged.find("7. Pending Tasks").unwrap();
        assert!(user_at < pending_at);
    }

    /// 验证回读指引附在最后。
    #[test]
    fn pointer_is_appended_at_the_end() {
        let merged = assemble(
            &model_summary(),
            "## 6. All user messages\n- 原话",
            Some("---\n可回读"),
        );

        assert!(merged.trim_end().ends_with("可回读"));
    }

    /// 验证移除第 6 节不会波及后面的小节。
    #[test]
    fn removing_section_six_leaves_later_sections_intact() {
        let summary = "## 6. All user messages\n转述\n\n## 7. Pending Tasks\n待办\n\n## 9. Optional Next Step\n下一步";

        let merged = assemble(summary, "## 6. All user messages\n- 原话", None);

        assert!(merged.contains("## 7. Pending Tasks"));
        assert!(merged.contains("待办"));
        assert!(merged.contains("## 9. Optional Next Step"));
        assert!(merged.contains("下一步"));
    }
}
