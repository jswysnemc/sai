use super::{ComposerFrame, ComposerSignature, CHROME_INPUT_PREFIX_COLS};
use crate::render::terminal_image::{self, ComposerTile};
use crossterm::{cursor::MoveTo, queue};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::sync::Arc;

const WIDTH: usize = 6;
const HEIGHT: usize = 3;
const GAP: usize = 1;
const MAX_VISIBLE: usize = 64;
type Environment = (bool, bool, bool, (usize, usize));
type TileCache = HashMap<u64, Option<Arc<ComposerTile>>>;

#[derive(Clone, Default)]
pub(super) struct ComposerImages {
    sources: Vec<String>,
    cache: RefCell<(Option<Environment>, TileCache)>,
}

#[derive(Default)]
pub(super) struct ImageStrip {
    tiles: Vec<Option<Arc<ComposerTile>>>,
    hidden: usize,
    badge_visible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PlacedTile {
    column: u16,
    row: u16,
    tile: Arc<ComposerTile>,
}

impl ComposerFrame {
    /// 【输入图片】【状态继承】新草稿继承图片缓存，但只展示当前仍登记的附件。
    /// 参数: sources 为有序来源，previous 为旧输入框；返回: 无。
    pub(in crate::cli::repl_runtime) fn set_images(
        &mut self,
        sources: Vec<String>,
        previous: Option<&mut Self>,
    ) {
        self.images = previous
            .map(|frame| std::mem::take(&mut frame.images))
            .unwrap_or_default();
        self.images.sources = sources;
        if self.images.sources.is_empty() {
            self.images.cache.get_mut().1.clear();
        }
    }
}

impl ComposerImages {
    /// 【输入图片】【预览布局】按净宽裁剪缩略图数量，仅解码可见附件并缓存失败结果。
    /// 参数: width 为输入净列数；返回: 缩略图条，不支持图形时为空。
    pub(super) fn layout(&self, width: usize) -> ImageStrip {
        if self.sources.is_empty() {
            return ImageStrip::default();
        }
        let Some(cell) = terminal_image::composer_tile_environment() else {
            return ImageStrip::default();
        };
        let (kitty, iterm, sixel, _) = terminal_image::preview_environment();
        let environment = Some((kitty, iterm, sixel, cell));
        let mut cache = self.cache.borrow_mut();
        if cache.0 != environment {
            *cache = (environment, HashMap::new());
        }
        let (visible, hidden) = visible_tiles(width, self.sources.len());
        let mut previous = std::mem::take(&mut cache.1);
        let mut occurrences = HashMap::<&str, usize>::new();
        let tiles = self
            .sources
            .iter()
            .take(visible)
            .map(|source| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                source.hash(&mut hasher);
                let occurrence = occurrences.entry(source.as_str()).or_default();
                occurrence.hash(&mut hasher);
                *occurrence += 1;
                if !source.starts_with("data:") {
                    if let Ok(metadata) = std::fs::metadata(source) {
                        metadata.len().hash(&mut hasher);
                        metadata.modified().ok().hash(&mut hasher);
                    }
                }
                let key = hasher.finish();
                // 1. 【输入图片】【放置隔离】同源附件按出现次数独立缓存，避免互相覆盖与重复解码
                let tile = previous.remove(&key).unwrap_or_else(|| {
                    terminal_image::render_composer_tile(source)
                        .ok()
                        .map(Arc::new)
                });
                cache.1.insert(key, tile.clone());
                tile
            })
            .collect();
        ImageStrip {
            tiles,
            hidden,
            badge_visible: hidden > 0 && width >= hidden.to_string().len() + 1,
        }
    }
}

impl ImageStrip {
    /// 【输入图片】【预留高度】图片与正文之间保留一行，极窄窗口只显示计数。
    /// 参数: 无；返回: 占位行数。
    pub(super) fn height(&self) -> usize {
        if !self.tiles.is_empty() {
            HEIGHT + GAP
        } else if self.badge_visible {
            2
        } else {
            0
        }
    }

