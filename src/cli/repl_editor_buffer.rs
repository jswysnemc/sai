use super::repl_clipboard::ReplClipboardState;

/// 进入外部编辑器前摘出的图片占位块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DetachedImage {
    /// 在送入编辑器的正文中的字符位置
    anchor: usize,
    /// 原始占位符文本
    marker: String,
}

/// 送入外部编辑器的缓冲区。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorBuffer {
    /// 展开长文本、去掉图片后的正文
    pub(super) text: String,
    /// 摘出的图片占位块，退出后按锚点复位
    images: Vec<DetachedImage>,
}

/// 构造送入外部编辑器的缓冲区。
///
/// 折叠的长文本展开成完整正文，用户才能在编辑器里真正修改它；
/// 图片是超长的 base64 data URL，留在正文里会把编辑器刷满，因此先摘出。
///
/// 参数:
/// - `input`: 当前输入内容
/// - `clipboard`: 当前剪贴板占位块状态
///
/// 返回:
/// - 编辑器缓冲区
pub(super) fn prepare_editor_buffer(input: &str, clipboard: &ReplClipboardState) -> EditorBuffer {
    let (text, images) = clipboard.expand_for_editor(input);
    EditorBuffer {
        text,
        images: images
            .into_iter()
            .map(|(anchor, marker)| DetachedImage { anchor, marker })
            .collect(),
    }
}

/// 把编辑器返回的正文与摘出的图片重新合成输入内容。
///
/// 编辑器里的改动可能让原锚点失效，按公共前后缀判断每个锚点的归属：
///   1. 锚点落在未改动的前缀里，位置保持不变
///   2. 锚点落在未改动的后缀里，按长度差整体平移
///   3. 锚点落在被改写的区间里，原位置已不存在，追加到末尾而不是随意插入
///
/// 参数:
/// - `buffer`: 进入编辑器前的缓冲区
/// - `edited`: 编辑器返回的正文
///
/// 返回:
/// - 复位图片占位块后的输入内容
pub(super) fn restore_editor_buffer(buffer: &EditorBuffer, edited: &str) -> String {
    if buffer.images.is_empty() {
        return edited.to_string();
    }
    let before: Vec<char> = buffer.text.chars().collect();
    let after: Vec<char> = edited.chars().collect();
    let prefix = common_prefix_len(&before, &after);
    let suffix = common_suffix_len(&before, &after, prefix);
    let before_tail_start = before.len() - suffix;
    let shift = after.len() as isize - before.len() as isize;

    // 1. 逐个把锚点映射到编辑后的位置，映射不出来的排到末尾
    let mut placements: Vec<(usize, &str)> = Vec::new();
    let mut appended: Vec<&str> = Vec::new();
    for image in &buffer.images {
        if image.anchor <= prefix {
            placements.push((image.anchor, image.marker.as_str()));
        } else if image.anchor >= before_tail_start {
            let mapped = (image.anchor as isize + shift).max(0) as usize;
            placements.push((mapped.min(after.len()), image.marker.as_str()));
        } else {
            appended.push(image.marker.as_str());
        }
    }
    // 2. 从后往前插入，先插入的位置不会影响后续锚点
    placements.sort_by_key(|(position, _)| *position);
    let mut result: String = after.iter().collect();
    for (position, marker) in placements.iter().rev() {
        let byte = char_to_byte(&result, *position);
        result.insert_str(byte, marker);
    }
    // 3. 位置已失效的图片挂到末尾，不丢失附件
    for marker in appended {
        if !result.is_empty() && !result.ends_with(char::is_whitespace) {
            result.push(' ');
        }
        result.push_str(marker);
    }
    result
}

/// 计算两段字符序列的公共前缀长度。
///
/// 参数:
/// - `left`: 左侧字符序列
/// - `right`: 右侧字符序列
///
/// 返回:
/// - 公共前缀的字符数
fn common_prefix_len(left: &[char], right: &[char]) -> usize {
    left.iter()
        .zip(right.iter())
        .take_while(|(a, b)| a == b)
        .count()
}

