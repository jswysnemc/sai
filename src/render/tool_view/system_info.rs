use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【系统信息】用系统摘要和分组明细展示只读环境检查，不输出重复协议字段。
/// 参数: 工具视图及显示模式；返回: 可识别的系统信息卡片。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "check_os_info" {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    if value["ok"] == false {
        return None;
    }
    let system = system_name(&value)?;
    let arch = result_card::text(&value, "arch").unwrap_or_default();
    let title = format!(
        "Checked OS info · {system}{}",
        if arch.is_empty() {
            String::new()
        } else {
            format!(" · {arch}")
        }
    );
    let mut body = vec!["\x1b[1mSystem\x1b[0m".into(), format!("  OS: {system}")];
    // 1. 【终端】【系统摘要】常用系统与会话字段置前，缺失字段不占行
    for (key, label) in [
        ("arch", "Architecture"),
        ("hostname", "Host"),
        ("username", "User"),
    ] {
        field(&mut body, &value, key, label);
    }
    if let Some(packages) = value["package_manager_guess"].as_array() {
        let packages = packages
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        if !packages.is_empty() {
            body.push(format!("  Packages: {packages}"));
        }
    }
    if let Some(env) = value
        .get("env")
        .filter(|env| env.as_object().is_some_and(|env| !env.is_empty()))
    {
        body.push("\x1b[1mSession\x1b[0m".into());
        for (key, label) in [
            ("SHELL", "Shell"),
            ("COMSPEC", "Command shell"),
            ("TERM", "Terminal"),
            ("XDG_CURRENT_DESKTOP", "Desktop"),
            ("XDG_SESSION_TYPE", "Display protocol"),
            ("LANG", "Locale"),
        ] {
            field(&mut body, env, key, label);
        }
    }
    // 2. 【终端】【系统诊断】原始诊断信息独立分组，供展开回看，避免长 PATH 淹没摘要
    let mut diagnostics = value.clone();
    if let Some(fields) = diagnostics.as_object_mut() {
        fields.retain(|key, value| {
            !matches!(
                key.as_str(),
                "ok" | "success" | "arch" | "hostname" | "username" | "package_manager_guess"
            ) && !value.is_null()
        });
        if let Some(env) = fields.get_mut("env").and_then(Value::as_object_mut) {
            env.retain(|key, _| {
                !matches!(
                    key.as_str(),
                    "SHELL"
                        | "COMSPEC"
                        | "TERM"
                        | "XDG_CURRENT_DESKTOP"
                        | "XDG_SESSION_TYPE"
                        | "LANG"
                )
            });
            if env.is_empty() {
                fields.remove("env");
            }
        }
    }
    if diagnostics
        .as_object()
        .is_some_and(|fields| !fields.is_empty())
    {
        body.push("\x1b[2mDiagnostics\x1b[0m".into());
        body.extend(
            super::structured_payload::format_value(&diagnostics)
                .lines()
                .map(|line| {
                    format!(
                        "  \x1b[2m{}\x1b[0m",
                        crate::render::ansi_clip::strip_ansi(line)
                    )
                }),
        );
    }
    Some(result_card::render(
        view,
        mode,
        &title,
        ToolHealth::Ok,
        &body.join("\n"),
    ))
}

/// 【终端】【系统名称】优先读取发行版名称，兼容 macOS 和 Windows 的平台回退。
/// 参数: 系统信息对象；返回: 不含控制字符的单行名称。
fn system_name(value: &Value) -> Option<String> {
    let release = value["os_release"].as_str().unwrap_or_default();
    let pretty = ["PRETTY_NAME=", "NAME="].into_iter().find_map(|key| {
        release
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(|name| name.trim_matches(['\'', '"']).to_string())
    });
    pretty
        .or_else(|| {
            let name = result_card::text(&value["macos"], "product_name")?;
            let version = result_card::text(&value["macos"], "product_version").unwrap_or_default();
            Some(format!("{name} {version}").trim().into())
        })
        .or_else(|| result_card::text(value, "platform"))
        .or_else(|| result_card::text(value, "os"))
        .map(|name| crate::render::ansi_clip::strip_ansi(&name).replace(['\n', '\r', '\t'], " "))
}

/// 【终端】【信息字段】追加非空字段，统一标签缩进与正文样式。
/// 参数: 输出行、对象、字段名及标签；返回: 无。
fn field(lines: &mut Vec<String>, value: &Value, key: &str, label: &str) {
    if let Some(text) = result_card::text(value, key) {
        lines.push(format!("  {label}: {text}"));
    }
}
