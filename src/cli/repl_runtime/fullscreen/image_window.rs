//! 全屏正文窗口里的 Kitty 图片放置：跨行图片随滚动裁剪，不会整张消失或压住输入框。

use crate::render::transcript::AnsiLine;

/// 向上回溯查找跨行图片的最大行数，与块级图片的最大高度一致。
const MAX_IMAGE_ROWS: usize = 120;

/// 解析出的一条 Kitty 放置序列。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Placement {
    /// 序列在行内的字节范围
    start: usize,
    end: usize,
    /// 序列前的可见列数
    col: usize,
    image_id: u32,
    placement_id: u32,
    cols: Option<usize>,
    rows: usize,
}

/// 【全屏视图】【图片窗口】取出可见行，并按窗口边界改写跨行图片的放置序列。
///
/// 块级公式图片的放置序列只写在首行，其余行是占位空行：
/// - 首行滚出窗口顶部时，把放置移到第一可见行并裁掉上方已滚走的部分；
/// - 图片下沿超出窗口底部时，裁掉超出部分，避免压住固定在底部的输入框。
///
/// 参数:
/// - `lines`: 全屏正文全部行
/// - `scroll`: 窗口首行在正文中的行号
/// - `height`: 窗口行数
/// - `cell_ph`: 单格像素高
///
/// 返回:
/// - 窗口内每一行的文本
pub(super) fn window_lines(
    lines: &[AnsiLine],
    scroll: usize,
    height: usize,
    cell_ph: usize,
) -> Vec<String> {
    let mut window = (0..height)
        .map(|row| {
            lines
                .get(scroll + row)
                .map(|line| line.as_str().to_string())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    // 1. 窗口内起始的图片：只裁底部
    for (row, line) in window.iter_mut().enumerate() {
        for placement in parse_placements(line).into_iter().rev() {
            let visible = placement.rows.min(height - row);
            if visible < placement.rows {
                let rewritten = cropped(&placement, 0, visible, cell_ph);
                line.replace_range(placement.start..placement.end, &rewritten);
            }
        }
    }
    // 2. 窗口上方起始、仍延伸进窗口的图片：移到首个可见行并裁掉上部
    let mut carried = String::new();
    for source in (scroll.saturating_sub(MAX_IMAGE_ROWS)..scroll).rev() {
        let Some(line) = lines.get(source) else {
            continue;
        };
        for placement in parse_placements(line.as_str()) {
            let hidden = scroll - source;
            if hidden >= placement.rows {
                continue;
            }
            let visible = (placement.rows - hidden).min(height);
            if visible == 0 {
                continue;
            }
            // 保存光标、回到行首并右移到原列；列 0 不能写 CUF 0（多数终端按 1 处理）
            carried.push_str("\x1b7\r");
            if placement.col > 0 {
                carried.push_str(&format!("\x1b[{}C", placement.col));
            }
            carried.push_str(&cropped(&placement, hidden, visible, cell_ph));
            carried.push_str("\x1b8");
        }
    }
    if !carried.is_empty() {
        if let Some(first) = window.first_mut() {
            first.insert_str(0, &carried);
        }
    }
    window
}

/// 返回窗口内所有放置序列的签名，用于判断图片是否需要整屏重放。
///
/// 参数:
/// - `rows`: 已组装的屏幕行
///
/// 返回:
/// - 每条放置序列与所在行
pub(super) fn placement_signature(rows: &[String]) -> Vec<(usize, String)> {
    rows.iter()
        .enumerate()
        .flat_map(|(row, line)| {
            parse_placements(line)
                .into_iter()
                .map(move |placement| (row, line[placement.start..placement.end].to_string()))
        })
        .collect()
}

/// 生成裁剪后的放置序列。
///
/// 参数:
/// - `placement`: 原放置
/// - `skip_rows`: 从图片顶部裁掉的行数
/// - `visible_rows`: 保留的行数
/// - `cell_ph`: 单格像素高
///
/// 返回:
/// - 新的放置序列
fn cropped(placement: &Placement, skip_rows: usize, visible_rows: usize, cell_ph: usize) -> String {
    let mut control = format!(
        "a=p,q=2,C=1,i={},p={}",
        placement.image_id,
        derived_placement_id(placement.placement_id, skip_rows, visible_rows)
    );
    if let Some(cols) = placement.cols {
        control.push_str(&format!(",c={cols}"));
    }
    // 源矩形：块级图片已补齐到整格像素，y/h 按格高换算
    control.push_str(&format!(
        ",r={visible_rows},y={},h={}",
        skip_rows * cell_ph,
        visible_rows * cell_ph
    ));
    format!("\x1b_G{control}\x1b\\")
}

/// 为裁剪后的放置派生稳定且不与原放置冲突的 ID。
///
/// 同一滚动位置每帧得到相同 ID，重绘走替换语义；高位置位避开递增分配的原 ID。
fn derived_placement_id(original: u32, skip_rows: usize, visible_rows: usize) -> u32 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::hash::DefaultHasher::new();
    (original, skip_rows, visible_rows).hash(&mut hasher);
    (hasher.finish() as u32) | 0x8000_0000
}

/// 解析一行中的 Kitty 放置序列（`a=p`），忽略传输序列。
///
/// 参数:
/// - `line`: 终端行
///
/// 返回:
/// - 放置序列列表，按出现顺序
fn parse_placements(line: &str) -> Vec<Placement> {
    let mut placements = Vec::new();
    let mut col = 0usize;
    let mut index = 0usize;
    while index < line.len() {
        if line.as_bytes()[index] != 0x1b {
            let Some(ch) = line[index..].chars().next() else {
                break;
            };
            col += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
            index += ch.len_utf8();
            continue;
        }
        let end = crate::render::terminal_image::escape_sequence_end(line, index)
            .max(index + 1)
            .min(line.len());
        if let Some(placement) = parse_placement(&line[index..end], index, end, col) {
            placements.push(placement);
        }
        index = end;
    }
    placements
}

/// 解析单条转义序列；不是带行数的放置序列时返回 `None`。
fn parse_placement(sequence: &str, start: usize, end: usize, col: usize) -> Option<Placement> {
    let control = sequence.strip_prefix("\x1b_G")?;
    let control = control.split(['\x1b', ';']).next()?;
    let mut fields = std::collections::HashMap::new();
    for part in control.split(',') {
        if let Some((key, value)) = part.split_once('=') {
            fields.insert(key, value);
        }
    }
    if fields.get("a") != Some(&"p") {
        return None;
    }
    Some(Placement {
        start,
        end,
        col,
        image_id: fields.get("i")?.parse().ok()?,
        placement_id: fields.get("p").and_then(|value| value.parse().ok()).unwrap_or(0),
        cols: fields.get("c").and_then(|value| value.parse().ok()),
        rows: fields.get("r").and_then(|value| value.parse().ok()).unwrap_or(1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 生成首行带放置序列、其余为占位空行的块级图片。
    fn block_image(rows: usize) -> Vec<AnsiLine> {
        let mut lines = vec![AnsiLine::new("before".into())];
        lines.push(AnsiLine::new(format!(
            "  \x1b_Ga=p,q=2,C=1,i=42,p=5,c=20,r={rows}\x1b\\"
        )));
        for _ in 1..rows {
            lines.push(AnsiLine::new(String::new()));
        }
        lines.push(AnsiLine::new("after".into()));
        lines
    }

    /// 验证完整可见的图片原样保留。
    #[test]
    fn fully_visible_image_is_unchanged() {
        let lines = block_image(4);
        let window = window_lines(&lines, 0, 10, 20);
        assert_eq!(window[1], lines[1].as_str());
        assert_eq!(placement_signature(&window).len(), 1);
    }

    /// 验证首行滚出顶部后，图片移到首个可见行并裁掉上部。
    #[test]
    fn image_scrolled_past_top_is_carried_and_cropped() {
        let lines = block_image(4);
        // 正文第 1 行是图片首行；窗口从第 3 行开始，上方隐藏 2 行
        let window = window_lines(&lines, 3, 5, 20);
        let first = &window[0];
        assert!(first.starts_with("\x1b7\r\x1b[2C\x1b_G"), "{first:?}");
        assert!(first.contains("i=42"));
        assert!(first.contains(",r=2,y=40,h=40"), "{first:?}");
        assert!(first.ends_with("\x1b8"));
        // 图片完全滚出后不再携带
        let gone = window_lines(&lines, 5, 3, 20);
        assert!(placement_signature(&gone).is_empty());
    }

    /// 验证超出窗口底部的部分被裁掉，不压住下方输入框。
    #[test]
    fn image_overflowing_bottom_is_cropped() {
        let lines = block_image(6);
        let window = window_lines(&lines, 0, 4, 20);
        assert!(window[1].contains(",r=3,y=0,h=60"), "{:?}", window[1]);
        assert!(!window[1].contains("p=5,"));
    }

    /// 验证裁剪后的放置 ID 稳定且与原 ID 不同。
    #[test]
    fn derived_ids_are_stable_and_distinct() {
        assert_eq!(derived_placement_id(5, 2, 3), derived_placement_id(5, 2, 3));
        assert_ne!(derived_placement_id(5, 2, 3), 5);
        assert_ne!(derived_placement_id(5, 2, 3), derived_placement_id(5, 1, 3));
    }
}
