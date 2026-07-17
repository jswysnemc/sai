use crate::i18n::text as t;
use crate::render::background_command_event::background_command_block_action;
use crate::render::command_output::{render_command_block_with_action, write_tool_payload};
use crate::render::edit_diff::write_edit_file_diff_block;
use crate::render::streaming_command_block::StreamingCommandBlock;
use crate::render::streaming_replace::clear_rendered_rows;
use anyhow::Result;
use std::collections::HashSet;
use std::io::{self, Write};

/// 写入命令类工具调用块。
///
/// 参数:
/// - `name`: 工具名称
/// - `arguments`: 工具参数
/// - `event_label`: 工具事件标签
/// - `background_command_start`: 是否为后台命令启动
/// - `streaming_command_block`: 命令参数流式预览状态
/// - `command_block_tools`: 已渲染命令块工具集合
///
/// 返回:
/// - 是否已经写入命令块
pub(crate) fn write_command_tool_call_block(
    name: &str,
    arguments: &str,
    event_label: &str,
    background_command_start: bool,
    streaming_command_block: &mut StreamingCommandBlock,
    command_block_tools: &mut HashSet<String>,
) -> Result<bool> {
    if name != "run_command" && !background_command_start {
        return Ok(false);
    }
    command_block_tools.insert(name.to_string());
    let mut stdout = io::stdout();
    let command_block_action = if background_command_start {
        background_command_block_action()
    } else {
        event_label
    };
    if name == "run_command" {
        write!(
            stdout,
            "{}",
            clear_rendered_rows(streaming_command_block.take_rendered_rows())
        )?;
    }
    write!(
        stdout,
        "{}",
        render_command_block_with_action(arguments, command_block_action)
    )?;
    stdout.flush()?;
    Ok(true)
}

/// 写入编辑文件工具调用块。
///
/// 参数:
/// - `name`: 工具名称
/// - `arguments`: 工具参数
///
/// 返回:
/// - 是否已经写入编辑块
pub(crate) fn write_edit_tool_call_block(name: &str, arguments: &str) -> Result<bool> {
    if name != "edit_file" {
        return Ok(false);
    }
    let mut stdout = io::stdout();
    if !write_edit_file_diff_block(&mut stdout, arguments)? {
        write_tool_payload(&mut stdout, t("args", "参数"), arguments)?;
    }
    stdout.flush()?;
    Ok(true)
}

/// 仅在能够渲染 diff 时写入编辑文件工具调用块。
///
/// 参数:
/// - `name`: 工具名称
/// - `arguments`: 工具参数
///
/// 返回:
/// - 是否已经写入 diff 块
pub(crate) fn write_edit_tool_call_diff_block(name: &str, arguments: &str) -> Result<bool> {
    if name != "edit_file" {
        return Ok(false);
    }
    let mut stdout = io::stdout();
    let rendered = write_edit_file_diff_block(&mut stdout, arguments)?;
    if rendered {
        stdout.flush()?;
    }
    Ok(rendered)
}
