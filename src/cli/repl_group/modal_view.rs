//! 群组工作台居中圆角弹窗：对齐 Sai `PanelPalette` 与 `repl_background` 边框风格，
//! 保留明暗终端可读性并返回激活输入字段的真实硬件光标位置。

use super::model_info::GroupModelSnapshot;
use super::state::{GatewayStatusSummary, GroupModal};
use super::text_util::{
    clip_ansi, pad_ansi, rounded_bottom_border, rounded_top_border, wrap_plain, BOLD, RESET,
};
use crate::cli::repl_text::{char_terminal_width, visible_width};
use crate::render::panel_palette::{bg, fg, PanelPalette};

/// 弹窗单行可编辑输入渲染结果（含文本与相对值区起点的光标列偏移）。
struct ModalFieldRender {
    text: String,
    cursor_col_offset: usize,
}

/// 【群组终端】【圆角居中弹窗】在主画布区域（或窄屏整屏）中央叠加对齐 Sai 主题风格的圆角表单或帮助卡，
/// 宽屏时保留右侧统一侧栏完整可见。
///
/// 参数:
/// - `screen`: 已绘制的整屏行缓冲
/// - `modal`: 当前激活弹窗
/// - `gw`: 本地网关实时状态
/// - `model_info`: 当前模型快照（供帮助卡展示）
/// - `anchor_cols`: 弹窗对齐的目标区域宽度（宽屏为主画布宽度，窄屏为整屏列数）
/// - `rows`: 终端行数
/// - `palette`: 当前明暗调色板
///
/// 返回:
/// - 表单弹窗返回激活字段的真实屏幕光标 `(col, row)`；只读帮助卡返回 `None`
pub(super) fn overlay_modal(
    screen: &mut [String],
    modal: &GroupModal,
    gw: &GatewayStatusSummary,
    model_info: &GroupModelSnapshot,
    anchor_cols: usize,
    rows: usize,
    palette: &PanelPalette,
) -> Option<(u16, u16)> {
    let (box_lines, rel_cursor) = match modal {
        GroupModal::Help => (
            render_help_modal(gw, model_info, anchor_cols, rows, palette),
            None,
        ),
        _ => render_form_modal(modal, gw, anchor_cols, palette),
    };
    let box_h = box_lines.len();
    let start_row = rows.saturating_sub(box_h) / 2;
    let box_w = box_lines
        .first()
        .map(|line| visible_width(line))
        .unwrap_or(0);
    let left_col = anchor_cols.saturating_sub(box_w) / 2;
    for (offset, line) in box_lines.into_iter().enumerate() {
        if let Some(slot) = screen.get_mut(start_row + offset) {
            *slot = crate::render::ansi_overlay::overlay_columns(slot, left_col, box_w, &line);
        }
    }
    rel_cursor.map(|(col, row)| {
        (
            (left_col + col).min(anchor_cols.saturating_sub(1)) as u16,
            (start_row + row).min(rows.saturating_sub(1)) as u16,
        )
    })
}

