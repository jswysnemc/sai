use super::unified_diff::parse_unified_diff;
use crate::render::edit_diff::format_diff_stat_status;
use serde_json::Value;

/// 编辑工具结果解析出的可渲染 diff 报告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditResultReport {
    /// 与工具输出同源的 diff 正文（已按 transcript 样式渲染）
    pub rendered: String,
    /// 与正文画出的 `+` / `-` 行一致的 `+N -M` 统计
    pub stats: Option<String>,
}

/// 从编辑工具结果 JSON 解析 diff 报告。
///
/// `write_file` / `str_replace` 把 unified diff 放在输出 JSON 的顶层
/// `diff` 键（`changed_files` 条目内只有 action/path/added/removed 统计）；
/// 条目内嵌 `diff` 的旧格式也兼容。TUI 消费事件时文件可能已经写完，
/// 按参数重建预览会得到空差异，因此定稿快照必须改从结果报告恢复。
///
/// 参数:
/// - `output`: 工具原始输出
///
/// 返回:
/// - 成功输出里的 diff 报告；解析不到时返回空
pub(crate) fn parse_edit_result(output: &str) -> Option<EditResultReport> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    let files = value.get("changed_files")?.as_array()?;
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut body = String::new();
    for file in files {
        added += file.get("added").and_then(Value::as_u64).unwrap_or(0) as usize;
        removed += file.get("removed").and_then(Value::as_u64).unwrap_or(0) as usize;
        // 真实工具把 diff 放在顶层；条目内嵌是历史格式
        let diff = file
            .get("diff")
            .and_then(Value::as_str)
            .or_else(|| value.get("diff").and_then(Value::as_str))?;
        let patch = parse_unified_diff(diff)?;
        body.push_str(&render_patch(&patch));
        body.push('\n');
    }
    let rendered = body.trim_end().to_string();
    if rendered.is_empty() {
        return None;
    }
    let stats = Some(format_diff_stat_status(added, removed));
    Some(EditResultReport { rendered, stats })
}

/// 渲染单文件补丁为 transcript 正文。
///
/// 正文复用 CLI diff 的配色与行号结构；写入 `DiffCell::rendered` 后由
/// `strip_leading_bullet_header` 与折叠逻辑统一处理标题与超长正文。
///
/// 参数:
/// - `patch`: 从 unified diff 解析出的文件补丁
///
/// 返回:
/// - 带 ANSI 样式的行级 diff 文本
fn render_patch(patch: &super::unified_diff::UnifiedFilePatch) -> String {
    crate::render::edit_diff::render_unified_patch_for_transcript(patch)
}
