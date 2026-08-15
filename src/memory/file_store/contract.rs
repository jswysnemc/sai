use crate::i18n::text as t;

/// 英文版记忆契约。
const CONTRACT_EN: &str = r#"# Memory

You have a persistent, file-based memory. Each memory is one markdown file holding one fact, with an identifier, a one-line description, and a type. The index of existing memories is injected whenever it changes; it carries only titles and hooks, so read the entry itself with read_memory before acting on it.

Call write_memory when something will still hold in a later session:
- user: who the user is — role, expertise, standing preferences
- feedback: how the user wants you to work, and why
- project: ongoing work, goals and constraints that are not derivable from the code or commit history. Convert relative dates to absolute ones
- reference: pointers to external resources — URLs, boards, tickets

feedback and project entries must carry a `**Why:**` line and a `**How to apply:**` line. Without the reason a later turn cannot judge whether the entry still fits a new situation.

Set scope to project — the default — for anything tied to the current working directory; use global only for what holds across every project.

One fact per entry. Link related memories with [[identifier]]; a link to an entry that does not exist yet is fine — it marks something worth writing later.

Do not record what the repository already states (code structure, past fixes, commit history, instruction files), or what only matters inside this conversation. If asked to remember one of those, ask what was non-obvious about it and record that instead.

Before writing, check whether an entry already covers the same ground: writing an existing identifier updates it in place, which is what you want instead of a near-duplicate. Use delete_memory to remove a memory that turns out to be wrong."#;

/// 中文版记忆契约。
const CONTRACT_ZH: &str = r#"# 记忆

你有一份持久的文件式记忆。每条记忆是一个 markdown 文件，只放一个事实，带标识、一句话摘要与类型。既有记忆的索引在内容变化时注入；索引只有标题与提示，据此行动前先用 read_memory 把正文读出来。

跨会话仍然成立的内容才值得写，用 write_memory 写入：
- user：用户是谁——角色、专长领域、长期偏好
- feedback：用户要求你怎么工作，以及为什么
- project：进行中的工作、目标与约束，且无法从代码或提交历史看出。相对时间要换算成具体日期
- reference：外部资源指针——网址、看板、工单

feedback 与 project 两类的正文必须写上 `**Why:**` 与 `**How to apply:**` 两行。缺了理由，下一轮无法判断它在新情境下还适不适用。

scope 取 project（默认）表示这条只跟当前工作目录有关；确实跨项目成立才用 global。

一条记忆只放一个事实。用 [[标识]] 关联其它记忆；指向尚不存在的条目也可以，那标记的是还值得写但没写的线索。

不要记仓库本身已经写明的东西（代码结构、既往修复、提交历史、指令文件），也不要记只在本次对话内有意义的内容。用户要求记这类内容时，先问清其中不显然的那部分再记。

写入前先查有没有已经覆盖同一件事的条目：写入同一个标识就是就地更新，这比新建一条近似的更合适。已被证伪的记忆用 delete_memory 删掉。"#;

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

    /// 验证契约点名了三个记忆工具。
    ///
    /// 漏掉 write_memory 尤其致命：契约教了该记什么却没说用什么记，
    /// 模型只能从工具列表里自己猜。
    #[test]
    fn every_memory_tool_is_named() {
        for contract in [CONTRACT_EN, CONTRACT_ZH] {
            assert!(contract.contains("write_memory"));
            assert!(contract.contains("read_memory"));
            assert!(contract.contains("delete_memory"));
        }
    }

    /// 验证契约说明了作用域怎么选。
    ///
    /// 不说明就会把项目专属的事实写成全局记忆，污染其它工作区。
    #[test]
    fn the_scope_choice_is_explained() {
        for contract in [CONTRACT_EN, CONTRACT_ZH] {
            assert!(contract.contains("project"));
            assert!(contract.contains("global"));
        }
    }

    /// 验证契约给出了工具实际校验的两个标记。
    ///
    /// write_memory 按字面查 Why 与 How to apply，契约不写明格式，
    /// 模型每次写 feedback 都会拿到一条补写提示。
    #[test]
    fn the_rationale_markers_are_spelled_out() {
        for contract in [CONTRACT_EN, CONTRACT_ZH] {
            assert!(contract.contains("**Why:**"));
            assert!(contract.contains("**How to apply:**"));
        }
    }

    /// 验证契约要求先查重再写。
    ///
    /// 缺这条，同一件事会被反复记成多条互相矛盾的记忆。
    #[test]
    fn duplicates_are_addressed() {
        assert!(CONTRACT_EN.contains("near-duplicate"));
        assert!(CONTRACT_ZH.contains("就地更新"));
    }

    /// 验证契约划出了不该记的范围。
    #[test]
    fn the_contract_says_what_not_to_record() {
        assert!(CONTRACT_EN.contains("Do not record"));
        assert!(CONTRACT_ZH.contains("不要记"));
    }
}
