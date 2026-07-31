use super::viewport::TerminalSize;
use super::ReplRuntime;
use crate::agent::AgentMode;
use crate::render::content_indent::CONTENT_LEFT_INDENT;
use crate::render::transcript::{
    AnsiLine, DisplayWindow, TranscriptMode, TranscriptRenderOptions, TranscriptStore,
};

/// 按终端宽度渲染带左侧留白的 transcript 窗口。
///
/// 参数:
/// - `transcript`: 当前 TUI transcript
/// - `width`: 终端总列数
/// - `options`: transcript 渲染选项
/// - `min_rows`: 窗口至少覆盖的行数
/// - `max_start`: 窗口首行允许的最大全局行号
/// - `live_cap`: 临时 live 预览的最大行数
///
/// 返回:
/// - 已按净正文宽度折行并增加左侧留白的窗口
pub(super) fn display_window(
    transcript: &mut TranscriptStore,
    width: usize,
    options: &TranscriptRenderOptions,
    min_rows: usize,
    max_start: usize,
    live_cap: usize,
) -> DisplayWindow {
    // 【终端】【响应式布局】1. 按终端总宽度计算实际引导区与正文净宽度
    let padding = left_padding_columns(width);
    let content_width = width.saturating_sub(padding).max(1);
    let mut window = transcript.display_window_with_live_cap(
        content_width,
        options,
        min_rows,
        max_start,
        live_cap,
    );
    // 【终端】【响应式布局】2. 宽终端保留完整引导区，窄终端按可用列数压缩
    window.lines = window
        .lines
        .into_iter()
        .map(|line| {
            AnsiLine::new(
                crate::render::content_indent::align_to_guide_column_with_width(
                    line.as_str(),
                    padding,
                ),
            )
        })
        .collect();
    window
}

impl ReplRuntime {
    /// 按指定宽度渲染完整 transcript（供备用屏浏览模式使用）。
    ///
    /// 参数:
    /// - `width`: 目标终端列数
    ///
    /// 返回:
    /// - row cap 范围内带左侧留白的预折行 ANSI 行
    pub(in crate::cli) fn transcript_pager_lines(&mut self, width: usize) -> Vec<String> {
        let width = width.max(8);
        let min_rows = self.transcript.row_cap();
        // 回看场景展开全部折叠块（思考正文、命令输出等），绕过渲染缓存
        let window = crate::render::render_expand::with_expanded_render(|| {
            display_window(
                &mut self.transcript,
                width,
                &self.options,
                min_rows,
                usize::MAX,
                usize::MAX,
            )
        });
        window
            .lines
            .iter()
            .map(|line| line.as_str().to_string())
            .collect()
    }

    /// 计算当前终端尺寸下 composer 需要保留的行数。
    ///
    /// 参数:
    /// - `size`: 当前终端尺寸
    ///
    /// 返回:
    /// - 不超过终端高度的 composer 行数
    pub(super) fn composer_height_for(&self, size: TerminalSize) -> u16 {
        self.composer
            .as_ref()
            .map(|composer| composer.height(usize::from(size.cols)))
            .unwrap_or(0)
            .min(size.rows)
    }
}

/// 计算 live 预览允许占用的最大行数。
///
/// live 预览行一旦进入原生 scrollback 便无法修补，上限保证只有定稿内容进入回滚区。
///
/// 参数:
/// - `size`: 当前终端尺寸
///
/// 返回:
/// - live 预览行数上限
pub(super) fn live_preview_cap(size: TerminalSize) -> usize {
    (usize::from(size.rows) / 2).max(8)
}

/// 将 AgentMode 映射为 transcript 输入模式。
///
/// 参数:
/// - `mode`: 当前 Agent 权限模式
///
/// 返回:
/// - transcript 使用的输入模式
pub(super) fn transcript_mode(mode: AgentMode) -> TranscriptMode {
    match mode {
        AgentMode::Plan => TranscriptMode::Plan,
        AgentMode::Audited | AgentMode::AutoAudit | AgentMode::Yolo => TranscriptMode::Yolo,
    }
}

