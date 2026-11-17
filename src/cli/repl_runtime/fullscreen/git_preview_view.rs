//! 固定右侧审查区：文件清单和差异正文各自滚动，左侧聊天保持原样。

use super::git_preview::{DiffMode, GitPreview, ReviewFocus, ReviewHover};
use super::layout::FullscreenLayout;
use super::sidebar_git::GitSummary;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_style::clip;
use super::sidebar_widgets::{changes, columns, highlight, item, safe_text};

pub(super) struct PreviewLayout {
    pub(super) left: usize,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) files_top: usize,
    pub(super) files_visible: usize,
    pub(super) diff_top: usize,
    pub(super) visible: usize,
    pub(super) chrome: bool,
    pub(super) header_height: usize,
}

impl PreviewLayout {
    /// 【终端审查】【固定区域】右侧审查使用布局预留列，标题、清单和正文分别占固定区域。
    /// 参数: layout 为分栏布局，count 为文件数量；返回: 审查区几何。
    pub(super) fn compute(layout: &FullscreenLayout, count: usize) -> Self {
        let side = layout.sidebar.unwrap_or(super::layout::SidebarLayout {
            left: layout.cols / 2,
            width: layout.cols - layout.cols / 2,
        });
        let height = usize::from(layout.rows);
        let header_height = super::pane_header::height(height);
        let chrome = height >= header_height + 8;
        let files_top = if chrome {
            header_height + 2
        } else {
            header_height
        };
        let files_visible = if chrome {
            (height / 4)
                .clamp(1, 6)
                .min(count)
                .min(height.saturating_sub(header_height + 8))
        } else {
            0
        };
        let diff_top = if chrome {
            files_top + files_visible + 3
        } else if height >= 4 {
            3
        } else {
            1
        };
        Self {
            left: usize::from(side.left),
            width: usize::from(side.width),
            height,
            files_top,
            files_visible,
            diff_top,
            visible: height.saturating_sub(diff_top + 1),
            chrome,
            header_height,
        }
    }

    /// 【终端审查】【范围命中】审查仅接管右侧区域，左侧聊天鼠标事件继续处理。
    /// 参数: column/row 为坐标；返回: 是否在审查区。
    pub(super) fn contains(&self, column: u16, row: u16) -> bool {
        usize::from(column) >= self.left
            && usize::from(column) < self.left + self.width
            && usize::from(row) < self.height
    }

    /// 【终端审查】【返回按钮】在首行右侧（侧栏开关左方）提供点击返回概览侧栏的入口。
    /// 参数: 无；返回: 当前宽度下可完整显示的返回控件文案，过窄时返回空串。
    pub(super) fn back_label(&self) -> &'static str {
        let usable = self.width.saturating_sub(6);
        if usable >= 28 {
            super::sidebar_i18n::review_back_label(false)
        } else if usable >= 20 {
            super::sidebar_i18n::review_back_label(true)
        } else {
            ""
        }
    }

    /// 【终端审查】【返回命中】判断首行点击是否落在返回概览控件上（避开最右侧 4 列侧栏开关）。
    /// 参数: column/row 为屏幕坐标；返回: 是否命中返回控件。
    pub(super) fn back_at(&self, column: u16, row: u16) -> bool {
        if row != 0 || self.height == 0 || !self.contains(column, row) {
            return false;
        }
        let label = self.back_label();
        if label.is_empty() {
            return false;
        }
        let label_width = crate::cli::repl_text::visible_width(label);
        let usable = self.width.saturating_sub(6);
        let start = self.left + 2 + usable.saturating_sub(label_width);
        let end = self.left + 2 + usable;
        (start..end).contains(&usize::from(column))
    }

    /// 【终端审查】【比较控件】模式切换控件固定在标题下方，按显示列宽对齐中英文标签。
    /// 参数: column/row 为坐标；返回: 命中模式。
    pub(super) fn mode_at(&self, column: u16, row: u16) -> Option<DiffMode> {
        if !self.chrome || usize::from(row) != self.header_height || !self.contains(column, row) {
            return None;
        }
        if usize::from(column) < self.left + 2 {
            return None;
        }
        let relative = usize::from(column).saturating_sub(self.left + 2);
        let mut start = 0;
        for mode in [DiffMode::Working, DiffMode::Staged, DiffMode::Unstaged] {
            let width = crate::cli::repl_text::visible_width(self.mode_label(mode)) + 4;
            if relative >= start && relative < start + width {
                return Some(mode);
            }
            start += width + 1;
        }
        None
    }

    /// 【终端审查】【控件适配】按右栏净宽缩短模式名称，保证窄屏三种模式都完整可见。
    /// 参数: mode 为比较模式；返回: 当前尺寸对应的控件名称。
    pub(super) fn mode_label(&self, mode: DiffMode) -> &'static str {
        if self.width >= 42 {
            return mode.label();
        }
        if self.width >= 38 {
            return match mode {
                DiffMode::Working => crate::i18n::text("Working", "工作树"),
                _ => mode.label(),
            };
        }
        match mode {
            DiffMode::Working => "W",
            DiffMode::Staged => "S",
            DiffMode::Unstaged => "U",
        }
    }

    /// 【终端审查】【悬停目标】只给实际可点击区域反色，分隔线、正文和空白不触发。
    /// 参数: column/row 为坐标，start 为文件偏移，count 为文件数量；返回: 悬停控件。
    pub(super) fn hover_at(
        &self,
        column: u16,
        row: u16,
        start: usize,
        count: usize,
    ) -> Option<ReviewHover> {
        if !self.contains(column, row) || usize::from(column) < self.left + 2 {
            return None;
        }
        if self.back_at(column, row) {
            return Some(ReviewHover::Back);
        }
        if let Some(mode) = self.mode_at(column, row) {
            return Some(ReviewHover::Mode(mode));
        }
        if self.in_files(row) && usize::from(column) < self.left + self.width.saturating_sub(1) {
            let index = start + usize::from(row) - self.files_top;
            return (index < count).then_some(ReviewHover::File(index));
        }
        None
    }

    /// 【终端审查】【文件范围】文件列表拥有独立滚动范围，不包括标题或差异区。
    /// 参数: row 为屏幕行；返回: 是否在文件列表。
    pub(super) fn in_files(&self, row: u16) -> bool {
        let row = usize::from(row);
        row >= self.files_top && row < self.files_top + self.files_visible
    }

    /// 【终端审查】【差异范围】只有差异正文可触发差异滚动，底栏保持固定。
    /// 参数: row 为屏幕行；返回: 是否在差异正文。
    pub(super) fn in_diff(&self, row: u16) -> bool {
        let row = usize::from(row);
        row >= self.diff_top && row < self.diff_top + self.visible
    }
}