/// 计算两段字符序列的公共后缀长度。
///
/// 参数:
/// - `left`: 左侧字符序列
/// - `right`: 右侧字符序列
/// - `prefix`: 已确认的公共前缀长度，避免前后缀重叠计算
///
/// 返回:
/// - 公共后缀的字符数
fn common_suffix_len(left: &[char], right: &[char], prefix: usize) -> usize {
    let limit = left.len().min(right.len()).saturating_sub(prefix);
    let mut count = 0usize;
    while count < limit {
        let a = left[left.len() - 1 - count];
        let b = right[right.len() - 1 - count];
        if a != b {
            break;
        }
        count += 1;
    }
    count
}

/// 把字符位置换算为字节偏移。
///
/// 参数:
/// - `text`: 目标字符串
/// - `position`: 字符位置
///
/// 返回:
/// - 对应的字节偏移，越界时返回字符串长度
fn char_to_byte(text: &str, position: usize) -> usize {
    text.char_indices()
        .nth(position)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造带一个图片占位块的缓冲区。
    ///
    /// 参数:
    /// - `text`: 去掉图片后的正文
    /// - `anchor`: 图片锚点字符位置
    ///
    /// 返回:
    /// - 测试用缓冲区
    fn buffer_with_image(text: &str, anchor: usize) -> EditorBuffer {
        EditorBuffer {
            text: text.to_string(),
            images: vec![DetachedImage {
                anchor,
                marker: "[图片 #1]".to_string(),
            }],
        }
    }

    /// 验证正文未改动时图片回到原位。
    #[test]
    fn restores_image_at_original_anchor() {
        let buffer = buffer_with_image("看这张 和这段说明", 4);
        let restored = restore_editor_buffer(&buffer, "看这张 和这段说明");
        assert_eq!(restored, "看这张 [图片 #1]和这段说明");
    }

    /// 验证在图片之后追加内容时，图片位置保持不变。
    #[test]
    fn keeps_anchor_when_text_after_it_changes() {
        let buffer = buffer_with_image("前缀 后面", 3);
        let restored = restore_editor_buffer(&buffer, "前缀 后面补充了很多内容");
        assert_eq!(restored, "前缀 [图片 #1]后面补充了很多内容");
    }

    /// 验证在图片之前插入内容时，图片随后缀平移，仍夹在原来的两个字之间。
    #[test]
    fn shifts_anchor_when_text_before_it_changes() {
        // 锚点 4 表示图片夹在"后"与"面"之间
        let buffer = buffer_with_image("前缀 后面", 4);
        let restored = restore_editor_buffer(&buffer, "改写过的前缀 后面");
        assert_eq!(restored, "改写过的前缀 后[图片 #1]面");
    }

    /// 验证锚点所在区间被整体改写时，图片挂到末尾而不是丢失。
    #[test]
    fn appends_image_when_anchor_region_is_rewritten() {
        let buffer = buffer_with_image("开头中间部分结尾", 4);
        let restored = restore_editor_buffer(&buffer, "开头完全不同的正文结尾");
        assert!(restored.starts_with("开头完全不同的正文结尾"));
        assert!(restored.ends_with("[图片 #1]"));
    }

    /// 验证没有图片时原样返回编辑结果。
    #[test]
    fn returns_edited_text_without_images() {
        let buffer = EditorBuffer {
            text: "原文".to_string(),
            images: Vec::new(),
        };
        assert_eq!(restore_editor_buffer(&buffer, "改过的正文"), "改过的正文");
    }

    /// 验证多张图片按锚点顺序复位。
    #[test]
    fn restores_multiple_images_in_order() {
        let buffer = EditorBuffer {
            text: "abcdef".to_string(),
            images: vec![
                DetachedImage {
                    anchor: 2,
                    marker: "[A]".to_string(),
                },
                DetachedImage {
                    anchor: 4,
                    marker: "[B]".to_string(),
                },
            ],
        };
        assert_eq!(restore_editor_buffer(&buffer, "abcdef"), "ab[A]cd[B]ef");
    }
}
