use super::cell::HistoryCell;
use super::line::AnsiLine;
use super::tool_cell::ToolCell;
use super::TranscriptRenderOptions;

/// 单个 cell 在特定宽度与渲染选项下的缓存结果。
#[derive(Clone, Debug)]
struct CachedRender {
    width: usize,
    options: TranscriptRenderOptions,
    lines: Vec<AnsiLine>,
}

/// 与 transcript cells 平行的渲染缓存。
///
/// 定稿 cell 的渲染结果只与宽度和选项相关，缓存后同一宽度下重复同步
/// 不再触发 markdown 与语法高亮的全量重渲染。
#[derive(Default)]
pub(super) struct RenderCache {
    entries: Vec<Option<CachedRender>>,
}

impl RenderCache {
    /// 追加一个空缓存槽位，与新 push 的 cell 对齐。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn push_slot(&mut self) {
        self.entries.push(None);
    }

    /// 失效指定 cell 的缓存。
    ///
    /// 参数:
    /// - `index`: cell 下标
    ///
    /// 返回:
    /// - 无
    pub(super) fn invalidate(&mut self, index: usize) {
        if let Some(entry) = self.entries.get_mut(index) {
            *entry = None;
        }
    }

    /// 清空全部缓存。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    /// 返回指定 cell 的视觉行数，缓存未命中时渲染并写回。
    ///
    /// 参数:
    /// - `index`: cell 下标
    /// - `cell`: 对应 cell 数据
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    ///
    /// 返回:
    /// - 该 cell 在当前宽度下的行数
    pub(super) fn count_for(
        &mut self,
        index: usize,
        cell: &HistoryCell,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> usize {
        if is_live_subagent(cell) {
            return cell.display_lines(width, options).len();
        }
        while self.entries.len() <= index {
            self.entries.push(None);
        }
        if let Some(cached) = &self.entries[index] {
            if cached.width == width && cached.options == *options {
                return cached.lines.len();
            }
        }
        let lines = cell.display_lines(width, options);
        let count = lines.len();
        self.entries[index] = Some(CachedRender {
            width,
            options: *options,
            lines,
        });
        count
    }

    /// 返回指定 cell 的预换行渲染行，缓存未命中时重新渲染。
    ///
    /// 参数:
    /// - `index`: cell 下标
    /// - `cell`: 对应 cell 数据
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    ///
    /// 返回:
    /// - 预换行 ANSI 行
    pub(super) fn lines_for(
        &mut self,
        index: usize,
        cell: &HistoryCell,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> Vec<AnsiLine> {
        // 1. 后台子智能体单元的渲染依赖进程内快照，禁止缓存以免展示过期状态
        if is_live_subagent(cell) {
            return cell.display_lines(width, options);
        }
        while self.entries.len() <= index {
            self.entries.push(None);
        }
        if let Some(cached) = &self.entries[index] {
            if cached.width == width && cached.options == *options {
                return cached.lines.clone();
            }
        }
        // 2. 缓存未命中时渲染并写回
        let lines = cell.display_lines(width, options);
        self.entries[index] = Some(CachedRender {
            width,
            options: *options,
            lines: lines.clone(),
        });
        lines
    }
}

/// 判断 cell 是否为仍会产生后台更新的子智能体单元。
///
/// 参数:
/// - `cell`: transcript cell
///
/// 返回:
/// - 仍在后台更新时返回 true
fn is_live_subagent(cell: &HistoryCell) -> bool {
    matches!(
        cell,
        HistoryCell::Tool(ToolCell::Subagent(subagent)) if subagent.has_live_updates()
    )
}