/// 渲染多字段圆角输入表单弹窗，返回弹窗行数组及相对弹窗左上角的硬件光标坐标。
fn render_form_modal(
    modal: &GroupModal,
    gw: &GatewayStatusSummary,
    cols: usize,
    palette: &PanelPalette,
) -> (Vec<String>, Option<(usize, usize)>) {
    let box_w = cols.saturating_sub(6).clamp(44, 74);
    let inner_w = box_w.saturating_sub(2);
    let (fields, active_field, cursor_chars, footer_note): (Vec<(&str, &str)>, usize, usize, &str) =
        match modal {
            GroupModal::CreateGroup {
                name,
                description,
                field,
                cursor,
            } => (
                vec![("群组名称", name.as_str()), ("群组简介", description.as_str())],
                *field,
                *cursor,
                "新建群组自带独立常驻 Leader，不污染日常项目会话列表",
            ),
            GroupModal::SpawnRunner {
                name,
                specialty,
                prompt,
                field,
                cursor,
            } => (
                vec![
                    ("Runner 名称", name.as_str()),
                    ("角色专长", specialty.as_str()),
                    ("任务指令", prompt.as_str()),
                ],
                *field,
                *cursor,
                "Runner 在独立上下文中异步执行，完成后自动上报 Leader 归纳交付",
            ),
            GroupModal::BindChannel {
                channel,
                chat_id,
                admission,
                field,
                cursor,
            } => (
                vec![
                    ("渠道 (weixin/feishu/qq/onebot)", channel.as_str()),
                    ("外部会话 ID（* 匹配全部）", chat_id.as_str()),
                    ("准入策略 (auto/explicit_only)", admission.as_str()),
                ],
                *field,
                *cursor,
                if gw.weixin_running {
                    "微信网关运行中：绑定后发往微信机器人的消息会自动接入本群 Leader"
                } else if gw.weixin_logged_in {
                    "微信已登录但未启动：确认绑定后输入 /gateway start weixin 即可启动"
                } else {
                    "微信未登录：先在终端运行 sai gateway weixin-login 扫码，再执行 /gateway start weixin"
                },
            ),
            GroupModal::ChannelSim {
                channel,
                chat_id,
                actor,
                content,
                field,
                cursor,
            } => (
                vec![
                    ("模拟渠道 (weixin/feishu/qq)", channel.as_str()),
                    ("模拟会话 ID", chat_id.as_str()),
                    ("模拟发送人", actor.as_str()),
                    ("消息内容", content.as_str()),
                ],
                *field,
                *cursor,
                "仅本地模拟入站与回推回执，不触发外部平台网络发送",
            ),
            GroupModal::Help => return (Vec::new(), None),
        };

    let mut lines = Vec::new();
    let bg_fill = bg(palette.hover_bg);
    lines.push(format!(
        "{bg_fill}{}{RESET}",
        rounded_top_border(modal.title(), box_w, palette)
            .replace(RESET, &format!("{RESET}{bg_fill}"))
    ));
    for wrapped in wrap_plain(footer_note, inner_w.saturating_sub(2)) {
        lines.push(framed_row(
            &format!(" {}{wrapped}", fg(palette.muted)),
            inner_w,
            &bg_fill,
            palette,
        ));
    }
    lines.push(format!(
        "{bg_fill}{}├{}┤{RESET}",
        fg(palette.border),
        "─".repeat(inner_w)
    ));

    let mut active_cursor = None;
    for (idx, (label, value)) in fields.iter().enumerate() {
        let active = idx == active_field;
        let marker = if active { "→" } else { " " };
        let label_part = format!(" {marker} {label}：");
        let label_w = visible_width(&label_part);
        let value_w = inner_w.saturating_sub(label_w + 1).max(8);
        let row_idx = lines.len();
        let row = if active {
            let rendered = render_modal_input(value, cursor_chars, value_w, palette);
            // 左侧边框占 1 列，因此屏幕相对列为 1 + label_w + cursor_col_offset
            active_cursor = Some((1 + label_w + rendered.cursor_col_offset, row_idx));
            format!("{}{label_part}{}", fg(palette.accent), rendered.text)
        } else {
            format!(
                "{}{label_part}{}{}",
                fg(palette.muted),
                fg(palette.text),
                clip_ansi(value, value_w)
            )
        };
        lines.push(framed_row(&row, inner_w, &bg_fill, palette));
    }

    lines.push(format!(
        "{bg_fill}{}├{}┤{RESET}",
        fg(palette.border),
        "─".repeat(inner_w)
    ));
    lines.push(framed_row(
        &format!(
            " {}Tab/↑↓{} 切字段 · {}←→{} 移光标 · {}Enter{} 提交 · {}Esc{} 取消",
            fg(palette.text),
            fg(palette.muted),
            fg(palette.text),
            fg(palette.muted),
            fg(palette.text),
            fg(palette.muted),
            fg(palette.text),
            fg(palette.muted),
        ),
        inner_w,
        &bg_fill,
        palette,
    ));
    lines.push(format!(
        "{bg_fill}{}{RESET}",
        rounded_bottom_border(box_w, palette).replace(RESET, &format!("{RESET}{bg_fill}"))
    ));
    (lines, active_cursor)
}

