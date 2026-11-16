use super::mouse::{column_hitbox, ProviderClick, ProviderHitbox};
use super::*;
use crate::config_tui::layout::FrameRect;
use crate::config_tui::mouse::HitRect;

/// 列表区最少行数。
const MIN_LIST_ROWS: usize = 6;
/// 列表区最多行数，更长的模型列表在栏内滚动。
const MAX_LIST_ROWS: usize = 16;

impl ProviderBrowser<'_> {
    pub(super) fn draw(&mut self, stdout: &mut io::Stdout) -> Result<ProviderHitbox> {
        let (cols, rows) = terminal::size()?;
        // 高度按三栏中最长者计算并封顶，条目少时不再撑满整屏
        let longest = self
            .config
            .providers
            .len()
            .max(self.orgs.len())
            .max(self.models.len())
            .clamp(MIN_LIST_ROWS, MAX_LIST_ROWS);
        // 框内：过滤行 + 空行 + 栏标题 + 列表 + 两行留白 + 状态行
        let frame = crate::config_tui::layout::content_frame(
            cols,
            rows,
            (longest + 6) as u16,
            crate::config_tui::layout::MAX_FRAME_WIDTH,
        );
        // 外框内缩：左右各留边框 + 一格边距，底部留状态行与嵌入式帮助条
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
        // 1. 终端够宽时绘制三栏；宽度之和不超过内容区，保证不重叠
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
            // 2. 窄终端降级为单栏，仅绘制当前激活栏
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
        use crate::config_tui::theme::{help_line, ACCENT, DANGER, MUTED, RESET};
        let help = if self.filter_mode {
            help_line(&[
                ("type", t("filter models", "过滤模型")),
                ("↑↓", t("move", "移动")),
                ("Enter", t("keep filter", "保留过滤")),
                ("Esc", t("clear", "清除")),
            ])
        } else {
            // 删除是破坏性操作，键名用警示色与其它键区分
            let base = help_line(&[
                ("/", t("search", "搜索")),
                ("j/k", t("move", "移动")),
                ("h/l", t("columns", "切栏")),
                ("Space/u", t("use", "设为当前")),
                ("Tab", t("activate", "激活")),
                ("Enter", t("edit", "设置")),
                ("r", t("refresh", "刷新")),
                ("a", t("add", "添加")),
            ]);
            format!(
                "{base}{MUTED} · {RESET}{DANGER}d{RESET} {MUTED}{}{RESET}{MUTED} · {RESET}{ACCENT}q{RESET} {MUTED}{}{RESET}",
                t("delete", "删除"),
                t("back", "返回")
            )
        };
        // 状态行放在框内底部，弱化显示，不与内容抢视线
        queue!(
            stdout,
            MoveTo(
                inner_x,
                frame.y.saturating_add(frame.height.saturating_sub(2))
            ),
            Print(crate::render::readable_terminal_text(
                &(format!("{MUTED}{}{RESET}", truncate(&self.status, inner_w as usize)))
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
        // 光标显隐只在过滤态切换后的首帧发送，逐帧重发会放大终端闪烁
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

    /// 在三栏上方画一条全宽过滤框，避免查询被挤进窄列标题。
    ///
    /// 参数:
    /// - `stdout`: 终端输出
    /// - `x`: 内容区左列
    /// - `y`: 过滤框行
    /// - `width`: 内容区宽度
    ///
    /// 返回:
    /// - 过滤态下的光标坐标
    fn draw_filter_bar(
        &self,
        stdout: &mut io::Stdout,
        x: u16,
        y: u16,
        width: u16,
    ) -> Result<Option<(u16, u16)>> {
        use crate::config_tui::theme::{ACCENT, BOLD, MUTED, PILL_BG, RESET, SELECT_BG};
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
                    &(format!("{SELECT_BG}{ACCENT}{BOLD} {line}{RESET}")).to_string()
                ))
            )?;
            let cursor_x = x
                + 1
                + display_width(&truncate(
                    &format!("{prefix}  {}", self.filter),
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
                    &(format!("{PILL_BG}{ACCENT} {line}{RESET}")).to_string()
                ))
            )?;
            Ok(None)
        } else {
            queue!(
                stdout,
                MoveTo(x, y),
                Print(crate::render::readable_terminal_text(
                    &(format!("{MUTED} {line}{RESET}")).to_string()
                ))
            )?;
            Ok(None)
        }
    }
}
