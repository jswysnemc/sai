/// 返回工具启动状态。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 起始状态文本
pub(crate) fn tool_start_status(name: &str) -> &'static str {
    if name == "read_file" {
        "arg"
    } else {
        "run"
    }
}