/// 渲染 F1 / `?` / `/help` 综合帮助卡（同步展示当前 Leader / Worker 模型配置与网关状态）。
fn render_help_modal(
    gw: &GatewayStatusSummary,
    model_info: &GroupModelSnapshot,
    cols: usize,
    rows: usize,
    palette: &PanelPalette,
) -> Vec<String> {
    let box_w = cols.saturating_sub(6).clamp(52, 82);
    let inner_w = box_w.saturating_sub(2);
    let bg_fill = bg(palette.hover_bg);
    let wx_status = if gw.weixin_running {
        "微信网关：运行中"
    } else if gw.weixin_logged_in {
        "微信网关：已登录（输入 /gateway start weixin 启动）"
    } else {
        "微信网关：未登录（先执行 sai gateway weixin-login 扫码）"
    };
    let worktree_label = if model_info.worker_use_worktree {
        "Worktree 隔离"
    } else {
        "共享工作区"
    };
    let (model_line_a, model_line_b) = if model_info.worker_shares_leader {
        (
            format!(
                "Leader / Worker：{}（{}）",
                model_info.leader.full_label(),
                model_info.leader.provider_name,
            ),
            format!(
                "思考 {} · 窗口 {} · 并发上限 {} · {}",
                model_info.leader.thinking,
                crate::cli::repl_chrome::format_token_k(model_info.leader.context_window_tokens),
                model_info.max_concurrent_workers,
                worktree_label,
            ),
        )
    } else {
        (
            format!(
                "Leader：{}（{} · {} · {}）",
                model_info.leader.full_label(),
                model_info.leader.provider_name,
                model_info.leader.thinking,
                crate::cli::repl_chrome::format_token_k(model_info.leader.context_window_tokens),
            ),
            format!(
                "Worker：{}（{} · {}）· 并发 {} · {}",
                model_info.worker.full_label(),
                model_info.worker.provider_name,
                model_info.worker.thinking,
                model_info.max_concurrent_workers,
                worktree_label,
            ),
        )
    };
    let wx_line = format!("当前状态：{wx_status}");

    let entries = [
        ("当前模型与运行配置", true),
        (model_line_a.as_str(), false),
        (model_line_b.as_str(), false),
        ("输入 /model 或点击底栏/侧栏模型名，可直接切换模型与思考等级", false),
        ("快捷键", true),
        ("Enter 发送 · Shift+Enter / Ctrl+J 换行 · ↑↓ 历史或移动行", false),
        ("@ 成员补全 · / 命令补全 · Esc 收起补全 / 清空输入 / 退出", false),
        ("Ctrl+N 新建群组 · Ctrl+R 启 Runner · Ctrl+G 接渠道 · Ctrl+X 停 Runner", false),
        ("Tab 切侧栏页 · Ctrl+↑↓ 切群组 · Alt+↑↓ 切成员 · F2 侧栏 · F5 巡检", false),
        ("微信 / 飞书 / QQ 外部渠道接入", true),
        (wx_line.as_str(), false),
        ("1) 微信扫码：sai gateway weixin-login   2) 启动网关：/gateway start weixin", false),
        ("3) 微信直接对话 Leader（支持 /group bind 与 /runners）", false),
        ("工具权限边界（右侧「工具」页可随时查阅）", true),
        ("Leader：9 个 super_* 编排工具 + read_file/glob/grep 只读感知", false),
        ("Runner：完整代码读写与命令执行 + super_report_submit / super_group_post", false),
    ];

    let max_body = rows.saturating_sub(6).max(8);
    let mut lines = Vec::new();
    lines.push(format!(
        "{bg_fill}{}{RESET}",
        rounded_top_border("群组工作台帮助 · Esc / Enter 关闭", box_w, palette)
            .replace(RESET, &format!("{RESET}{bg_fill}"))
    ));
    let mut body_count = 0usize;
    for (text, is_heading) in entries {
        if body_count >= max_body {
            break;
        }
        if is_heading {
            lines.push(framed_row(
                &format!(" {}{BOLD}{text}", fg(palette.accent)),
                inner_w,
                &bg_fill,
                palette,
            ));
            body_count += 1;
        } else {
            for wrapped in wrap_plain(text, inner_w.saturating_sub(4)) {
                if body_count >= max_body {
                    break;
                }
                lines.push(framed_row(
                    &format!("   {}{wrapped}", fg(palette.text)),
                    inner_w,
                    &bg_fill,
                    palette,
                ));
                body_count += 1;
            }
        }
    }
    lines.push(format!(
        "{bg_fill}{}{RESET}",
        rounded_bottom_border(box_w, palette).replace(RESET, &format!("{RESET}{bg_fill}"))
    ));
    lines
}

/// 按字符光标做水平窗口裁剪，返回弹窗字段渲染文本及真实光标列偏移。
fn render_modal_input(
    text: &str,
    cursor_chars: usize,
    avail: usize,
    palette: &PanelPalette,
) -> ModalFieldRender {
    let chars: Vec<(char, usize)> = text
        .chars()
        .map(|ch| (ch, char_terminal_width(ch).max(1)))
        .collect();
    let cursor_idx = cursor_chars.min(chars.len());
    let width_before: usize = chars[..cursor_idx].iter().map(|(_, w)| *w).sum();
    let cursor_w = chars.get(cursor_idx).map(|(_, w)| *w).unwrap_or(1);
    let max_prefix = avail.saturating_sub(cursor_w);

    let mut start_idx = 0usize;
    if width_before > max_prefix {
        let mut skipped = 0usize;
        let need_skip = width_before - max_prefix;
        while start_idx < cursor_idx && skipped < need_skip {
            skipped += chars[start_idx].1;
            start_idx += 1;
        }
    }

    let mut out = String::new();
    out.push_str(&fg(palette.text));
    let mut used = 0usize;
    let mut cursor_col_offset = 0usize;
    for (idx, (ch, w)) in chars.iter().enumerate().skip(start_idx) {
        if idx == cursor_idx {
            cursor_col_offset = used;
        }
        if used + *w > avail {
            break;
        }
        out.push(*ch);
        used += *w;
    }
    if cursor_idx == chars.len() {
        cursor_col_offset = used.min(avail.saturating_sub(1));
    }
    out.push_str(RESET);
    ModalFieldRender {
        text: out,
        cursor_col_offset,
    }
}

/// 用柔和边框和统一底色包裹弹窗内容行。
fn framed_row(
    content: &str,
    inner_w: usize,
    bg_fill: &str,
    palette: &PanelPalette,
) -> String {
    let fitted = pad_ansi(content, inner_w).replace(RESET, &format!("{RESET}{bg_fill}"));
    format!(
        "{bg_fill}{}│{RESET}{bg_fill}{fitted}{RESET}{bg_fill}{}│{RESET}",
        fg(palette.border),
        fg(palette.border),
    )
}
