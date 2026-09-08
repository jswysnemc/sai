use mlua::{Lua, Table};

/// 【插件】【文本绑定】提供 URL 编码、HTML 转换和 Unicode 文本处理。
/// @param lua 虚拟机；api 为 sai 表；limit 为 HTML 输入输出字节限制
/// @returns 文本帮助函数安装结果
pub(super) fn install(lua: &Lua, api: &Table, limit: usize) -> mlua::Result<()> {
    let text = lua.create_table()?;
    text.set(
        "trim",
        lua.create_function(|_, value: String| Ok(value.trim().to_string()))?,
    )?;
    text.set(
        "collapse_whitespace",
        lua.create_function(move |_, value: String| {
            if value.len() > limit {
                return Err(mlua::Error::runtime("text input exceeds plugin size limit"));
            }
            Ok(value.split_whitespace().collect::<Vec<_>>().join(" "))
        })?,
    )?;
    text.set(
        "url_encode",
        lua.create_function(|_, value: String| Ok(urlencoding::encode(&value).into_owned()))?,
    )?;
    text.set(
        "html_to_text",
        lua.create_function(move |_, (html, width): (String, Option<usize>)| {
            check_size(&html, limit, "input")?;
            let output = html2text::from_read(html.as_bytes(), width.unwrap_or(120).clamp(20, 200));
            check_size(&output, limit, "output")?;
            Ok(output)
        })?,
    )?;
    text.set(
        "html_to_markdown",
        lua.create_function(move |_, html: String| {
            check_size(&html, limit, "input")?;
            let output = html2md::parse_html(&html);
            check_size(&output, limit, "output")?;
            Ok(output)
        })?,
    )?;
    text.set(
        "clip",
        lua.create_function(move |_, (value, count): (String, usize)| {
            let count = count.min(limit);
            let mut chars = value.chars();
            let clipped = chars.by_ref().take(count).collect::<String>();
            Ok(if chars.next().is_some() {
                format!("{clipped}\n...[truncated]")
            } else {
                clipped
            })
        })?,
    )?;
    api.set("text", text)
}

/// 【插件】【转换限制】校验 HTML 转换前后的 UTF-8 字节数。
/// @param value 待检查文本；limit 为字节上限；stage 区分输入与输出
/// @returns 超限时返回运行时错误
fn check_size(value: &str, limit: usize, stage: &str) -> mlua::Result<()> {
    if value.len() > limit {
        return Err(mlua::Error::runtime(format!(
            "HTML {stage} exceeds plugin size limit"
        )));
    }
    Ok(())
}