/// 【终端审查】【右侧绘制】直接填充右侧预留列，不覆盖、遮挡或清除左侧聊天。
/// 参数: preview 为状态，git 为文件快照，layout 为分栏布局，rows 为屏幕行；返回: 无。
pub(super) fn overlay(
    preview: &GitPreview,
    git: &GitSummary,
    layout: &FullscreenLayout,
    rows: &mut [String],
) {
    if preview.path.is_none() {
        return;
    }
    let geometry = PreviewLayout::compute(layout, git.files.len());
    let width = geometry.width.saturating_sub(3);
    let palette = SidebarPalette::current();
    let panel = panel_rows(preview, git, layout);
    for (row, output) in rows.iter_mut().enumerate().take(geometry.height) {
        let left = crate::render::ansi_clip::clip_ansi_to_width(output, geometry.left, "");
        let padding = geometry
            .left
            .saturating_sub(crate::cli::repl_text::visible_width(&left));
        let text = clip(&panel[row], width);
        *output = format!(
            "{left}{}\x1b[0m{}│\x1b[0m {text}{} ",
            " ".repeat(padding),
            fg(palette.border),
            " ".repeat(width.saturating_sub(crate::cli::repl_text::visible_width(&text)))
        );
    }
}

/// 【终端审查】【完整面板】生成固定右侧区域，供正文和输入框右侧各自绘制。
/// 参数: preview 为状态，git 为快照，layout 为布局；返回: 全高度的审查行。
pub(super) fn panel_rows(
    preview: &GitPreview,
    git: &GitSummary,
    layout: &FullscreenLayout,
) -> Vec<String> {
    let Some(path) = preview.path.as_deref() else {
        return Vec::new();
    };
    let geometry = PreviewLayout::compute(layout, git.files.len());
    let width = geometry.width.saturating_sub(3);
    let palette = SidebarPalette::current();
    let mut panel = vec![String::new(); geometry.height];
    for (index, title) in super::pane_header::render(
        super::sidebar_i18n::review_heading(),
        width,
        geometry.height,
    )
    .into_iter()
    .enumerate()
    {
        panel[index] = title;
    }
    let back_label = geometry.back_label();
    if !back_label.is_empty() && !panel.is_empty() {
        let raw_back = format!("{}{back_label}\x1b[39m", fg(palette.muted));
        let styled_back = if preview.hovered == Some(ReviewHover::Back) {
            highlight(&raw_back, false)
        } else {
            raw_back
        };
        let left_title = panel[0].trim_end().to_string();
        panel[0] = columns(&left_title, &format!("{styled_back}   "), width);
    }
    if geometry.chrome {
        panel[geometry.header_height] = [DiffMode::Working, DiffMode::Staged, DiffMode::Unstaged]
            .into_iter()
            .map(|mode| {
                let text = format!("[ {} ]", geometry.mode_label(mode));
                let text = if mode == preview.mode {
                    format!("{}\x1b[1m{text}\x1b[22m\x1b[39m", fg(palette.accent))
                } else {
                    format!("{}{text}\x1b[39m", fg(palette.muted))
                };
                if preview.hovered == Some(ReviewHover::Mode(mode)) {
                    highlight(&text, false)
                } else {
                    text
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        panel[geometry.header_height + 1] = columns(
            &super::sidebar_i18n::files_changed_summary(git.files.len()),
            &super::sidebar_git_view::totals(git),
            width,
        );
        for (offset, (_, file)) in git
            .files
            .iter()
            .skip(preview.file_scroll)
            .take(geometry.files_visible)
            .enumerate()
        {
            let stats = changes(git.changes.get(file));
            let name_width = width.saturating_sub(crate::cli::repl_text::visible_width(&stats) + 3);
            let text = columns(
                &format!(
                    "{}{} {}",
                    fg(palette.muted),
                    if file == path { "›" } else { " " },
                    fit_path(file, name_width)
                ),
                &stats,
                width,
            );
            let text = if file == path {
                highlight(&text, true)
            } else {
                text
            };
            panel[geometry.files_top + offset] =
                if preview.hovered == Some(ReviewHover::File(preview.file_scroll + offset)) {
                    highlight(&text, false)
                } else {
                    text
                };
        }
    }
    if geometry.diff_top >= 3 {
        let summary = preview
            .content
            .as_ref()
            .map(|content| {
                changes(Some(&crate::web::GitFileStats {
                    added: content.added as u64,
                    removed: content.removed as u64,
                    binary: false,
                }))
            })
            .unwrap_or_default();
        let rule = format!("{}{}\x1b[0m", fg(palette.border), "─".repeat(width));
        if geometry.chrome {
            panel[geometry.diff_top - 3] = rule.clone();
        }
        panel[geometry.diff_top - 2] = columns(
            &format!(
                "\x1b[1m{}\x1b[22m",
                fit_path(
                    path,
                    width.saturating_sub(
                        crate::cli::repl_text::visible_width(&summary)
                            + usize::from(!summary.is_empty())
                    )
                )
            ),
            &summary,
            width,
        );
        panel[geometry.diff_top - 1] = rule;
    }
    for offset in 0..geometry.visible {
        let text = if let Some(error) = &preview.error {
            if offset == 0 {
                format!("{}{}", fg(palette.removed), safe_text(error))
            } else {
                String::new()
            }
        } else if let Some(content) = &preview.content {
            content
                .lines
                .get(preview.scroll + offset)
                .map(|line| {
                    super::git_preview_code::render(
                        line,
                        content
                            .code
                            .get(preview.scroll + offset)
                            .map(String::as_str),
                        preview.horizontal,
                        width,
                    )
                })
                .unwrap_or_default()
        } else if offset == 0 {
            super::sidebar_i18n::loading_changes().into()
        } else {
            String::new()
        };
        let text =
            super::git_preview_selection::highlight(preview, preview.scroll + offset, text, width);
        panel[geometry.diff_top + offset] = item(&text, width, false);
    }
    if let Some(footer) = panel.last_mut() {
        use crate::i18n::text as t;
        let total = preview
            .content
            .as_ref()
            .map_or(0, |content| content.lines.len());
        let focus = match preview.focus {
            Some(ReviewFocus::Files) => t("files", "文件"),
            Some(ReviewFocus::Diff) => t("diff", "差异"),
            None => t("chat", "聊天"),
        };
        let truncated = preview
            .content
            .as_ref()
            .is_some_and(|content| content.truncated);
        let range = format!(
            "{}-{}/{total}",
            preview.scroll + usize::from(total > 0),
            (preview.scroll + geometry.visible).min(total),
        );
        let trunc_suffix = if truncated {
            t(" · truncated", " · 已截断")
        } else {
            ""
        };
        let full = format!(
            "{range} · {} · F6 {focus} · {}{trunc_suffix}",
            t("Tab mode", "Tab 切模式"),
            t("Esc overview", "Esc 返回"),
        );
        let medium = format!(
            "{range} · F6 {focus} · {}{trunc_suffix}",
            t("Esc back", "Esc 返回"),
        );
        let body = if crate::cli::repl_text::visible_width(&full) <= width {
            full
        } else if crate::cli::repl_text::visible_width(&medium) <= width {
            medium
        } else {
            format!("{range} · F6 {focus}{trunc_suffix}")
        };
        *footer = format!("{}{}", fg(palette.muted), clip(&body, width));
    }
    panel
}

/// 【终端审查】【路径裁剪】窄屏从目录前端省略，优先保留末尾文件名，避免同目录文件无法区分。
/// 参数: path 为原始路径，width 为可用列数；返回: 不超过列宽的路径标签。
fn fit_path(path: &str, width: usize) -> String {
    let text = safe_text(path);
    let size = crate::cli::repl_text::visible_width(&text);
    if size <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    format!(
        "…{}",
        super::git_preview_code::skip_columns(&text, size.saturating_sub(width - 1))
    )
}
