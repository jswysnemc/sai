/// 规范化动态工具名，格式为 `mcp_<server>_<tool>`。
///
/// 参数:
/// - `server_id`: MCP 服务标识
/// - `tool_name`: 原始工具名称
///
/// 返回:
/// - 经过清理并限制长度的动态工具名
pub fn dynamic_tool_name(server_id: &str, tool_name: &str) -> String {
    let server = sanitize_token(server_id);
    let tool = sanitize_token(tool_name);
    let mut name = format!("mcp_{server}_{tool}");
    if name.len() > 64 {
        let hash = {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(name.as_bytes());
            format!("{:x}", digest)[..8].to_string()
        };
        name = format!("mcp_{server}_{hash}");
        name.truncate(64);
    }
    name
}

/// 将标识文本清理为动态工具名可用的 ASCII 片段。
///
/// 参数:
/// - `value`: 原始标识文本
///
/// 返回:
/// - 小写字母、数字和下划线组成的名称片段
fn sanitize_token(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').to_string()
}
