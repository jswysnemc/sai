use crate::render::table::CellContent;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use crossterm::terminal;
use std::fs;
#[cfg(test)]
use std::fs::File;
#[cfg(test)]
use std::io::BufWriter;
use std::path::Path;
use std::process::{Command, Stdio};

const KITTY_CHUNK_SIZE: usize = 4096;
const ANSI_ALPHA_THRESHOLD: u8 = 16;
const SIXEL_CELL_WIDTH_PX: usize = 8;
const SIXEL_CELL_HEIGHT_PX: usize = 16;
const SIXEL_MAX_WIDTH_PX: usize = 1600;
const SIXEL_MAX_HEIGHT_PX: usize = 1200;
const SIXEL_COLOR_STEPS: [u8; 6] = [0, 51, 102, 153, 204, 255];
const ANSI_FALLBACK_BG: Rgba = Rgba {
    r: 11,
    g: 16,
    b: 32,
    a: 255,
};

/// 查询终端单元格的像素尺寸。
///
/// 通过 `ioctl(TIOCGWINSIZE)` 获取终端窗口总像素尺寸，
/// 除以字符行列数得到单个单元格的像素宽高。
/// 不支持时回退到 8×16。
#[cfg(unix)]
fn terminal_cell_pixel_size() -> (usize, usize) {
    use std::os::unix::io::AsRawFd;

    const TIOCGWINSIZE: libc::c_ulong = 0x5413;

    #[repr(C)]
    struct Winsize {
        ws_row: u16,
        ws_col: u16,
        ws_xpixel: u16,
        ws_ypixel: u16,
    }

    let fd = std::io::stdout().as_raw_fd();
    let mut ws = Winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let ret = unsafe { libc::ioctl(fd, TIOCGWINSIZE, &mut ws) };
    if ret == 0 && ws.ws_col > 0 && ws.ws_row > 0 && ws.ws_xpixel > 0 && ws.ws_ypixel > 0 {
        let cw = ws.ws_xpixel as usize / ws.ws_col as usize;
        let ch = ws.ws_ypixel as usize / ws.ws_row as usize;
        if cw > 0 && ch > 0 {
            return (cw, ch);
        }
    }
    (SIXEL_CELL_WIDTH_PX, SIXEL_CELL_HEIGHT_PX)
}

#[cfg(not(unix))]
fn terminal_cell_pixel_size() -> (usize, usize) {
    (SIXEL_CELL_WIDTH_PX, SIXEL_CELL_HEIGHT_PX)
}

#[derive(Clone, Copy, Debug)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Clone, Copy, Debug, Default)]
struct TerminalImageSize {
    width_cells: Option<usize>,
    height_cells: Option<usize>,
}

/// 将图片渲染为当前终端可显示的文本。
///
/// 参数:
/// - `path`: 图片文件路径
///
/// 返回:
/// - 终端图片协议文本或 chafa 文本输出
pub(crate) fn render_terminal_image(path: &Path) -> Result<String> {
    render_terminal_image_with_size(path, None)
}

/// 将图片按可选终端单元格尺寸渲染为当前终端可显示的文本。
///
/// 参数:
/// - `path`: 图片文件路径
/// - `size`: 可选尺寸，格式同 chafa `WIDTHxHEIGHT`，允许省略一边
///
/// 返回:
/// - 终端图片协议文本、chafa 文本输出，或 ANSI 半块降级文本
pub(crate) fn render_terminal_image_with_size(path: &Path, size: Option<&str>) -> Result<String> {
    let parsed_size = TerminalImageSize::parse(size);
    if supports_kitty_graphics() {
        return render_kitty_image(path);
    }
    if supports_iterm_inline_image() {
        return render_iterm_image(path);
    }
    if supports_windows_terminal_sixel() {
        return render_sixel_image(path, &parsed_size)
            .or_else(|_| render_ansi_halfblock_image(path, &parsed_size));
    }
    render_chafa_image(path, size, &parsed_size)
        .or_else(|_| render_ansi_halfblock_image(path, &parsed_size))
}

