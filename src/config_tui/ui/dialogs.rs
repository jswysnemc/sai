use super::*;

/// 提示弹窗最大宽度：短提示不需要横跨整屏。
const MESSAGE_MAX_WIDTH: u16 = 88;

/// 未保存更改时的退出选择。
pub(crate) enum UnsavedExitChoice {
    Save,
    Discard,
    Cancel,
}

/// 破坏性删除操作的确认弹窗。
///
/// 默认停在「取消」：删除键往往与目标键相邻（如 s/d、a/d），
/// 误触后无法撤销，因此默认选项必须是安全项。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 顶栏标题
/// - `target`: 待删除对象名称
/// - `warning`: 副标题中的后果说明（可为空）
///
/// 返回:
/// - 是否确认删除
pub(crate) fn confirm_delete(
    stdout: &mut io::Stdout,
    title: &str,
    target: &str,
    warning: &str,
) -> Result<bool> {
    let mut selected = 1usize;
    loop {
        let options = [
            format!("{} {target}", t("Delete", "删除")),
            t("Cancel", "取消").to_string(),
        ];
        draw_menu_with_details(
            stdout,
            title,
            &options,
            &[],
            selected,
            &help_line(&[
                ("↑↓", t("move", "移动")),
                ("Enter", t("confirm", "确认")),
                ("Esc", t("cancel", "取消")),
            ]),
            warning,
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => return Ok(selected == 0),
            _ => {}
        }
    }
}

/// 用配置界面的菜单询问如何处理未保存更改。
///
/// 参数:
/// - `stdout`: 终端输出
///
/// 返回:
/// - 保存、放弃或取消
pub(crate) fn confirm_unsaved_exit(stdout: &mut io::Stdout) -> Result<UnsavedExitChoice> {
    let mut selected = 0usize;
    loop {
        let options = [
            t("Save and exit", "保存并退出"),
            t("Discard changes", "放弃更改"),
            t("Cancel", "取消"),
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        draw_menu(
            stdout,
            t(" Unsaved changes ", " 未保存的更改 "),
            &options,
            selected,
            &help_line(&[
                ("↑↓", t("move", "移动")),
                ("Enter", t("confirm", "确认")),
                ("Esc", t("back", "返回")),
            ]),
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(UnsavedExitChoice::Cancel),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => {
                return Ok(match selected {
                    0 => UnsavedExitChoice::Save,
                    1 => UnsavedExitChoice::Discard,
                    _ => UnsavedExitChoice::Cancel,
                });
            }
            _ => {}
        }
    }
}

pub(crate) fn message(stdout: &mut io::Stdout, text: &str) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    // 1. 先按最大宽度折行，再让外框高度贴合正文（上下各留一行）
    let probe = content_frame(cols, rows, 1, MESSAGE_MAX_WIDTH);
    let lines = wrap_text(text, probe.width.saturating_sub(4) as usize);
    let content_rows = lines.len().saturating_add(2).min(usize::from(u16::MAX)) as u16;
    let frame = content_frame(cols, rows, content_rows.max(3), MESSAGE_MAX_WIDTH);
    let page = frame.height.saturating_sub(4) as usize;
    // 内容超出一屏时可滚动：Skills 详情、校验错误这类长文本原先只取头部，
    // 直接断在句子中间且无法看到后面
    let max_offset = lines.len().saturating_sub(page);
    let mut offset = 0usize;
    loop {
        begin_synced_frame(stdout)?;
        queue!(stdout, Clear(ClearType::All))?;
        draw_box(
            stdout,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            t(" Notice ", " 提示 "),
        )?;
        for (row, line) in lines.iter().skip(offset).take(page).enumerate() {
            queue!(
                stdout,
                MoveTo(
                    frame.x.saturating_add(2),
                    frame.y.saturating_add(2).saturating_add(row as u16)
                ),
                Print(line.clone())
            )?;
        }
        let status = if max_offset == 0 {
            help_line(&[(t("any key", "任意键"), t("continue", "继续"))])
        } else {
            // 滚动时把位置一并显示，否则用户不知道下面还有多少
            format!(
                "{}  {}/{}",
                help_line(&[
                    ("↑↓", t("scroll", "滚动")),
                    (t("any key", "任意键"), t("continue", "继续")),
                ]),
                offset.saturating_add(page).min(lines.len()),
                lines.len()
            )
        };
        draw_status_bar(stdout, &frame, &status)?;
        end_synced_frame(stdout)?;
        let key = read_key()?;
        let next = match key {
            KeyCode::Up | KeyCode::Char('k') => offset.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => (offset + 1).min(max_offset),
            KeyCode::PageUp => offset.saturating_sub(page),
            KeyCode::PageDown | KeyCode::Char(' ') => (offset + page).min(max_offset),
            KeyCode::Home => 0,
            KeyCode::End => max_offset,
            _ => return Ok(()),
        };
        // 已经到头/到尾时按方向键视为「任意键」直接关闭，避免空按一下没反应
        if next == offset && max_offset > 0 {
            return Ok(());
        }
        offset = next;
    }
}
