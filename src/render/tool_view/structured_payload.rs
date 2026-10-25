use serde_json::Value;

/// 【终端】【语义载荷】结构化结果转换为字段与列表，普通文本保持原样。
/// 参数: `payload` 为工具参数或结果；返回: 不带 JSON 容器语法的可读正文。
pub(crate) fn format(payload: &str) -> String {
    serde_json::from_str::<Value>(payload)
        .map(|value| format_value(&value))
        .unwrap_or_else(|_| payload.to_string())
}

/// 【终端】【语义字段】递归展示 JSON 数据，保留全部字段和条目内容。
/// 参数: `value` 为解析后的数据；返回: 语义化多行正文。
pub(crate) fn format_value(value: &Value) -> String {
    let mut lines = Vec::new();
    write_value(value, 0, &mut lines);
    lines.join("\n")
}

/// 【终端】【流式载荷】参数尚未闭合时只展示已接收的语义字段，不显示 JSON 碎片。
/// 参数: `preview` 为参数流；返回: 可读进度正文。
pub(crate) fn format_preview(preview: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(preview) {
        return format_value(&value);
    }
    if !preview.trim_start().starts_with(['{', '[']) {
        return preview.to_string();
    }
    let mut fields = serde_json::Map::new();
    for key in [
        "action",
        "path",
        "pattern",
        "query",
        "url",
        "description",
        "text",
        "content",
        "status",
        "name",
        "command",
    ] {
        if let Some(value) = crate::render::tool_event_line::lenient_string_field(preview, key)
            .filter(|value| !value.is_empty())
        {
            fields.insert(key.to_string(), Value::String(value));
        }
    }
    if fields.is_empty() {
        crate::render::terminal_text("Receiving arguments", "正在接收参数").to_string()
    } else {
        format_value(&Value::Object(fields))
    }
}

/// 【终端】【语义层级】对象逐字段展示，数组逐条展示，标量解码为正文。
/// 参数: `value` 为数据，`indent` 为层级列数，`lines` 为输出行；返回: 无。
fn write_value(value: &Value, indent: usize, lines: &mut Vec<String>) {
    match value {
        Value::Object(fields) if !fields.is_empty() => {
            for (key, value) in fields {
                write_field(key, value, indent, lines);
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for item in items {
                let mut entry = Vec::new();
                write_value(item, 0, &mut entry);
                for (position, line) in entry.into_iter().enumerate() {
                    lines.push(format!(
                        "{}{}{line}",
                        " ".repeat(indent),
                        if position == 0 { "• " } else { "  " }
                    ));
                }
            }
        }
        _ => write_text(&scalar(value), indent, "", lines),
    }
}

/// 【终端】【语义字段】容器使用子层级，标量使用标签与正文，长文本续行保持对齐。
/// 参数: `key` 为字段名，`value` 为数据，`indent` 为缩进，`lines` 为输出行；返回: 无。
fn write_field(key: &str, value: &Value, indent: usize, lines: &mut Vec<String>) {
    let label = label(key);
    let container = matches!(value, Value::Object(fields) if !fields.is_empty())
        || matches!(value, Value::Array(items) if !items.is_empty());
    if container {
        lines.push(format!("{}{label}:", " ".repeat(indent)));
        write_value(value, indent + 2, lines);
    } else {
        write_text(&scalar(value), indent, &format!("{label}: "), lines);
    }
}

/// 【终端】【正文续行】保留原始换行，续行按字段正文列缩进。
/// 参数: `text` 为正文，`indent` 为起始列，`prefix` 为首行标签，`lines` 为输出行；返回: 无。
fn write_text(text: &str, indent: usize, prefix: &str, lines: &mut Vec<String>) {
    let continuation = indent + unicode_width::UnicodeWidthStr::width(prefix);
    for (position, line) in text.split('\n').enumerate() {
        lines.push(if position == 0 {
            format!("{}{prefix}{line}", " ".repeat(indent))
        } else {
            format!("{}{line}", " ".repeat(continuation))
        });
    }
}

/// 【终端】【语义标量】去除字符串引号与转义包装，空值和布尔值使用英文语义。
/// 参数: `value` 为标量或空容器；返回: 可读文本。
fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(value) => if *value { "Yes" } else { "No" }.to_string(),
        Value::Null => "None".to_string(),
        Value::Number(number) => number.to_string(),
        Value::Array(_) => "No items".to_string(),
        Value::Object(_) => "No details".to_string(),
    }
}

/// 【终端】【字段标签】常见字段使用语义名称，其他字段分隔下划线、连字符及驼峰。
/// 参数: `key` 为内部字段名；返回: 英文展示标签。
fn label(key: &str) -> String {
    match key {
        "ok" | "success" => return "Success".to_string(),
        "id" => return "ID".to_string(),
        "url" => return "URL".to_string(),
        "cwd" => return "Working directory".to_string(),
        "stdout" => return "Output".to_string(),
        "stderr" => return "Error output".to_string(),
        _ => {}
    }
    let mut output = String::new();
    let mut previous_lower = false;
    for character in key.chars() {
        if matches!(character, '_' | '-') {
            output.push(' ');
            previous_lower = false;
        } else {
            if previous_lower && character.is_uppercase() {
                output.push(' ');
            }
            if output.is_empty() {
                output.extend(character.to_uppercase());
            } else {
                output.extend(character.to_lowercase());
            }
            previous_lower = character.is_lowercase() || character.is_ascii_digit();
        }
    }
    output
}