impl TerminalImageSize {
    /// 解析终端图片尺寸。
    ///
    /// 参数:
    /// - `value`: `WIDTHxHEIGHT`、`WIDTHx` 或 `xHEIGHT`
    ///
    /// 返回:
    /// - 已解析的尺寸约束
    fn parse(value: Option<&str>) -> Self {
        let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
            return Self::default();
        };
        let Some((width, height)) = value.split_once('x') else {
            return Self::default();
        };
        Self {
            width_cells: parse_cell_count(width),
            height_cells: parse_cell_count(height),
        }
    }
}

/// 解析正整数终端单元格数量。
///
/// 参数:
/// - `value`: 数字文本
///
/// 返回:
/// - 有效正整数
fn parse_cell_count(value: &str) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
}

/// 判断当前终端是否支持 Kitty 图形协议。
///
/// 返回:
/// - 是否支持 Kitty 图形协议
fn supports_kitty_graphics() -> bool {
    std::env::var_os("KITTY_WINDOW_ID").is_some()
        || std::env::var("TERM")
            .map(|term| term.contains("xterm-kitty"))
            .unwrap_or(false)
}

/// 判断当前终端是否支持 iTerm2 图片协议。
///
/// 返回:
/// - 是否支持 iTerm2 图片协议
fn supports_iterm_inline_image() -> bool {
    std::env::var("TERM_PROGRAM")
        .map(|program| matches!(program.as_str(), "iTerm.app" | "WezTerm"))
        .unwrap_or(false)
}

/// 判断当前终端是否支持 Windows Terminal 使用的 Sixel 图形协议。
///
/// 返回:
/// - 是否可能支持 Sixel 图形协议
fn supports_windows_terminal_sixel() -> bool {
    std::env::var_os("WT_SESSION").is_some()
        || std::env::var("TERM_PROGRAM")
            .map(|program| program == "Windows_Terminal")
            .unwrap_or(false)
}

/// 块级 Kitty 图在终端中的最大占位（列/行）。
///
/// 仅作「超出才缩小」的上限，不主动压到半屏（避免图本身变小，却仍留下空白）。
///
/// 返回:
/// - `(max_cols, max_rows, cell_pw, cell_ph)`
fn kitty_block_limits() -> (usize, usize, usize, usize) {
    let (cell_pw, cell_ph) = terminal_cell_pixel_size();
    let (cell_pw, cell_ph) = normalize_mono_cell_pixels(cell_pw, cell_ph);
    let (term_cols, term_rows) = terminal::size()
        .map(|(cols, rows)| (usize::from(cols), usize::from(rows)))
        .unwrap_or((80, 24));
    let max_cols = term_cols.saturating_sub(2).max(1);
    // 几乎可用全高，只留一点边距；真正空白靠「按比例算 r」消除
    let max_rows = term_rows.saturating_sub(2).clamp(4, 120);
    (max_cols, max_rows, cell_pw, cell_ph)
}

/// 计算 Kitty 块级图片的列数，以及与图片宽高比一致的行数。
///
/// 行数由列宽反推，避免 `ceil(宽)` 与 `ceil(高)` 各自取整后 `r` 偏大，
/// 在图下方留下一块「空白占位」。
///
/// 参数:
/// - `pixel_width`: 图片像素宽度
/// - `pixel_height`: 图片像素高度
///
/// 返回:
/// - `(列数 c, 行数 r)`
fn kitty_cell_dimensions(pixel_width: usize, pixel_height: usize) -> (usize, usize) {
    let (max_cols, max_rows, cell_pw, cell_ph) = kitty_block_limits();
    let pixel_width = pixel_width.max(1);
    let pixel_height = pixel_height.max(1);
    let cell_pw = cell_pw.max(1);
    let cell_ph = cell_ph.max(1);

    // 1. 先定列宽（不超过终端）
    let mut cols = pixel_width.div_ceil(cell_pw).max(1).min(max_cols);
    // 2. 行高严格按宽高比：r = ceil(h * c * cell_pw / (w * cell_ph))
    let mut rows = pixel_height
        .saturating_mul(cols)
        .saturating_mul(cell_pw)
        .div_ceil(pixel_width.saturating_mul(cell_ph).max(1))
        .max(1);

    // 3. 仅当确实超出终端高度时再压列宽
    if rows > max_rows {
        rows = max_rows;
        cols = pixel_width
            .saturating_mul(rows)
            .saturating_mul(cell_ph)
            .div_ceil(pixel_height.saturating_mul(cell_pw).max(1))
            .max(1)
            .min(max_cols);
        rows = pixel_height
            .saturating_mul(cols)
            .saturating_mul(cell_pw)
            .div_ceil(pixel_width.saturating_mul(cell_ph).max(1))
            .max(1)
            .min(max_rows);
    }
    (cols, rows)
}

