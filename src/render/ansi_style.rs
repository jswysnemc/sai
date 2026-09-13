/// 解析 SGR 操作码；参数为样式序列，返回跳过颜色分量后的操作码列表。
fn sgr_commands(sequence: &str) -> Vec<u16> {
    let Some(params) = sequence
        .strip_prefix("\x1b[")
        .and_then(|body| body.strip_suffix('m'))
    else {
        return Vec::new();
    };
    let mut values = params.split(';');
    let mut commands = Vec::new();
    while let Some(value) = values.next() {
        let code = value
            .split(':')
            .next()
            .filter(|part| !part.is_empty())
            .unwrap_or("0");
        let Ok(code) = code.parse::<u16>() else {
            continue;
        };
        commands.push(code);
        // 1. 颜色分量中的 0 和 48 不是 reset 或背景设置；冒号格式的分量已在同一参数中
        if matches!(code, 38 | 48 | 58) && !value.contains(':') {
            let count = match values.next() {
                Some("2") => 3,
                Some("5") => 1,
                _ => 0,
            };
            for _ in 0..count {
                values.next();
            }
        }
    }
    commands
}

/// 判断样式是否重置全部属性；参数为 SGR 序列，返回是否包含独立 reset 操作。
pub(crate) fn is_reset_sgr(sequence: &str) -> bool {
    sgr_commands(sequence).contains(&0)
}

/// 判断样式是否设置背景；参数为 SGR 序列，返回是否包含背景色操作。
pub(crate) fn sgr_sets_background(sequence: &str) -> bool {
    sgr_commands(sequence)
        .into_iter()
        .any(|code| matches!(code, 40..=48 | 100..=107))
}

/// 更新续行样式；参数为当前样式缓冲和新序列，返回值为空。
pub(crate) fn update_active_sgr(active_sgr: &mut String, sequence: &str) {
    if is_reset_sgr(sequence) {
        active_sgr.clear();
    }
    if sequence != "\x1b[m" && sequence != "\x1b[0m" {
        active_sgr.push_str(sequence);
    }
}
