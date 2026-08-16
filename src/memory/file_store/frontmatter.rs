use super::memory_type::MemoryType;

/// 记忆文件头部的元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    /// 短横线分隔的标识，同时是文件名与关联目标
    pub name: String,
    /// 一句话摘要，召回时据此判断相关性
    pub description: String,
    /// 条目类型
    pub memory_type: MemoryType,
}

/// 头部与正文的分隔线。
const FENCE: &str = "---";

/// 把文件内容拆成头部与正文。
///
/// 解析刻意宽容：记忆文件是给人读也给人改的，缺字段时用兜底值继续，
/// 而不是让一处笔误导致整条记忆读不出来。
///
/// 参数:
/// - `content`: 文件全文
///
/// 返回:
/// - （头部，正文）；没有头部围栏时返回 None
pub fn split(content: &str) -> Option<(Frontmatter, String)> {
    let trimmed = content.trim_start_matches('\u{feff}');
    let mut lines = trimmed.lines();
    if lines.next()?.trim() != FENCE {
        return None;
    }
    let mut header = Vec::new();
    let mut closed = false;
    for line in lines.by_ref() {
        if line.trim() == FENCE {
            closed = true;
            break;
        }
        header.push(line);
    }
    if !closed {
        return None;
    }
    let body = lines.collect::<Vec<_>>().join("\n");
    Some((parse_header(&header), body.trim().to_string()))
}

/// 解析头部各字段。
///
/// 参数:
/// - `lines`: 头部行
///
/// 返回:
/// - 解析出的元数据，缺失字段取兜底值
fn parse_header(lines: &[&str]) -> Frontmatter {
    let mut name = String::new();
    let mut description = String::new();
    let mut memory_type = None;
    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim();
        match key {
            "name" => name = value.to_string(),
            "description" => description = value.to_string(),
            // type 嵌在 metadata 之下，缩进不参与判定，键名唯一即可
            "type" => memory_type = MemoryType::parse(value),
            _ => {}
        }
    }
    Frontmatter {
        name,
        description,
        memory_type: memory_type.unwrap_or(MemoryType::Project),
    }
}

/// 把元数据与正文渲染成完整文件内容。
///
/// 参数:
/// - `front`: 头部元数据
/// - `body`: 正文
///
/// 返回:
/// - 可直接落盘的文件内容
pub fn render(front: &Frontmatter, body: &str) -> String {
    format!(
        "{FENCE}\nname: {}\ndescription: {}\nmetadata:\n  type: {}\n{FENCE}\n\n{}\n",
        front.name,
        single_line(&front.description),
        front.memory_type.as_str(),
        body.trim()
    )
}

/// 把摘要压成一行。
///
/// 摘要跨行会破坏头部结构，读回来时第二行会被当成未知键丢弃。
///
/// 参数:
/// - `value`: 原始摘要
///
/// 返回:
/// - 单行摘要
fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证完整文件可以拆出头部与正文。
    #[test]
    fn splits_a_well_formed_file() {
        let content = "---\nname: zh-writing\ndescription: 中文书写规范\nmetadata:\n  type: feedback\n---\n\n正文内容";

        let (front, body) = split(content).unwrap();

        assert_eq!(front.name, "zh-writing");
        assert_eq!(front.description, "中文书写规范");
        assert_eq!(front.memory_type, MemoryType::Feedback);
        assert_eq!(body, "正文内容");
    }

    /// 验证渲染后能原样读回。
    #[test]
    fn rendering_round_trips_through_parsing() {
        let front = Frontmatter {
            name: "pnpm-over-npm".to_string(),
            description: "Node 项目一律用 pnpm".to_string(),
            memory_type: MemoryType::Feedback,
        };

        let (parsed, body) = split(&render(&front, "正文")).unwrap();

        assert_eq!(parsed, front);
        assert_eq!(body, "正文");
    }

    /// 验证摘要里的冒号不会截断取值。
    ///
    /// 「原因：xxx」这种摘要很常见，按第一个冒号切会把它腰斩。
    #[test]
    fn a_colon_inside_the_description_is_preserved() {
        let content =
            "---\nname: a\ndescription: 原因: 供应商缓存\nmetadata:\n  type: project\n---\n正文";

        let (front, _) = split(content).unwrap();

        assert_eq!(front.description, "原因: 供应商缓存");
    }

    /// 验证缺少类型时退到项目类型而不是整条读不出来。
    #[test]
    fn a_missing_type_falls_back_instead_of_failing() {
        let content = "---\nname: a\ndescription: b\n---\n正文";

        let (front, _) = split(content).unwrap();

        assert_eq!(front.memory_type, MemoryType::Project);
    }

    /// 验证没有头部围栏时不产生结果。
    #[test]
    fn content_without_a_fence_is_rejected() {
        assert!(split("没有头部的纯文本").is_none());
    }

    /// 验证围栏未闭合时不产生结果。
    ///
    /// 半截头部继续解析会把正文当成字段读进来。
    #[test]
    fn an_unclosed_fence_is_rejected() {
        assert!(split("---\nname: a\n正文").is_none());
    }

    /// 验证渲染时把多行摘要压成一行。
    #[test]
    fn a_multiline_description_is_folded() {
        let front = Frontmatter {
            name: "a".to_string(),
            description: "第一行\n第二行".to_string(),
            memory_type: MemoryType::User,
        };

        let rendered = render(&front, "正文");

        assert!(rendered.contains("description: 第一行 第二行"));
        assert!(split(&rendered).is_some());
    }

    /// 验证正文里的分隔线不被当作围栏。
    #[test]
    fn a_horizontal_rule_in_the_body_survives() {
        let content =
            "---\nname: a\ndescription: b\nmetadata:\n  type: user\n---\n上段\n\n---\n\n下段";

        let (_, body) = split(content).unwrap();

        assert!(body.contains("上段"));
        assert!(body.contains("下段"));
    }
}
