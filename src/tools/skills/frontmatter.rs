/// 技能文档头部元数据，名称缺失时由调用方回退到目录名
#[derive(Default)]
pub(crate) struct SkillMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// 【技能目录】【元数据解析】读取 YAML 顶层名称和简介，整理为单行展示文本
///
/// 参数：
/// - `raw`：完整 SKILL.md 文本，支持 UTF-8 BOM 和 CRLF 换行
///
/// 返回：解析后的元数据；头部缺失、未闭合或 YAML 无效时返回空字段
pub(crate) fn parse_skill_frontmatter(raw: &str) -> SkillMetadata {
    // 1. 【技能目录】【元数据解析】只提取闭合头部，避免把正文中的字段当成元数据
    let Some(document) = frontmatter_document(raw) else {
        return SkillMetadata::default();
    };
    let Ok(value) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(document) else {
        return SkillMetadata::default();
    };

    // 2. 【技能目录】【元数据解析】由 YAML 解析器处理折叠、保留换行、引号和注释，再压平显示空白
    SkillMetadata {
        name: display_scalar(value.get("name")),
        description: display_scalar(value.get("description")),
    }
}

/// 提取 Markdown 开头的 YAML 文档，不读取结束分隔线后的正文
///
/// 参数：`raw` 为完整技能文档
/// 返回：头部 YAML 切片；缺少任一分隔线时返回 None
fn frontmatter_document(raw: &str) -> Option<&str> {
    let raw = raw.trim_start_matches('\u{feff}');
    let mut lines = raw.split_inclusive('\n');
    let opening = lines.next()?;
    if opening.trim_end() != "---" {
        return None;
    }
    let start = opening.len();
    let mut end = start;
    for line in lines {
        if matches!(line.trim_end(), "---" | "...") {
            return Some(&raw[start..end]);
        }
        end += line.len();
    }
    None
}

/// 将 YAML 字符串整理为目录、补全菜单和提示词可共用的单行文本
///
/// 参数：`value` 为顶层字段值
/// 返回：非空字符串；空值、集合和非字符串标量返回 None
fn display_scalar(value: Option<&serde_yaml_ng::Value>) -> Option<String> {
    let text = value?
        .as_str()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}
