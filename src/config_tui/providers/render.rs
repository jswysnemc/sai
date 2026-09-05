use super::*;

impl ProviderBrowser<'_> {
    pub(super) fn draw(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        let (cols, rows) = terminal::size()?;
        let frame = crate::config_tui::layout::full_frame(cols, rows);
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
        let list_y = inner_y.saturating_add(2);
        let list_h = inner_h.saturating_sub(2);
        // 1. 终端够宽时绘制三栏；宽度之和不超过内容区，保证不重叠
        if let Some((left_w, mid_w, right_w)) = three_column_widths(inner_w) {
            draw_column(
                stdout,
                inner_x,
                list_y,
                left_w,
                list_h,
                t(" PROVIDERS ", " 供应商 "),
                &providers,
                self.provider_idx,
                self.active_col == 0,
            )?;
            draw_column(
                stdout,
                inner_x + left_w + 1,
                list_y,
                mid_w,
                list_h,
                t(" ORG ", " 组织 "),
                &self.orgs,
                self.org_idx,
                self.active_col == 1,
            )?;
            draw_column(
                stdout,
                inner_x + left_w + mid_w + 2,
                list_y,
                right_w,
                list_h,
                &models_title,
                &models,
                self.model_idx,
                self.active_col == 2,
            )?;
        } else {
            // 2. 窄终端降级为单栏，仅绘制当前激活栏
            let (title, items, selected) = match self.active_col {
                0 => (
                    t(" PROVIDERS ", " 供应商 ").to_string(),
                    &providers,
                    self.provider_idx,
                ),
                1 => (t(" ORG ", " 组织 ").to_string(), &self.orgs, self.org_idx),
                _ => (models_title, &models, self.model_idx),
            };
            draw_column(
                stdout, inner_x, list_y, inner_w, list_h, &title, items, selected, true,
            )?;
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
                ("h/l", t("columns", "切栏")),
                ("j/k", t("move", "移动")),
                ("Tab", t("activate", "激活模型")),
                ("Enter", t("settings", "模型设置")),
                ("/", t("search", "搜索")),
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
            MoveTo(inner_x, rows.saturating_sub(2)),
            Print(format!(
                "{MUTED}{}{RESET}",
                truncate(&self.status, inner_w as usize)
            ))
        )?;
        crate::config_tui::ui::draw_status_bar(stdout, &frame, &help)?;
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
        stdout.flush()?;
        Ok(())
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
        use crate::config_tui::theme::{ACCENT, BOLD, MUTED, RESET, SELECT_BG};
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
                Print(format!("{SELECT_BG}{ACCENT}{BOLD} {line}{RESET}"))
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
        } else {
            queue!(
                stdout,
                MoveTo(x, y),
                Print(format!("{MUTED} {line}{RESET}"))
            )?;
            Ok(None)
        }
    }
}
