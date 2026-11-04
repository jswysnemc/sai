//! 【命令预览】【语法边界】按命令解释器着色完整源码行，再进行无损视觉折行。

use super::code_block::{highlight_code_line_continued, CodeLineHighlightState};
use super::transcript::{copy_line, AnsiLine};
use crate::platform::shell_selection::{shell_flavor, ShellFlavor};
use serde_json::Value;
use std::ffi::OsStr;

/// 【命令预览】【解释器选择】复用执行器的 Shell 分类，Windows 默认采用 PowerShell。
/// 参数: `arguments` 为完整或流式工具参数；返回: 代码高亮语言标识。
pub(super) fn language(arguments: &str) -> &'static str {
    let shell = serde_json::from_str::<Value>(arguments)
        .ok()
        .and_then(|value| {
            value
                .get("_sai_command_shell")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or_else(|| super::tool_event_line::lenient_string_field(arguments, "_sai_command_shell"));
    match shell
        .as_deref()
        .map(|shell| shell_flavor(OsStr::new(shell)))
    {
        Some(ShellFlavor::Posix) => "sh",
        Some(ShellFlavor::Cmd) => "bat",
        Some(ShellFlavor::PowerShell) => "powershell",
        _ if cfg!(windows) => "powershell",
        _ => "sh",
    }
}

/// 【命令预览】【无损折行】保留源码换行和着色状态，视觉续行只拆分已着色文本。
/// 参数: `command` 为原始命令，`language` 为解释器语法，`width` 为正文列宽；返回: 显示行及复制来源标记。
pub(super) fn display_lines(command: &str, language: &str, width: usize) -> Vec<String> {
    let mut state = CodeLineHighlightState::default();
    let mut lines = Vec::new();
    for raw in command.lines() {
        // 1. 【命令预览】【源码边界】向解析器提供真实行尾，避免注释和转义污染下一源码行
        let source = format!("{raw}\n");
        let highlighted = highlight_code_line_continued(language, &source, &mut state);
        let highlighted = highlighted.replace('\n', "");
        // 2. 【命令预览】【视觉折行】按可见列宽折行，保留全部字符、样式和复制拼接信息
        lines.extend(
            AnsiLine::wrap_block(&highlighted, width)
                .iter()
                .map(|line| copy_line::tag(line.as_str(), line.copy.join_previous)),
        );
    }
    lines
}
