mod buffered;

pub(crate) use buffered::render_buffered_image;

/// 【终端图片】【预览环境】读取协议能力和单元格尺寸，防止缓存跨尺寸错误复用。
/// 参数: 无；返回: 协议能力与渲染上限。
pub(crate) fn preview_environment() -> (bool, bool, bool, (usize, usize, usize, usize)) {
    (
        supports_kitty_graphics(),
        supports_iterm_inline_image(),
        supports_windows_terminal_sixel(),
        kitty_block_limits(),
    )
}

include!("terminal_image/protocol.rs");
include!("terminal_image/escape.rs");
include!("terminal_image/renderers.rs");
include!("terminal_image/raster.rs");
include!("terminal_image/halfblock.rs");
include!("terminal_image/inline_line.rs");
include!("terminal_image/kitty_screens.rs");
include!("terminal_image/reserved_rows.rs");
include!("terminal_image/tests.rs");
