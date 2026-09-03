use crate::render::status_style::{tool_bullet, ToolHealth};
use crate::render::terminal_text as t;
use serde_json::Value;

/// 前台命令被提升为后台任务的结果视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BackgroundPromotion {
    /// 后台任务短 ID
    pub task_id: String,
    /// 是否因等待超时提升（false 表示直接以后台模式启动）
    pub promoted: bool,
    /// 已等待秒数
    pub waited_seconds: u64,
    /// 等待期间产生的部分 stdout
    pub partial_stdout: String,
}

/// 解析 run_command 结果里的后台提升信息。
///
/// 前台命令等待超时会返回 `mode=background` 的 JSON；此前渲染层把它
/// 当成普通输出，部分场景落入 err 视图。这里识别提升语义，交由
/// 专用渲染表达「命令转入后台」，而不是报错。
///
/// 参数:
/// - `output`: run_command 工具输出
///
/// 返回:
/// - 后台提升视图；不是 background 结果时返回空
pub(crate) fn parse_background_promotion(output: &str) -> Option<BackgroundPromotion> {
    let value = serde_json::from_str::<Value>(output.trim()).ok()?;
    if value.get("mode")?.as_str()? != "background" {
        return None;
    }
    Some(BackgroundPromotion {
        task_id: value
            .get("task_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        promoted: value
            .get("promoted")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        waited_seconds: value
            .get("waited_seconds")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        partial_stdout: value
            .get("partial_stdout")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

/// 渲染后台提升的专用事件行。
///
/// 提升不是失败：圆点用进行中色，徽标给任务 ID 与等待秒数，
/// 正文提示后续用 background_command 管理该任务。
///
/// 参数:
/// - `promotion`: 后台提升视图
/// - `command`: 命令文本，用于事件行对象
///
/// 返回:
/// - ANSI 事件行文本
pub(crate) fn render_promotion_line(promotion: &BackgroundPromotion, command: &str) -> String {
    let verb = if promotion.promoted {
        t("Backgrounded", "已转入后台")
    } else {
        t("Started background", "已启动后台任务")
    };
    // 命令正文用与命令工具一致的 bash 语法着色，未命中高亮规则时保持原样
    let object = if command.trim().is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            crate::render::code_block::highlight_code_line("bash", command)
        )
    };
    let waited = if promotion.promoted && promotion.waited_seconds > 0 {
        format!(
            " · {} {}{}",
            promotion.waited_seconds,
            t("s", "秒"),
            t(" waited", "等待")
        )
    } else {
        String::new()
    };
    let badge = format!(
        "\x1b[36m{}{waited}\x1b[0m",
        short_task_id(&promotion.task_id)
    );
    let line = format!(
        "{} \x1b[1m{verb}\x1b[0m{object} {badge}",
        tool_bullet(ToolHealth::Pending)
    );
    format!(
        "{line}\n\x1b[2m\x1b[36m  └ {}\x1b[0m",
        t(
            "manage with background_command (list/output/wait/stop)",
            "用 background_command 管理（list/output/wait/stop）"
        )
    )
}

/// 缩短任务 ID 供单行展示。
///
/// 参数:
/// - `task_id`: 完整任务 ID
///
/// 返回:
/// - 截断后的任务 ID
fn short_task_id(task_id: &str) -> String {
    crate::render::clip_to_width(task_id, 18, "...")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test;

    /// 解析提升结果并区分直接后台启动。
    #[test]
    fn parses_promotion_result() {
        let output = r#"{"mode":"background","ok":true,"promoted":true,"waited_seconds":30,"task_id":"task-abc123","partial_stdout":"hello\n"}"#;
        let promotion = parse_background_promotion(output).unwrap();
        assert_eq!(promotion.task_id, "task-abc123");
        assert!(promotion.promoted);
        assert_eq!(promotion.waited_seconds, 30);

        assert!(parse_background_promotion(r#"{"mode":"foreground"}"#).is_none());
        assert!(parse_background_promotion("not json").is_none());
    }

    /// 提升行展示转入后台语义与任务 ID，不出现 err。
    #[test]
    fn promotion_line_uses_pending_semantics() {
        let promotion = BackgroundPromotion {
            task_id: "task-abc123".to_string(),
            promoted: true,
            waited_seconds: 30,
            partial_stdout: String::new(),
        };
        let plain = strip_ansi_for_test(&render_promotion_line(&promotion, "cargo test"));
        assert!(plain.contains("cargo test"), "{plain}");
        assert!(plain.contains("task-abc123"), "{plain}");
        assert!(!plain.contains("err"), "{plain}");
    }
}
