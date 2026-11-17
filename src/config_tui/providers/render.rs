//! 供应商与模型三栏浏览器的屏幕渲染、统一底栏热区与过滤框真实光标定位。

use super::mouse::{column_hitbox, ProviderClick, ProviderHitbox};
use super::*;
use crate::config_tui::layout::FrameRect;
use crate::config_tui::mouse::HitRect;
use crate::config_tui::search::split_at_char;
use crate::config_tui::theme::{help_line, help_separator, ThemePalette, BOLD, RESET};
use crate::i18n::text as t;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};

/// 列表区最少行数。
const MIN_LIST_ROWS: usize = 6;
/// 列表区最多行数，更长的模型列表在栏内滚动。
const MAX_LIST_ROWS: usize = 16;

/// 【供应商配置】【底栏文案】生成供应商浏览器的状态栏帮助串，统一使用 `help_separator` 确保鼠标热区切分准确。
///
/// 参数:
/// - `filter_mode`: 是否处于过滤输入态
///
/// 返回:
/// - 带分段样式与标准分隔符的底栏文本
pub(super) fn browser_help_line(filter_mode: bool) -> String {
    if filter_mode {
        help_line(&[
            ("type", t("filter models", "过滤模型")),
            ("↑↓", t("move", "移动")),
            ("Enter", t("keep filter", "保留过滤")),
            ("Esc", t("clear", "清除")),
        ])
    } else {
        let palette = ThemePalette::current();
        let sep = help_separator();
        let base = help_line(&[
            ("/", t("search", "搜索")),
            ("j/k", t("move", "移动")),
            ("h/l", t("cols", "切栏")),
            ("Space/u", t("use", "设为当前")),
            ("Tab", t("toggle", "激活")),
            ("Enter", t("edit", "设置")),
            ("r", t("sync", "刷新")),
            ("a", t("add", "添加")),
        ]);
        // 1. 【供应商配置】【热区对齐】使用统一 help_separator 分隔删除与返回键，并控制英文总宽不超过面板上限
        format!(
            "{base}{sep}{}d{RESET} {}{}{RESET}{sep}{}q{RESET} {}{}{RESET}",
            palette.danger,
            palette.muted,
            t("delete", "删除"),
            palette.accent,
            palette.muted,
            t("back", "返回")
        )
    }
}

