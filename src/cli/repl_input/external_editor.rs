use super::*;

/// 【终端】【外部编辑】展开文本占位块后编辑，恢复图片锚点及附件登记
/// 参数: input 为输入草稿，clipboard 为附件状态；返回编辑后的输入，失败时为空
pub(super) fn edit(input: &str, clipboard: &mut ReplClipboardState) -> Option<String> {
    let buffer = crate::cli::repl_editor_buffer::prepare_editor_buffer(input, clipboard);
    let had_text_blocks = clipboard.has_text_blocks(input);
    match edit_input_buffer(&buffer.text) {
        Ok(edited) => {
            let cleaned = strip_terminal_control_sequences(&edited);
            let result = crate::cli::repl_editor_buffer::restore_editor_buffer(&buffer, &cleaned);
            if had_text_blocks {
                clipboard.forget_text_blocks();
            }
            Some(result)
        }
        Err(error) => {
            eprintln!("{error}");
            None
        }
    }
}
