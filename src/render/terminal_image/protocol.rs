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

/// 测试用的协议判定覆盖（线程局部）。
///
/// 直接改环境变量会污染同进程内的其它测试：测试默认并行执行，一处
/// `remove_var("KITTY_WINDOW_ID")` 会让并发运行的图片渲染测试误判终端
/// 不支持图形协议，从而随机失败。线程局部覆盖天然按测试隔离。
#[cfg(test)]
pub(super) mod test_override {
    use std::cell::Cell;

    thread_local! {
        static KITTY: Cell<Option<bool>> = const { Cell::new(None) };
        static ITERM: Cell<Option<bool>> = const { Cell::new(None) };
        static SIXEL: Cell<Option<bool>> = const { Cell::new(None) };
    }

    /// 一次性设置三项协议判定结果，`None` 表示回退到真实环境检测。
    pub fn set(kitty: Option<bool>, iterm: Option<bool>, sixel: Option<bool>) {
        KITTY.with(|cell| cell.set(kitty));
        ITERM.with(|cell| cell.set(iterm));
        SIXEL.with(|cell| cell.set(sixel));
    }

    pub fn kitty() -> Option<bool> {
        KITTY.with(|cell| cell.get())
    }

    pub fn iterm() -> Option<bool> {
        ITERM.with(|cell| cell.get())
    }

    pub fn sixel() -> Option<bool> {
        SIXEL.with(|cell| cell.get())
    }
}

/// 判断当前终端是否支持 Kitty 图形协议。
///
/// 环境变量只能覆盖一部分终端：Ghostty / foot / Konsole / rio 都实现了
/// Kitty 协议，但不设 `KITTY_WINDOW_ID`。漏判会让 mermaid 与 `$$…$$`
/// 公式静默降级成粗糙的半块栅格。
///
/// tmux 需要显式开启 allow-passthrough 才会转发图形序列，默认不透传，
/// 因此在 tmux 里不按终端名推断支持——宁可降级，也不要吐出一串被吞掉的
/// 转义序列。WezTerm 走下面的 iTerm2 分支，这里不重复判定。
///
/// 返回:
/// - 是否支持 Kitty 图形协议
fn supports_kitty_graphics() -> bool {
    #[cfg(test)]
    if let Some(value) = test_override::kitty() {
        return value;
    }
    if std::env::var_os("KITTY_WINDOW_ID").is_some() {
        return true;
    }
    if std::env::var_os("TMUX").is_some() {
        return false;
    }
    let term = std::env::var("TERM").unwrap_or_default();
    // xterm-kitty 与 Ghostty 都走 Kitty 协议；foot 以 foot / foot-extra 开头
    if term.contains("xterm-kitty") || term.starts_with("xterm-ghostty") || term.starts_with("foot")
    {
        return true;
    }
    matches!(
        std::env::var("TERM_PROGRAM").unwrap_or_default().as_str(),
        "ghostty" | "foot" | "Konsole" | "rio"
    )
}

/// 判断当前终端是否支持 iTerm2 图片协议。
///
/// 返回:
/// - 是否支持 iTerm2 图片协议
fn supports_iterm_inline_image() -> bool {
    #[cfg(test)]
    if let Some(value) = test_override::iterm() {
        return value;
    }
    std::env::var("TERM_PROGRAM")
        .map(|program| matches!(program.as_str(), "iTerm.app" | "WezTerm"))
        .unwrap_or(false)
}

