use super::section::SummarySection;

/// 压缩摘要的九个固定小节，顺序即输出顺序。
///
/// 顺序不是随意的：先确立"要做什么"（1）与"依赖什么概念"（2），
/// 再落到具体改动（3），然后是负面知识（4、5），用户原话（6）作为
/// 校准锚点居中，最后三节收束到"还剩什么、正在做什么、下一步做什么"。
pub(crate) const SUMMARY_SECTIONS: &[SummarySection] = &[
    SummarySection {
        ordinal: 1,
        title: "Primary Request and Intent",
        guidance: "Capture every explicit request the user made and what they were trying to achieve. Number them if there were several, and say which one governs the next move. If a request was raised but never acted on, record it as unaddressed rather than dropping it — an omitted request is indistinguishable from a finished one.",
        machine_filled: false,
    },
    SummarySection {
        ordinal: 2,
        title: "Key Technical Concepts",
        guidance: "List the technologies, frameworks, invariants and domain concepts this work depends on, including any non-obvious constraint that was discovered rather than given. The next turn should not have to rediscover them.",
        machine_filled: false,
    },
    SummarySection {
        ordinal: 3,
        title: "Files and Code Sections",
        guidance: "For every file read, created or modified: the path, why it matters, and what changed. Quote the code the next turn would otherwise have to re-read, verbatim — the final working version only. Drop intermediate attempts and superseded drafts.",
        machine_filled: false,
    },
    SummarySection {
        ordinal: 4,
        title: "Errors and fixes",
        guidance: "Every error encountered and how it was resolved, plus every correction the user made. This is negative knowledge and it is the section most often lost: without it the same mistake is repeated verbatim. Include corrections that proved an earlier diagnosis wrong, and say what the real cause turned out to be.",
        machine_filled: false,
    },
    SummarySection {
        ordinal: 5,
        title: "Problem Solving",
        guidance: "Problems already solved and the reasoning that settled them — especially trade-offs deliberately chosen, so the next turn does not silently reopen a closed decision. Also record troubleshooting still in flight.",
        machine_filled: false,
    },
    SummarySection {
        ordinal: 6,
        title: "All user messages",
        guidance: "Filled in by the system with the verbatim text of the user's own messages. Do not write this section yourself and do not restate its content elsewhere.",
        machine_filled: true,
    },
    SummarySection {
        ordinal: 7,
        title: "Pending Tasks",
        guidance: "Work the user explicitly asked for that is not yet done. Do not invent tasks the user never requested, and do not silently drop a task merely because it was raised long ago.",
        machine_filled: false,
    },
    SummarySection {
        ordinal: 8,
        title: "Current Work",
        guidance: "Precisely what was being done when the context ran out: file paths, the exact command or tool call, and the state it was left in. Be specific enough that the work can be resumed mid-step.",
        machine_filled: false,
    },
    SummarySection {
        ordinal: 9,
        title: "Optional Next Step",
        guidance: "The next action, tied to what was explicitly being worked on. Quote the user statement or your own most recent statement that justifies it. If the last task was finished and nothing follows from it, say so plainly instead of inventing a next step.",
        machine_filled: false,
    },
];

/// 返回由程序填充的那一节。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 机器填充节的定义
pub(crate) fn machine_filled_section() -> &'static SummarySection {
    SUMMARY_SECTIONS
        .iter()
        .find(|section| section.machine_filled)
        .expect("目录中必须恰好有一个机器填充节")
}

/// 返回紧跟在机器填充节之后的那一节。
///
/// 程序据此把机器填充节插回正确位置：模型不写第 6 节，
/// 输出里第 5 节之后直接是第 7 节，插入点就是第 7 节的标题行。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 机器填充节的后继节
pub(crate) fn section_after_machine_filled() -> &'static SummarySection {
    let anchor = machine_filled_section();
    SUMMARY_SECTIONS
        .iter()
        .find(|section| section.ordinal == anchor.ordinal + 1)
        .expect("机器填充节之后必须还有小节")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证小节序号从 1 开始且连续。
    ///
    /// 序号断档会让插入锚点算错位置。
    #[test]
    fn ordinals_are_contiguous_from_one() {
        for (index, section) in SUMMARY_SECTIONS.iter().enumerate() {
            assert_eq!(section.ordinal, index + 1);
        }
    }

    /// 验证恰好有一个机器填充节。
    #[test]
    fn exactly_one_section_is_machine_filled() {
        let count = SUMMARY_SECTIONS
            .iter()
            .filter(|section| section.machine_filled)
            .count();

        assert_eq!(count, 1);
    }

    /// 验证机器填充节不是最后一节。
    ///
    /// 插入逻辑依赖它有后继节作为锚点；若它排到末尾，插入将无处可锚。
    #[test]
    fn machine_filled_section_is_not_last() {
        let anchor = machine_filled_section();

        assert!(anchor.ordinal < SUMMARY_SECTIONS.len());
        assert_eq!(section_after_machine_filled().ordinal, anchor.ordinal + 1);
    }

    /// 验证每节都有非空的填写要求。
    #[test]
    fn every_section_carries_guidance() {
        for section in SUMMARY_SECTIONS {
            assert!(!section.guidance.trim().is_empty(), "{}", section.title);
        }
    }
}