/// 编码 Kitty 图形协议载荷（不含光标占位换行）。
///
/// 参数:
/// - `path`: 图片文件路径
/// - `cols`: 可选显示列数
/// - `rows`: 可选显示行数
///
/// 返回:
/// - Kitty 图形协议转义序列
fn encode_kitty_png(path: &Path, cols: Option<usize>, rows: Option<usize>) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read image {}", path.display()))?;
    let encoded = general_purpose::STANDARD.encode(bytes);
    // 1. 组装尺寸与静默参数，避免 Kitty 回写响应干扰 REPL
    let mut control = String::from("f=100,a=T,q=2");
    if let Some(cols) = cols.filter(|value| *value > 0) {
        control.push_str(&format!(",c={cols}"));
    }
    if let Some(rows) = rows.filter(|value| *value > 0) {
        control.push_str(&format!(",r={rows}"));
    }
    let mut output = String::new();
    let mut chunks = encoded.as_bytes().chunks(KITTY_CHUNK_SIZE).peekable();
    // 2. 首包携带完整控制键，后续分包只传 m 续传标记
    if let Some(first) = chunks.next() {
        let more = if chunks.peek().is_some() { 1 } else { 0 };
        output.push_str(&format!(
            "\x1b_G{control},m={more};{}\x1b\\",
            String::from_utf8_lossy(first)
        ));
    }
    while let Some(chunk) = chunks.next() {
        let more = if chunks.peek().is_some() { 1 } else { 0 };
        output.push_str(&format!(
            "\x1b_Gm={more};{}\x1b\\",
            String::from_utf8_lossy(chunk)
        ));
    }
    Ok(output)
}

/// 使用 Kitty 图形协议渲染图片。
///
/// Kitty 放置图片后不会自动下移光标；若不按显示高度预留空行，
/// 后续文本写入同一单元格会覆盖并删除图片。
///
/// 参数:
/// - `path`: 图片文件路径
///
/// 返回:
/// - Kitty 图形协议转义序列，末尾带足够换行以占位
fn render_kitty_image(path: &Path) -> Result<String> {
    // 1. 加载并裁掉透明边距（去掉大画布四周空白，不是把图内容缩小）
    let image = load_image_rgba(path)?;
    let image = crop_transparent_bounds(&image);
    // 2. 仅当超出终端可用区域时才缩小，避免「为了压高度把图压扁」
    let (max_cols, max_rows, cell_pw, cell_ph) = kitty_block_limits();
    let image = fit_raster_to_max_cells(image, max_cols, max_rows, cell_pw, cell_ph);
    let temp = tempfile::Builder::new()
        .prefix("sai-kitty-")
        .suffix(".png")
        .tempfile()
        .context("failed to create temporary kitty image")?;
    write_raster_png(temp.path(), &image)?;
    // 3. c 定宽；r 按宽高比计算。Kitty 只传 c，高度由终端按比例决定，
    //    避免同时传偏大的 r 时在图下方 letterbox 出空白
    let (cols, rows) = kitty_cell_dimensions(image.width, image.height);
    let mut output = encode_kitty_png(temp.path(), Some(cols), None)?;
    // 4. 换行数与比例推算的 r 一致（不再多预留）
    for _ in 0..rows {
        output.push('\n');
    }
    Ok(output)
}

/// 使用 iTerm2 图片协议渲染图片。
///
/// 参数:
/// - `path`: 图片文件路径
///
/// 返回:
/// - iTerm2 图片协议转义序列
fn render_iterm_image(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read image {}", path.display()))?;
    let encoded = general_purpose::STANDARD.encode(bytes);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image.png");
    let name = general_purpose::STANDARD.encode(name.as_bytes());
    Ok(format!(
        "\x1b]1337;File=inline=1;name={name}:{encoded}\x07\n"
    ))
}
