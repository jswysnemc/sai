use super::index_file::IndexDocument;
use super::library::FileMemoryLibrary;
use std::path::Path;

/// 注入文本的包裹标签。
const WRAPPER: &str = "memory";

/// 读取指定目录下的索引并渲染为注入文本。
///
/// 参数:
/// - `base`: 记忆根目录，已按人格隔离
/// - `workspace`: 当前工作区路径；无工作区时只注入全局记忆
///
/// 返回:
/// - 注入文本；没有任何记忆时为 None
pub fn render_index_injection_for(base: &Path, workspace: Option<&str>) -> Option<String> {
    let library = FileMemoryLibrary::new(base, workspace.map(Path::new));
    let (project, global) = library.index_contents();
    render_index_injection(&project, &global)
}

/// 把两份索引渲染成注入上下文的文本。
///
/// 注入的是索引而不是正文：索引每条只占一行，全量带上也不过几百字符，
/// 而按相关性挑选正文的做法会让分数低于阈值的记忆彻底消失——那正是
/// 「明明记过却没生效」的来源。正文改为按需读取。
///
/// 参数:
/// - `project_index`: 当前工作区的索引正文
/// - `global_index`: 全局索引正文
///
/// 返回:
/// - 注入文本；两份索引都没有条目时为 None
fn render_index_injection(project_index: &str, global_index: &str) -> Option<String> {
    let project = IndexDocument::parse(project_index);
    let global = IndexDocument::parse(global_index);
    if project.entries().is_empty() && global.entries().is_empty() {
        return None;
    }
    let mut output = format!("<{WRAPPER}>\n");
    output.push_str(
        "以下是既有记忆的索引，每行一条。这些是此前确认过的事实与要求，本轮需要遵循。\n\
         需要某条的完整内容时，用 read_memory 工具按标识读取正文，不要凭标题猜测。\n",
    );
    // 项目记忆排在前面：同名时它覆盖全局那条，先看到的才是生效的那条
    for (title, document) in [("项目记忆", &project), ("全局记忆", &global)] {
        let entries = document.entries();
        if entries.is_empty() {
            continue;
        }
        output.push_str(&format!("\n{title}：\n"));
        for entry in entries {
            output.push_str(&format!("{}\n", entry.render()));
        }
    }
    output.push_str(&format!("</{WRAPPER}>"));
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证两份索引都为空时不产生注入。
    #[test]
    fn renders_nothing_without_entries() {
        assert!(render_index_injection("", "").is_none());
        assert!(render_index_injection("# 标题\n\n没有条目", "").is_none());
    }

    /// 验证每条索引都出现在注入文本里。
    ///
    /// 全量注入是这套方案与相关性检索的根本差别，漏一条就退化回旧行为。
    #[test]
    fn every_pointer_appears() {
        let index = "- [一](a.md) — 提示一\n- [二](b.md) — 提示二";

        let rendered = render_index_injection(index, "").unwrap();

        assert!(rendered.contains("a.md"));
        assert!(rendered.contains("b.md"));
        assert!(rendered.contains("提示一"));
    }

    /// 验证项目记忆排在全局之前。
    #[test]
    fn project_memories_come_first() {
        let rendered = render_index_injection("- [项目](p.md)", "- [全局](g.md)").unwrap();

        assert!(rendered.find("p.md").unwrap() < rendered.find("g.md").unwrap());
    }

    /// 验证注入措辞不自我否定。
    ///
    /// 「可能相关也可能不相关」这类措辞等于教模型忽略这段内容。
    #[test]
    fn the_wording_does_not_undermine_itself() {
        let rendered = render_index_injection("- [一](a.md)", "").unwrap();

        assert!(!rendered.contains("可能相关"));
        assert!(rendered.contains("本轮需要遵循"));
    }

    /// 验证注入指明了读取正文的方式。
    ///
    /// 只给索引不说怎么展开，模型只能按标题猜内容。
    #[test]
    fn the_injection_names_the_lookup_tool() {
        let rendered = render_index_injection("- [一](a.md)", "").unwrap();

        assert!(rendered.contains("read_memory"));
    }

    /// 验证空的一侧不产生孤立标题。
    #[test]
    fn an_empty_side_produces_no_heading() {
        let rendered = render_index_injection("", "- [全局](g.md)").unwrap();

        assert!(!rendered.contains("项目记忆"));
        assert!(rendered.contains("全局记忆"));
    }
}