/// 判断当前终端是否支持 Windows Terminal 使用的 Sixel 图形协议。
///
/// 返回:
/// - 是否可能支持 Sixel 图形协议
fn supports_windows_terminal_sixel() -> bool {
    #[cfg(test)]
    if let Some(value) = test_override::sixel() {
        return value;
    }
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
#[cfg(test)]
fn kitty_cell_dimensions(pixel_width: usize, pixel_height: usize) -> (usize, usize) {
    kitty_cell_dimensions_with(kitty_block_limits(), pixel_width, pixel_height)
}

/// 以显式终端限制计算 Kitty 块级图片的列/行数（便于复用同一份限制）。
///
/// 参数:
/// - `limits`: `(max_cols, max_rows, cell_pw, cell_ph)`
/// - `pixel_width`: 图片像素宽度
/// - `pixel_height`: 图片像素高度
///
/// 返回:
/// - `(列数 c, 行数 r)`
fn kitty_cell_dimensions_with(
    limits: (usize, usize, usize, usize),
    pixel_width: usize,
    pixel_height: usize,
) -> (usize, usize) {
    let (max_cols, max_rows, cell_pw, cell_ph) = limits;
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
    // 图像 ID 取内容 hash，放置 ID 进程内递增分配一次。
    // 渲染产物字符串会被缓存重放：重打时 (i,p) 不变，Kitty 以「替换」
    // 语义更新放置位置，transcript 重排 / 重绘不再留下旧图残影叠影
    let image_id = kitty_image_id(&bytes);
    let placement_id = next_kitty_placement_id();
    // 1. 传输与放置分离：图像数据（大 payload）每个 image_id 只直写终端
    //    一次；产物字符串只含轻量放置序列。live 重绘 / transcript 重打
    //    重放的都是缓存字符串，分离后不再反复重传几百 KB 的 base64
    if register_kitty_transmission(image_id) {
        transmit_kitty_data_now(&kitty_transmission_payload(image_id, &bytes));
    }
    // 2. 放置序列：q=2 静默；C=1 放置后光标保持原位（默认会跳到图像
    //    右下角之后，表格等逐行拼接的布局会从图片列开始整体错位），
    //    占位由文本层（换行 / 空格）自行控制（对齐 jcode/ratatui-image）
    let mut control = format!("a=p,q=2,C=1,i={image_id},p={placement_id}");
    if let Some(cols) = cols.filter(|value| *value > 0) {
        control.push_str(&format!(",c={cols}"));
    }
    if let Some(rows) = rows.filter(|value| *value > 0) {
        control.push_str(&format!(",r={rows}"));
    }
    Ok(format!("\x1b_G{control}\x1b\\"))
}

/// 组装 Kitty 图像数据传输载荷（a=t 只传输不放置）。
///
/// 参数:
/// - `image_id`: 图像 ID
/// - `bytes`: PNG 文件字节
///
/// 返回:
/// - 分块传输转义序列
fn kitty_transmission_payload(image_id: u32, bytes: &[u8]) -> String {
    let encoded = general_purpose::STANDARD.encode(bytes);
    let mut output = String::new();
    let mut chunks = encoded.as_bytes().chunks(KITTY_CHUNK_SIZE).peekable();
    // 首包携带完整控制键，后续分包只传 m 续传标记
    if let Some(first) = chunks.next() {
        let more = if chunks.peek().is_some() { 1 } else { 0 };
        output.push_str(&format!(
            "\x1b_Ga=t,f=100,q=2,i={image_id},m={more};{}\x1b\\",
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
    output
}

/// 首次见到该图像 ID 时登记并返回 true（需要传输数据）。
///
/// 参数:
/// - `image_id`: 图像 ID
///
/// 返回:
/// - 本进程内是否为首次传输
fn register_kitty_transmission(image_id: u32) -> bool {
    use std::collections::HashSet;
    use std::sync::{LazyLock, Mutex};
    static TRANSMITTED: LazyLock<Mutex<HashSet<u32>>> =
        LazyLock::new(|| Mutex::new(HashSet::new()));
    TRANSMITTED
        .lock()
        .map(|mut set| set.insert(image_id))
        .unwrap_or(true)
}

/// 将传输载荷立即直写终端。
///
/// a=t 只上传数据：不产生可见输出、不移动光标，插入到任何绘制序列
/// 之间都无副作用；放置序列随渲染文本在其后送达。
///
/// 参数:
/// - `payload`: 传输转义序列
fn transmit_kitty_data_now(payload: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(payload.as_bytes());
    let _ = stdout.flush();
}

/// 删除所有可见 Kitty 图像放置（保留已传输数据供重放引用）。
pub(crate) const KITTY_DELETE_PLACEMENTS: &str = "\x1b_Ga=d,d=a,q=2\x1b\\";

/// 由 PNG 字节计算非零 Kitty 图像 ID。
///
/// 参数:
/// - `bytes`: PNG 文件字节
///
/// 返回:
/// - 32 位非零图像 ID
fn kitty_image_id(bytes: &[u8]) -> u32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    bytes.hash(&mut hasher);
    let id = hasher.finish() as u32;
    if id == 0 { 1 } else { id }
}

/// 分配进程内唯一的 Kitty 放置 ID（非零）。
///
/// 同一渲染产物缓存重放时 ID 随字节保持不变；相同图像内容的两次
/// 独立渲染各获得不同放置 ID，互不顶替。
///
/// 返回:
/// - 32 位非零放置 ID
fn next_kitty_placement_id() -> u32 {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if id == 0 { 1 } else { id }
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
    let limits = kitty_block_limits();
    let (max_cols, max_rows, cell_pw, cell_ph) = limits;
    let image = fit_raster_to_max_cells(image, max_cols, max_rows, cell_pw, cell_ph);
    // 3. 推算占位网格后，把像素补齐到恰好覆盖整格。
    //    只传 c 时终端按自己的取整决定实际行数，与这里预留的换行数可能差一行，
    //    表现为文本覆盖图片底部或图下多出空行；c/r 同传 + 像素对齐整格后，
    //    渲染行数与占位行数强制一致（对齐 jcode/ratatui-image 的做法）
    let (cols, rows) = kitty_cell_dimensions_with(limits, image.width, image.height);
    let image = pad_raster_to_cell_grid(image, cols, rows, cell_pw, cell_ph);
    let temp = tempfile::Builder::new()
        .prefix("sai-kitty-")
        .suffix(".png")
        .tempfile()
        .context("failed to create temporary kitty image")?;
    write_raster_png(temp.path(), &image)?;
    let mut output = encode_kitty_png(temp.path(), Some(cols), Some(rows))?;
    // 4. 换行数与声明的 r 严格一致
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

/// 使用 iTerm2 图片协议按显式单元格尺寸渲染图片。
///
/// 表格等按行占位的布局依赖「渲染行数与声明一致」；不带尺寸时终端按
/// 图片像素与自身 DPI 自算行数，与布局侧预留完全脱钩。
///
/// 参数:
/// - `path`: 图片文件路径
/// - `cols`: 显示列数（cells）
/// - `rows`: 显示行数（cells）
///
/// 返回:
/// - 声明宽高的 iTerm2 图片协议转义序列
fn render_iterm_image_with_cells(path: &Path, cols: usize, rows: usize) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read image {}", path.display()))?;
    let encoded = general_purpose::STANDARD.encode(bytes);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("image.png");
    let name = general_purpose::STANDARD.encode(name.as_bytes());
    let cols = cols.max(1);
    let rows = rows.max(1);
    Ok(format!(
        "\x1b]1337;File=inline=1;width={cols};height={rows};preserveAspectRatio=1;name={name}:{encoded}\x07\n"
    ))
}
