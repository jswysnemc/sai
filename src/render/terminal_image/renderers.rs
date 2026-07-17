/// 使用 Sixel 图形协议渲染图片。
///
/// 参数:
/// - `path`: 图片文件路径
///
/// 返回:
/// - Sixel 图形协议转义序列
fn render_sixel_image(path: &Path, size: &TerminalImageSize) -> Result<String> {
    let image = load_image_rgba(path)?;
    let (width, height) = sixel_dimensions(image.width, image.height, size);
    let pixels = quantize_for_sixel(&image, width, height);
    Ok(encode_sixel(&pixels, width, height))
}

/// 使用 chafa 降级渲染图片。
///
/// 参数:
/// - `path`: 图片文件路径
///
/// 返回:
/// - chafa 输出的终端文本
fn render_chafa_image(
    path: &Path,
    raw_size: Option<&str>,
    parsed_size: &TerminalImageSize,
) -> Result<String> {
    let mut command = Command::new("chafa");
    if let Some(size) = raw_size
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| chafa_size(parsed_size))
    {
        command.arg("--size").arg(size);
    }
    command.arg("--fg-only").arg("--threshold").arg("0.75");
    run_chafa(
        command,
        path,
        "failed to run chafa; install chafa or use a terminal image protocol",
    )
}

/// 执行 chafa 并返回标准输出文本。
///
/// 参数:
/// - `command`: 已配置的 chafa 命令
/// - `path`: 图片文件路径
/// - `context`: 命令执行失败时的上下文
///
/// 返回:
/// - chafa 输出文本
fn run_chafa(mut command: Command, path: &Path, context: &str) -> Result<String> {
    let output = command
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .with_context(|| context.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "chafa exited with status {}: {}",
            output.status,
            stderr.trim()
        );
    }
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// 使用 ANSI 真彩色半块字符渲染 PNG，作为不依赖外部命令的兜底方案。
///
/// 参数:
/// - `path`: PNG 图片路径
///
/// 返回:
/// - 可直接打印到终端的 ANSI 文本
fn render_ansi_halfblock_image(path: &Path, size: &TerminalImageSize) -> Result<String> {
    let image = load_image_rgba(path)?;
    let (width, height) = ansi_dimensions(image.width, image.height, size);
    let mut output = String::new();
    let reset = "\x1b[0m";
    for y in (0..height).step_by(2) {
        for x in 0..width {
            let top = sample_resized_pixel(&image, x, y, width, height);
            let bottom = if y + 1 < height {
                sample_resized_pixel(&image, x, y + 1, width, height)
            } else {
                Rgba { a: 0, ..top }
            };
            output.push_str(&render_halfblock_cell(top, bottom));
        }
        output.push_str(reset);
        output.push('\n');
    }
    Ok(output)
}

/// 表格内联图片的最大列宽与行高上限。
const TABLE_INLINE_MAX_COLS: usize = 48;
const TABLE_INLINE_MAX_ROWS: usize = 24;
/// 等宽字体单元格高/宽的目标比（约 2:1）
const MONO_CELL_ASPECT_NUM: usize = 2;
const MONO_CELL_ASPECT_DEN: usize = 1;
/// 行高安全余量：在比例推算结果上再加的行数
const TABLE_IMAGE_ROW_PAD: usize = 1;

/// 将 PNG 按最大列宽渲染为表格图片单元格（Kitty / iTerm2 / Sixel）。
///
/// 参数:
/// - `path`: PNG 图片路径
/// - `max_cols`: 允许的最大终端列数
///
/// 返回:
/// - 带协议载荷与声明宽高的单元格内容
pub(crate) fn render_inline_image_with_max_cols(path: &Path, max_cols: usize) -> Result<CellContent> {
    let image = load_image_rgba(path)?;
    let (cell_pw, cell_ph) = terminal_cell_pixel_size();
    let max_cols = max_cols.clamp(1, TABLE_INLINE_MAX_COLS);
    // 1. 等宽字体校正格高后，先定列宽再按比例推行高
    let (cell_width, cell_height) = table_image_cell_dimensions(
        image.width,
        image.height,
        cell_pw,
        cell_ph,
        max_cols,
        TABLE_INLINE_MAX_ROWS,
    );
    // 2. Kitty：只传 c，由终端按图片比例自算 r；布局侧用 cell_height 预留空行
    if supports_kitty_graphics() {
        let kitty = encode_kitty_png(path, Some(cell_width), None)?;
        return Ok(image_cell_content(kitty, cell_width, cell_height));
    }
    if supports_iterm_inline_image() {
        let iterm = render_iterm_image(path)?;
        return Ok(image_cell_content(
            iterm.trim_end().to_string(),
            cell_width,
            cell_height,
        ));
    }
    let size = TerminalImageSize {
        width_cells: Some(cell_width),
        height_cells: Some(cell_height),
    };
    let (pixel_width, pixel_height) = sixel_dimensions(image.width, image.height, &size);
    let pixels = quantize_for_sixel(&image, pixel_width, pixel_height);
    let sixel = encode_sixel(&pixels, pixel_width, pixel_height);
    Ok(image_cell_content(
        sixel.trim_end().to_string(),
        cell_width,
        cell_height,
    ))
}