    /// 【输入图片】【背景行】生成纯文本背景，不把图形协议混入文本折行与选区。
    /// 参数: 无；返回: 输入框内部行内容。
    pub(super) fn lines(&self) -> Vec<String> {
        let mut lines = vec![String::new(); self.height()];
        if lines.is_empty() {
            return lines;
        }
        for (index, tile) in self.tiles.iter().enumerate() {
            let left = index * (WIDTH + GAP);
            if tile.is_none() {
                for (row, text) in ["┌────┐", "│ -- │", "└────┘"].iter().enumerate()
                {
                    let padding =
                        left.saturating_sub(crate::cli::repl_text::visible_width(&lines[row]));
                    lines[row].push_str(&" ".repeat(padding));
                    lines[row].push_str(text);
                }
            }
        }
        if self.badge_visible {
            let left = self.tiles.len() * (WIDTH + GAP);
            let padding = left.saturating_sub(crate::cli::repl_text::visible_width(&lines[0]));
            lines[0].push_str(&" ".repeat(padding));
            lines[0].push_str(&format!("+{}", self.hidden));
        }
        lines
    }

    /// 【输入图片】【屏幕定位】只有整个缩略图条可见时才生成图形放置。
    /// 参数: top 为首行屏幕坐标；返回: 独立图片放置列表。
    pub(super) fn placements(&self, top: u16) -> Vec<PlacedTile> {
        self.tiles
            .iter()
            .enumerate()
            .filter_map(|(index, tile)| {
                Some(PlacedTile {
                    column: (CHROME_INPUT_PREFIX_COLS + index * (WIDTH + GAP)) as u16,
                    row: top,
                    tile: tile.clone()?,
                })
            })
            .collect()
    }
}

impl ComposerSignature {
    /// 【输入图片】【背景覆盖】仅当图片占位行发生变化时补绘，普通打字不重传图片。
    /// 参数: previous 为相同位置的旧文本行；返回: 是否需要补绘。
    pub(super) fn image_rows_changed(&self, previous: Option<&[String]>) -> bool {
        self.images.iter().any(|image| {
            let start = usize::from(image.row.saturating_sub(self.top));
            (start..start + HEIGHT)
                .any(|row| previous.and_then(|lines| lines.get(row)) != self.lines.get(row))
        })
    }

    /// 【输入图片】【定向清理】仅删除当前草稿图片，不影响正文或浮窗放置。
    /// 参数: output 为帧缓冲；返回: 写入结果。
    pub(in crate::cli::repl_runtime) fn clear_images(
        &self,
        output: &mut impl Write,
    ) -> io::Result<()> {
        for image in &self.images {
            output.write_all(image.tile.cleanup.as_bytes())?;
        }
        Ok(())
    }

    /// 【输入图片】【图片补绘】文本背景完成后绘制图片，避免后续空白行覆盖像素。
    /// 参数: output 为帧缓冲；返回: 写入结果。
    pub(super) fn paint_images(&self, output: &mut impl Write) -> io::Result<()> {
        for image in &self.images {
            queue!(output, MoveTo(image.column, image.row))?;
            output.write_all(image.tile.payload.as_bytes())?;
        }
        Ok(())
    }

    /// 【输入图片】【整屏失效】终端已删除所有图形时使图片签名失效，下一帧重新放置。
    /// 参数: 无；返回: 无。
    pub(in crate::cli::repl_runtime) fn invalidate_images(&mut self) {
        self.images.clear();
    }
}

/// 【输入图片】【宽度分配】为剩余数量保留位置，限制可见数量防止超宽终端过量解码。
/// 参数: width 为净列数，count 为附件数；返回: 可见与隐藏数量。
fn visible_tiles(width: usize, count: usize) -> (usize, usize) {
    let mut visible = ((width + GAP) / (WIDTH + GAP)).min(count).min(MAX_VISIBLE);
    while visible > 0 && visible < count {
        let badge = (count - visible).to_string().len() + 1;
        if visible * (WIDTH + GAP) + badge <= width {
            break;
        }
        visible -= 1;
    }
    (visible, count - visible)
}

#[cfg(test)]
#[path = "image_strip_tests.rs"]
mod tests;