impl ProviderBrowser<'_> {
    /// 【供应商配置】【画面绘制】渲染三栏浏览器、过滤框、状态摘要及底栏热区。
    ///
    /// 参数:
    /// - `stdout`: 终端输出流
    ///
    /// 返回:
    /// - 当前帧命中几何
    pub(super) fn draw(&mut self, stdout: &mut io::Stdout) -> Result<ProviderHitbox> {
        let palette = ThemePalette::current();
        let (cols, rows) = terminal::size()?;
        let longest = self
            .config
            .providers
            .len()
            .max(self.orgs.len())
            .max(self.models.len())
            .clamp(MIN_LIST_ROWS, MAX_LIST_ROWS);
        let frame = crate::config_tui::layout::content_frame(
            cols,
            rows,
            (longest + 6) as u16,
            crate::config_tui::layout::MAX_FRAME_WIDTH,
        );
        let inner_x = frame.x.saturating_add(2);
        let inner_y = frame.y.saturating_add(1);
        let inner_w = frame.width.saturating_sub(4);
        let inner_h = frame.height.saturating_sub(4);
        let providers = self
            .config
            .providers
            .iter()
            .map(|provider| {
                let active = if provider.id == self.config.active_provider {
                    "● "
                } else {
                    "  "
                };
                let disabled = if provider.enabled {
                    ""
                } else {
                    t(" · disabled", " · 已停用")
                };
                format!("{active}{}{disabled}", provider.display_name)
            })
            .collect::<Vec<_>>();
        let models = self
            .models
            .iter()
            .map(|model| {
                let current = self
                    .config
                    .providers
                    .get(self.provider_idx)
                    .map(|provider| provider.default_model == model.full)
                    .unwrap_or(false);
                let active = self
                    .config
                    .providers
                    .get(self.provider_idx)
                    .map(|provider| provider.models.iter().any(|item| item == &model.full))
                    .unwrap_or(false);
                if current && active {
                    format!(
                        "{} [{} {}]",
                        model.name,
                        t("current", "当前"),
                        t("active", "激活")
                    )
                } else if current {
                    format!("{} [{}]", model.name, t("current", "当前"))
                } else if active {
                    format!("{} [{}]", model.name, t("active", "激活"))
                } else {
                    model.name.clone()
                }
            })
            .collect::<Vec<_>>();

        let models_title = t(" MODELS ", " 模型 ").to_string();
        crate::config_tui::ui::begin_synced_frame(stdout)?;
        queue!(stdout, Clear(ClearType::All))?;
        crate::config_tui::ui::draw_box(
            stdout,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            t("PROVIDERS & MODELS", "供应商与模型"),
        )?;
        let filter_cursor = self.draw_filter_bar(stdout, inner_x, inner_y, inner_w)?;
        let filter_bar = HitRect {
            x: inner_x,
            y: inner_y,
            width: inner_w,
            height: 1,
        };
        let list_y = inner_y.saturating_add(2);
        let list_h = inner_h.saturating_sub(2);
        let mut headers = [None, None, None];
        let mut columns = [None, None, None];
        let hover_row_for = |col: usize| match self.hovered {
            Some(ProviderClick::Row(hover_col, row)) if hover_col == col => Some(row),
            _ => None,
        };
        let header_hovered = |col: usize| self.hovered == Some(ProviderClick::ColumnHeader(col));
        // 1. 【供应商配置】【三栏绘制】终端足够宽时并排绘制供应商、组织与模型三栏
        if let Some((left_w, mid_w, right_w)) = three_column_widths(inner_w) {
            let area0 = FrameRect {
                x: inner_x,
                y: list_y,
                width: left_w,
                height: list_h,
            };
            crate::config_tui::ui::draw_column_interactive(
                stdout,
                area0,
                t(" PROVIDERS ", " 供应商 "),
                &providers,
                self.provider_idx,
                hover_row_for(0),
                header_hovered(0),
                self.active_col == 0,
            )?;
            let (h0, c0) = column_hitbox(area0, self.provider_idx, providers.len());
            headers[0] = Some(h0);
            columns[0] = Some(c0);

            let area1 = FrameRect {
                x: inner_x + left_w + 1,
                y: list_y,
                width: mid_w,
                height: list_h,
            };
            crate::config_tui::ui::draw_column_interactive(
                stdout,
                area1,
                t(" ORG ", " 组织 "),
                &self.orgs,
                self.org_idx,
                hover_row_for(1),
                header_hovered(1),
                self.active_col == 1,
            )?;
            let (h1, c1) = column_hitbox(area1, self.org_idx, self.orgs.len());
            headers[1] = Some(h1);
            columns[1] = Some(c1);

            let area2 = FrameRect {
                x: inner_x + left_w + mid_w + 2,
                y: list_y,
                width: right_w,
                height: list_h,
            };
            crate::config_tui::ui::draw_column_interactive(
                stdout,
                area2,
                &models_title,
                &models,
                self.model_idx,
                hover_row_for(2),
                header_hovered(2),
                self.active_col == 2,
            )?;
            let (h2, c2) = column_hitbox(area2, self.model_idx, models.len());
            headers[2] = Some(h2);
            columns[2] = Some(c2);
        } else {
            // 2. 【供应商配置】【窄屏单栏】超窄终端降级为单栏，仅绘制当前聚焦栏
            let area = FrameRect {
                x: inner_x,
                y: list_y,
                width: inner_w,
                height: list_h,
            };
            let col = self.active_col.min(2);
            let (title, items, selected) = match col {
                0 => (
                    t(" PROVIDERS ", " 供应商 ").to_string(),
                    &providers,
                    self.provider_idx,
                ),
                1 => (t(" ORG ", " 组织 ").to_string(), &self.orgs, self.org_idx),
                _ => (models_title, &models, self.model_idx),
            };
            crate::config_tui::ui::draw_column_interactive(
                stdout,
                area,
                &title,
                items,
                selected,
                hover_row_for(col),
                header_hovered(col),
                true,
            )?;
            let (hdr, hb) = column_hitbox(area, selected, items.len());
            headers[col] = Some(hdr);
            columns[col] = Some(hb);
        }

        let help = browser_help_line(self.filter_mode);
        queue!(
            stdout,
            MoveTo(
                inner_x,
                frame.y.saturating_add(frame.height.saturating_sub(2))
            ),
            Print(crate::render::readable_terminal_text(
                &(format!(
                    "{}{}{RESET}",
                    palette.muted,
                    truncate(&self.status, inner_w as usize)
                ))
                .to_string()
            ))
        )?;
        crate::config_tui::ui::draw_status_bar_interactive(
            stdout,
            &frame,
            &help,
            self.hovered_footer,
        )?;
        let footer = crate::config_tui::ui::status_bar_hits(&frame, &help);
        if let Some((cx, cy)) = filter_cursor {
            if !self.cursor_visible {
                queue!(stdout, Show)?;
                self.cursor_visible = true;
            }
            queue!(stdout, MoveTo(cx, cy))?;
        } else if self.cursor_visible {
            queue!(stdout, Hide)?;
            self.cursor_visible = false;
        }
        crate::config_tui::ui::end_synced_frame(stdout)?;
        Ok(ProviderHitbox {
            frame: HitRect::from(frame),
            filter_bar,
            headers,
            columns,
            footer,
        })
    }

    /// 【供应商配置】【过滤框绘制】在三栏上方绘制全宽搜索栏并按游标字符下标返回真实光标坐标。
    ///
    /// 参数:
    /// - `stdout`: 终端输出
    /// - `x`: 内容区左列
    /// - `y`: 过滤框所在行
    /// - `width`: 内容区宽度
    ///
    /// 返回:
    /// - 过滤编辑态下的 `(列, 行)` 光标坐标
    fn draw_filter_bar(
        &self,
        stdout: &mut io::Stdout,
        x: u16,
        y: u16,
        width: u16,
    ) -> Result<Option<(u16, u16)>> {
        let palette = ThemePalette::current();
        let prefix = format!("/ {}", t("Search models", "搜索模型"));
        let query = if self.filter.is_empty() && !self.filter_mode {
            t("type to filter", "输入以过滤")
        } else {
            self.filter.as_str()
        };
        let count = if self.filter.is_empty() {
            String::new()
        } else {
            format!("  {} {}", self.models.len(), t("matches", "项"))
        };
        let field_w = (width as usize).saturating_sub(display_width(&count));
        let body = truncate(&format!("{prefix}  {query}"), field_w.saturating_sub(1));
        let line = format!("{}{count}", pad(&body, field_w.saturating_sub(1)));
        if self.filter_mode {
            queue!(
                stdout,
                MoveTo(x, y),
                Print(crate::render::readable_terminal_text(
                    &(format!(
                        "{}{}{BOLD} {line}{RESET}",
                        palette.select_bg, palette.accent
                    ))
                    .to_string()
                ))
            )?;
            let before_cursor = split_at_char(&self.filter, self.filter_cursor).0;
            let cursor_x = x
                + 1
                + display_width(&truncate(
                    &format!("{prefix}  {before_cursor}"),
                    field_w.saturating_sub(1),
                )) as u16;
            Ok(Some((
                cursor_x.min(x.saturating_add(width.saturating_sub(1))),
                y,
            )))
        } else if self.hovered == Some(ProviderClick::FilterBar) {
            queue!(
                stdout,
                MoveTo(x, y),
                Print(crate::render::readable_terminal_text(
                    &(format!("{}{} {line}{RESET}", palette.hover_bg, palette.accent)).to_string()
                ))
            )?;
            Ok(None)
        } else {
            queue!(
                stdout,
                MoveTo(x, y),
                Print(crate::render::readable_terminal_text(
                    &(format!("{} {line}{RESET}", palette.muted)).to_string()
                ))
            )?;
            Ok(None)
        }
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    /// 【供应商配置】【底栏热区回归测试】验证中英文环境下添加、删除与返回键各自映射独立热区。
    #[test]
    fn browser_footer_splits_add_delete_and_back_actions_accurately() {
        let frame = FrameRect {
            x: 1,
            y: 0,
            width: 118,
            height: 20,
        };
        for locale in [crate::i18n::Locale::En, crate::i18n::Locale::Zh] {
            crate::i18n::with_locale(locale, || {
                let help = browser_help_line(false);
                let hits = crate::config_tui::ui::status_bar_hits(&frame, &help);

                let mut found_add = false;
                let mut found_delete = false;
                let mut found_back = false;
                for col in 0..118 {
                    match hits.hit(col, 19) {
                        Some(KeyCode::Char('a')) => found_add = true,
                        Some(KeyCode::Char('d')) => found_delete = true,
                        Some(KeyCode::Char('q')) => found_back = true,
                        _ => {}
                    }
                }
                assert!(found_add, "应包含 a 添加热区 ({locale:?})");
                assert!(found_delete, "应包含独立的 d 删除热区 ({locale:?})");
                assert!(found_back, "应包含独立的 q 返回热区 ({locale:?})");
            });
        }
    }
}