/// 将 ioctl 得到的单元格像素尺寸校正为等宽字体的合理高宽比。
///
/// ioctl 在复用器/缩放场景下常把 `cell_ph` 报得偏大，导致行数 `r` 偏小、高度不够。
/// 等宽字体单元格高宽比通常接近 2:1，超出合理区间时钳回该比例。
///
/// 参数:
/// - `cell_pw`: 单格像素宽
/// - `cell_ph`: 单格像素高
///
/// 返回:
/// - 校正后的 `(格宽, 格高)`
fn normalize_mono_cell_pixels(cell_pw: usize, cell_ph: usize) -> (usize, usize) {
    let cell_pw = cell_pw.max(1);
    let cell_ph = cell_ph.max(1);
    let target_ph = (cell_pw * MONO_CELL_ASPECT_NUM / MONO_CELL_ASPECT_DEN).max(1);
    // 高宽比 < 1.5 或 > 2.5 时视为异常，钳到约 2:1
    let lo = (cell_pw * 3 / 2).max(1);
    let hi = (cell_pw * 5 / 2).max(lo);
    let cell_ph = if cell_ph < lo || cell_ph > hi {
        target_ph
    } else {
        cell_ph
    };
    (cell_pw, cell_ph)
}

/// 计算表格内联图占用的终端单元格宽高。
///
/// 参数:
/// - `image_width`: 图片像素宽
/// - `image_height`: 图片像素高
/// - `cell_pw`: 单字符格像素宽
/// - `cell_ph`: 单字符格像素高
/// - `max_cols`: 最大列数
/// - `max_rows`: 最大行数
///
/// 返回:
/// - `(列数 c, 行数 r)`
fn table_image_cell_dimensions(
    image_width: usize,
    image_height: usize,
    cell_pw: usize,
    cell_ph: usize,
    max_cols: usize,
    max_rows: usize,
) -> (usize, usize) {
    image_cell_dimensions(
        image_width,
        image_height,
        cell_pw,
        cell_ph,
        max_cols,
        max_rows,
        TABLE_IMAGE_ROW_PAD,
    )
}

/// 计算图片在终端中的单元格占位（宽优先 + 等宽字体校正）。
///
/// 参数:
/// - `image_width`: 图片像素宽
/// - `image_height`: 图片像素高
/// - `cell_pw`: 单字符格像素宽
/// - `cell_ph`: 单字符格像素高
/// - `max_cols`: 最大列数
/// - `max_rows`: 最大行数
/// - `row_pad`: 行高额外余量
///
/// 返回:
/// - `(列数 c, 行数 r)`
fn image_cell_dimensions(
    image_width: usize,
    image_height: usize,
    cell_pw: usize,
    cell_ph: usize,
    max_cols: usize,
    max_rows: usize,
    row_pad: usize,
) -> (usize, usize) {
    let (cell_pw, cell_ph) = normalize_mono_cell_pixels(cell_pw, cell_ph);
    let image_width = image_width.max(1);
    let image_height = image_height.max(1);
    let max_cols = max_cols.max(1);
    let max_rows = max_rows.max(1);

    // 按列宽推算基础行数（不含安全余量）
    let base_rows_for_cols = |cols: usize| -> usize {
        let cols = cols.max(1);
        let numerator = image_height.saturating_mul(cols).saturating_mul(cell_pw);
        let denominator = image_width.saturating_mul(cell_ph).max(1);
        numerator.div_ceil(denominator).max(1)
    };
    let with_pad = |base: usize| -> usize {
        base.saturating_add(row_pad).min(max_rows).max(1)
    };

    // 1. 列宽：四舍五入取自然列，减轻 ceil 带来的宽度富余
    let natural_cols = ((image_width + cell_pw / 2) / cell_pw).max(1);
    let mut cols = natural_cols.min(max_cols);
    let base_rows = base_rows_for_cols(cols);

    // 2. 加余量后超上限：以可用行高反推列宽
    if with_pad(base_rows) > max_rows || base_rows > max_rows.saturating_sub(row_pad) {
        let usable_rows = max_rows.saturating_sub(row_pad).max(1);
        let numerator = image_width
            .saturating_mul(usable_rows)
            .saturating_mul(cell_ph);
        let denominator = image_height.saturating_mul(cell_pw).max(1);
        cols = numerator.div_ceil(denominator).max(1).min(max_cols);
    }

    (cols, with_pad(base_rows_for_cols(cols)))
}

/// 构造带多行占位的图片单元格。
///
/// 参数:
/// - `first_line`: 首行协议载荷
/// - `cell_width`: 终端列宽
/// - `cell_height`: 终端行高
///
/// 返回:
/// - 表格单元格内容
fn image_cell_content(first_line: String, cell_width: usize, cell_height: usize) -> CellContent {
    let mut lines = vec![first_line];
    for _ in 1..cell_height.max(1) {
        lines.push(String::new());
    }
    CellContent::from_image(lines, cell_width, None)
}

/// 将 PNG 渲染为单终端行半块图片，尾部补齐空格以匹配显示宽度。
///
/// 参数:
/// - `path`: PNG 图片路径
/// - `target_height_px`: 目标像素高度（应为 2 的倍数）
///
/// 返回:
/// - 单行 ANSI 半块文本，宽度由图片比例决定
pub(crate) fn render_halfblock_line(path: &Path, target_height_px: usize) -> Result<String> {
    let image = load_image_rgba(path)?;
    let target_height_px = target_height_px.max(2);
    let target_width = if image.height == 0 {
        1
    } else {
        (image.width as usize * target_height_px / image.height as usize).max(1)
    };
    let mut output = String::new();
    let reset = "\x1b[0m";
    for x in 0..target_width {
        let top = sample_resized_pixel(&image, x, 0, target_width, target_height_px);
        let bottom = if target_height_px > 1 {
            sample_resized_pixel(&image, x, 1, target_width, target_height_px)
        } else {
            Rgba { a: 0, ..top }
        };
        output.push_str(&render_halfblock_cell(top, bottom));
    }
    output.push_str(reset);
    Ok(output)
}


