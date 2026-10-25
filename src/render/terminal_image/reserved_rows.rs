/// 【终端图片】【占位识别】读取行内图片声明的占位高度，避免空白裁剪删除图片区域。
/// 参数: `text` 为单行终端文本；返回: 最大图片行数，无图片时为空。
pub(crate) fn reserved_image_rows(text: &str) -> Option<usize> {
    let kitty = text
        .split("\x1b_Ga=p")
        .skip(1)
        .filter_map(|part| {
            let control = part.split("\x1b\\").next()?;
            control
                .split(",r=")
                .nth(1)?
                .split(',')
                .next()?
                .parse::<usize>()
                .ok()
        })
        .max();
    let iterm = text
        .split("\x1b]1337;File=")
        .skip(1)
        .filter_map(|part| {
            let control = part.split(':').next()?;
            control
                .split("height=")
                .nth(1)?
                .split(';')
                .next()?
                .parse::<usize>()
                .ok()
        })
        .max();
    kitty.into_iter().chain(iterm).max()
}

/// 【终端图片】【末尾占位】只删除非图像尾部换行，保留图片声明高度所需的全部视觉行。
/// 参数: `text` 为已渲染正文；返回: 没有多余尾部留白且图像占位完整的文本。
pub(crate) fn trim_trailing_image_newlines(text: &str) -> String {
    let reserved_end = text
        .split('\n')
        .enumerate()
        .filter_map(|(row, line)| reserved_image_rows(line).map(|height| row + height))
        .max()
        .unwrap_or(0);
    let mut output = text.trim_end_matches('\n').to_string();
    let rows = if output.is_empty() {
        0
    } else {
        output.split('\n').count()
    };
    if reserved_end > rows {
        output.push_str(&"\n".repeat(reserved_end - rows));
    }
    output
}
