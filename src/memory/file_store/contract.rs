use crate::i18n::text as t;

/// 英文版记忆契约。
const CONTRACT_EN: &str = r#"# Memory

You have a persistent, file-based memory. Each memory is one markdown file holding one fact, with an identifier, a one-line description, and a type. The index of existing memories is injected each turn; the index carries only titles and hooks, so read the entry itself with read_memory before acting on it.

Write a memory when something will still hold in a later session:
- user: who the user is — role, expertise, standing preferences
- feedback: how the user wants you to work, and why. Include the reason and how to apply it; without the reason a later turn cannot judge whether it still fits a new situation
- project: ongoing work, goals and constraints that are not derivable from the code or commit history. Convert relative dates to absolute ones
- reference: pointers to external resources — URLs, boards, tickets

One fact per entry. Link related memories with [[identifier]]; a link to an entry that does not exist yet is fine — it marks something worth writing later.

Do not record what the repository already states (code structure, past fixes, commit history, instruction files), or what only matters inside this conversation. If asked to remember one of those, ask what was non-obvious about it and record that instead.

Before writing, check whether an entry already covers the same ground: update that one rather than creating a duplicate. Use delete_memory to remove a memory that turns out to be wrong."#;

/// 中文版记忆契约。
const CONTRACT_ZH: &str = r#"# 记忆

你有一份持久的文件式记忆。每条记忆是一个 markdown 文件，只放一个事实，带标识、一句话摘要与类型。既有记忆的索引每轮都会注入；索引只有标题与提示，据此行动前先用 read_memory 把正文读出来。

跨会话仍然成立的内容才值得写：
- user：用户是谁——角色、专长领域、长期偏好
- feedback：用户要求你怎么工作，以及为什么。理由与应用方式都要写，缺了理由，下一轮无法判断它在新情境下还适不适用
- project：进行中的工作、目标与约束，且无法从代码或提交历史看出。相对时间要换算成具体日期
- reference：外部资源指针——网址、看板、工单

一条记忆只放一个事实。用 [[标识]] 关联其它记忆；指向尚不存在的条目也可以，那标记的是还值得写但没写的线索。

不要记仓库本身已经写明的东西（代码结构、既往修复、提交历史、指令文件），也不要记只在本次对话内有意义的内容。用户要求记这类内容时，先问清其中不显然的那部分再记。

写入前先查有没有已经覆盖同一件事的条目：有就更新那条，不要新建重复的。已被证伪的记忆用 delete_memory 删掉。"#;

/// 返回随系统提示词注入的记忆使用契约。
///
/// 索引本身只说明有哪些记忆，不说明什么时候该写新的。缺这段契约，
/// 记忆库只会被读不会被写，或者被写成对话流水账。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 与当前界面语言匹配的契约文本
pub fn memory_contract() -> &'static str {
    t(CONTRACT_EN, CONTRACT_ZH)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证契约点名了四种类型。
    ///
    /// 少一种就意味着那类内容永远不会被记下来。
    #[test]
    fn every_memory_type_is_named() {
        for kind in ["user", "feedback", "project", "reference"] {
            assert!(CONTRACT_EN.contains(kind), "英文契约缺少 {kind}");
            assert!(CONTRACT_ZH.contains(kind), "中文契约缺少 {kind}");
        }
    }

    /// 验证契约点名了读取与删除工具。
    #[test]
    fn the_lookup_and_delete_tools_are_named() {
        for contract in [CONTRACT_EN, CONTRACT_ZH] {
            assert!(contract.contains("read_memory"));
            assert!(contract.contains("delete_memory"));
        }
    }

    /// 验证契约要求先查重再写。
    ///
    /// 缺这条，同一件事会被反复记成多条互相矛盾的记忆。
    #[test]
    fn duplicates_are_addressed() {
        assert!(CONTRACT_EN.contains("duplicate"));
        assert!(CONTRACT_ZH.contains("重复"));
    }

    /// 验证契约划出了不该记的范围。
    #[test]
    fn the_contract_says_what_not_to_record() {
        assert!(CONTRACT_EN.contains("Do not record"));
        assert!(CONTRACT_ZH.contains("不要记"));
    }
}
