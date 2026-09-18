use super::*;

impl ReplChrome {
    /// 底栏整行：左侧模式/上下文/模型/思考，右侧目录。
    ///
    /// 参数:
    /// - `cols`: 终端列数（面板内为扣除彩条后的净宽）
    ///
    /// 返回:
    /// - 已着色状态行
    pub(in crate::cli) fn footer_line(&self, cols: usize) -> String {
        self.footer_line_with_activity(cols, self.activity.as_deref())
    }

    /// 底栏整行，左侧可附加当前工作状态。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `activity`: 如 `Working 12s`
    ///
    /// 返回:
    /// - 已着色状态行
    pub(in crate::cli) fn footer_line_with_activity(
        &self,
        cols: usize,
        activity: Option<&str>,
    ) -> String {
        if let Some(layout) = self.plugin_layout(cols) {
            let activity = activity
                .filter(|text| !text.is_empty())
                .map(|text| format!("{text}  "))
                .unwrap_or_default();
            return self.compose_footer_line(
                cols,
                &format!("{activity}{}", layout.left),
                &layout.right,
                true,
            );
        }
        let left_plain = match activity.filter(|text| !text.is_empty()) {
            Some(activity) => format!(
                "{activity}  {}  {}  {}  {}",
                self.mode_plain(),
                self.context_status(),
                self.model,
                self.thinking
            ),
            None => format!(
                "{}  {}  {}  {}",
                self.mode_plain(),
                self.context_status(),
                self.model,
                self.thinking
            ),
        };
        self.compose_footer_line(cols, &left_plain, &self.directory, false)
    }

    /// 按净宽裁剪并着色底栏左右两段。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    /// - `left_plain`: 左侧纯文本
    /// - `right_plain`: 右侧纯文本；`custom` 表示插件布局
    ///
    /// 返回:
    /// - 已着色状态行
    fn compose_footer_line(
        &self,
        cols: usize,
        left_plain: &str,
        right_plain: &str,
        custom: bool,
    ) -> String {
        let cols = cols.max(1);
        let pad = CHROME_FOOTER_SIDE_PAD.min(cols.saturating_sub(1) / 2);
        let inner = cols.saturating_sub(pad.saturating_mul(2)).max(1);
        // 1. 在扣除左右外边距后的净宽上裁剪，避免贴边
        let (left_text, right_text, gap) = fit_status_segments(&left_plain, &right_plain, inner);
        // 2. 裁剪后再着色，避免 ANSI 干扰宽度计算
        let left = if custom {
            color_model(&left_text)
        } else {
            colorize_left_status(self.mode, &left_text, self.context_ratio)
        };
        let right = if right_text.is_empty() {
            String::new()
        } else {
            color_directory(&right_text)
        };
        format!(
            "{}{left}{}{right}{}",
            " ".repeat(pad),
            " ".repeat(gap),
            " ".repeat(pad)
        )
    }

    /// 【底栏插件】【状态交付】提交有限的显示状态，读取后台已完成的相同快照结果。
    /// @param cols 当前终端列数；返回插件布局或默认底栏标记
    fn plugin_layout(&self, cols: usize) -> Option<sai_plugin_runtime::TuiStatusLine> {
        self.status_plugin
            .as_ref()?
            .render(sai_plugin_runtime::TuiStatusContext {
                columns: cols,
                locale: if is_zh() { "zh-CN" } else { "en-US" }.into(),
                mode: self.mode_plain().into(),
                model: self.model.clone(),
                thinking: self.thinking.clone(),
                directory: self.directory.clone(),
                context_ratio: self.context_ratio,
                context_window_tokens: self.context_window_tokens,
                cache_hit_ratio: self.cache_hit_ratio,
            })
    }

    /// 【底栏插件】【等待上限】后台结果就绪后最多 100 毫秒进入下一次重绘。
    /// @param wait 原有终端等待时间；返回兼顾插件缓存刷新的等待时间
    pub(in crate::cli) fn status_poll_wait(
        &self,
        wait: Option<std::time::Duration>,
    ) -> Option<std::time::Duration> {
        let tick = std::time::Duration::from_millis(100);
        if self
            .status_plugin
            .as_ref()
            .is_some_and(|plugin| plugin.active())
        {
            Some(wait.map_or(tick, |wait| wait.min(tick)))
        } else {
            wait
        }
    }

    /// 【底栏插件】【错误展示】收集一次性诊断，沿用终端普通消息展示入口。
    /// @returns 插件标识与错误文字，不直接写入终端
    pub(in crate::cli) fn status_diagnostics(&self) -> Vec<String> {
        self.status_plugin
            .as_ref()
            .map(|plugin| {
                plugin
                    .take_diagnostics()
                    .into_iter()
                    .map(|error| format!("{}: {}", error.source, error.error))
                    .collect()
            })
            .unwrap_or_default()
    }
}