/// 计算当前终端宽度允许的左侧留白列数。
///
/// 参数:
/// - `width`: 终端总列数
///
/// 返回:
/// - 宽度足够时返回引导线与间隔列宽，窄终端至少保留一列正文
fn left_padding_columns(width: usize) -> usize {
    CONTENT_LEFT_INDENT.min(width.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::display_window;
    use crate::cli::repl_text::visible_width;
    use crate::llm::{ChatStreamChunk, ChatStreamKind};
    use crate::render::activity_animation::strip_ansi_for_test;
    use crate::render::transcript::{TranscriptRenderOptions, TranscriptStore};
    use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

    /// 构造 transcript 测试渲染选项。
    ///
    /// 返回:
    /// - 展开正文所需的渲染选项
    fn options() -> TranscriptRenderOptions {
        TranscriptRenderOptions {
            reasoning_mode: ReasoningDisplayMode::Full,
            tool_call_mode: ToolCallDisplayMode::Full,
        }
    }

    /// 验证普通正文位于引导线右侧，且折行后不超过终端宽度。
    #[test]
    fn transcript_lines_have_left_padding_without_width_overflow() {
        let mut transcript = TranscriptStore::new(100);
        transcript.push_meta("1234567890".to_string());

        let window = display_window(&mut transcript, 8, &options(), 100, usize::MAX, usize::MAX);

        assert!(window.lines.len() >= 2);
        for line in &window.lines {
            assert!(line.as_str().starts_with("  "), "历史行没有位于引导线右侧");
            assert!(visible_width(line.as_str()) <= 8, "历史行超出终端宽度");
        }
    }

    /// 验证用户和工具引导符号位于引导线左侧，正文从右侧开始。
    #[test]
    fn transcript_markers_stay_left_of_content_column() {
        let mut transcript = TranscriptStore::new(100);
        transcript.push_user_echo(
            super::transcript_mode(crate::agent::AgentMode::Yolo),
            "input".to_string(),
        );
        transcript.push_meta("plain".to_string());

        let window = display_window(&mut transcript, 80, &options(), 100, usize::MAX, usize::MAX);
        let marker = window
            .lines
            .iter()
            .find(|line| line.as_str().contains('●'))
            .expect("user marker should be rendered");
        let plain = window
            .lines
            .iter()
            .find(|line| line.as_str().contains("plain"))
            .expect("plain content should be rendered");

        assert!(marker.as_str().starts_with("\x1b[38;5;208m●"));
        assert!(plain.as_str().starts_with("  "));
    }

    /// 【终端】【正文引导测试】验证真实 TUI 布局只扣除两列，并统一流式与定稿正文。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn assistant_body_uses_reserved_guide_columns() {
        let mut transcript = TranscriptStore::new(100);
        transcript.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "abcdefgh\n".to_string(),
        });

        let live = display_window(&mut transcript, 6, &options(), 100, usize::MAX, usize::MAX);
        let live_plain = live
            .lines
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(live_plain, vec!["• abcd", "  efgh"]);
        assert!(live
            .lines
            .iter()
            .all(|line| visible_width(line.as_str()) <= 6));

        assert!(transcript.finalize_live_tail());
        let finalized = display_window(&mut transcript, 6, &options(), 100, usize::MAX, usize::MAX);
        let finalized_plain = finalized
            .lines
            .iter()
            .map(|line| strip_ansi_for_test(line.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(finalized_plain, live_plain);
    }

    /// 【终端】【响应式引导测试】验证一至两列终端压缩引导区后不会超宽。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn assistant_body_compacts_guide_on_narrow_terminals() {
        for width in [1usize, 2] {
            let mut transcript = TranscriptStore::new(100);
            transcript.push_chunk(&ChatStreamChunk {
                kind: ChatStreamKind::Content,
                text: "ab\n".to_string(),
            });

            let window = display_window(
                &mut transcript,
                width,
                &options(),
                100,
                usize::MAX,
                usize::MAX,
            );

            assert!(window
                .lines
                .iter()
                .all(|line| visible_width(line.as_str()) <= width));
        }
    }

    /// 验证 TUI diff 比正文再内收一列，并保留对称右边距。
    #[test]
    fn transcript_diff_uses_nested_symmetric_insets() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("sample.txt");
        std::fs::write(&path, "old\n").unwrap();
        let arguments = serde_json::json!({
            "path": path.display().to_string(),
            "old_string": "old",
            "new_string": "new"
        })
        .to_string();
        let mut transcript = TranscriptStore::new(100);
        transcript.push_tool_call("edit_file".to_string(), arguments);

        let window = display_window(
            &mut transcript,
            100,
            &options(),
            100,
            usize::MAX,
            usize::MAX,
        );

        for line in &window.lines {
            assert!(
                line.as_str().starts_with("   ") && !line.as_str().starts_with("    "),
                "diff 行未保持三列内收: {:?}",
                line.as_str()
            );
        }
        assert!(window
            .lines
            .iter()
            .filter(|line| line.as_str().contains("\x1b[48;5;"))
            .all(|line| line.as_str().contains("\x1b[2D\x1b[3X")));
    }

    /// 【终端】【Diff 换行测试】验证 diff 自动换行后的续行仍保留内部缩进。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn wrapped_diff_rows_stay_right_of_visual_guide() {
        let cwd = crate::runtime_cwd::current_dir().unwrap();
        let temp = tempfile::tempdir_in(cwd).unwrap();
        let path = temp.path().join("wrapped-diff.txt");
        let old_line = "old";
        let new_line = "new";
        std::fs::write(&path, format!("{old_line}\n")).unwrap();
        let arguments = serde_json::json!({
            "path": path.display().to_string(),
            "old_string": format!("{old_line}\n"),
            "new_string": format!("{new_line}\n")
        })
        .to_string();
        let mut transcript = TranscriptStore::new(100);
        transcript.push_tool_call("edit_file".to_string(), arguments);

        let window = display_window(&mut transcript, 30, &options(), 100, usize::MAX, usize::MAX);
        assert!(window.lines.len() > 4, "测试数据必须触发 diff 自动换行");
        for line in &window.lines {
            assert!(
                line.as_str().starts_with("   "),
                "diff 续行进入视觉引导区: {:?}",
                strip_ansi_for_test(line.as_str())
            );
            assert!(visible_width(line.as_str()) <= 30, "diff 续行超出终端宽度");
        }
    }

    /// 验证单列终端不增加留白，保留唯一的正文列。
    #[test]
    fn single_column_terminal_keeps_content_column() {
        let mut transcript = TranscriptStore::new(100);
        transcript.push_meta("x".to_string());

        let window = display_window(&mut transcript, 1, &options(), 100, usize::MAX, usize::MAX);

        assert!(window
            .lines
            .iter()
            .all(|line| !line.as_str().starts_with(' ')));
    }
}
