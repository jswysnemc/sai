use super::{render, ToolView};
use crate::render::{
    activity_animation::strip_ansi_for_test as plain, render_expand, ToolCallDisplayMode,
};
use serde_json::{json, Value};

/// 【终端】【结果测试】构造已完成工具，测试数据不执行真实文件或系统操作。
/// 参数: 工具名、参数、工具状态与输出；返回: 工具视图。
fn view(name: &str, args: Value, ok: bool, output: Value) -> ToolView {
    let mut view = ToolView::running(name.into(), args.to_string());
    view.finish(ok, output.to_string());
    view
}

/// 【终端】【搜索失败测试】空匹配不冒充失败，错误内容不会在摘要模式消失。
/// 参数: 无；返回: 无。
#[test]
fn search_distinguishes_empty_results_and_errors() {
    for (ok, code, expected) in [(true, 1, "no matches"), (false, 2, "Search failed")] {
        let view = view(
            "grep",
            json!({"pattern":"delta"}),
            ok,
            json!({"success":ok,"exit_code":code,"stdout":"","stderr":if ok {""} else {"permission denied"}}),
        );
        let text = plain(&render(&view, ToolCallDisplayMode::Summary));
        assert!(text.contains(expected), "{text}");
        if !ok {
            assert!(
                text.contains("permission denied") && text.contains("Exit code: 2"),
                "{text}"
            );
        }
    }
}

/// 【终端】【结果折叠测试】预览遵循统一折叠边界，展开保留所有命中及截断说明。
/// 参数: 无；返回: 无。
#[test]
fn search_expands_all_matches_without_protocol_noise() {
    let stdout = (1..=40)
        .map(|n| format!("sample.txt:{n}:matched-{n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let view = view(
        "grep",
        json!({"pattern":"matched","path":"fixture"}),
        true,
        json!({"ok":true,"stdout":stdout,"truncated":true}),
    );
    let preview = render_expand::with_force_collapse(|| render(&view, ToolCallDisplayMode::Full));
    assert!(!preview.contains("matched-20"));
    let full = plain(&render_expand::with_force_expand(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    for n in 1..=40 {
        assert!(full.contains(&format!("matched-{n}")), "{n}");
    }
    assert!(full.contains("Results truncated"));
    assert!(full.contains("40+ matches"));
    assert!(render(&view, ToolCallDisplayMode::Hidden).is_empty());
}

/// 【终端】【回收失败测试】原路径仍存在时禁止展示可恢复成功状态。
/// 参数: 无；返回: 无。
#[test]
fn trash_does_not_claim_success_when_path_remains() {
    let view = view(
        "trash_path",
        json!({"path":"fixture"}),
        true,
        json!({"ok":false,"original_path":"/sandbox/fixture","exists_after":true,"note":"access denied"}),
    );
    let text = plain(&render(&view, ToolCallDisplayMode::Summary));
    assert!(
        text.contains("Trash failed") && text.contains("access denied"),
        "{text}"
    );
    assert!(
        !text.contains("recoverable") && !text.contains("Not permanently deleted"),
        "{text}"
    );
}

/// 【终端】【系统平台测试】缺少发行版时兼容 macOS 和 Windows，不输出空环境分组。
/// 参数: 无；返回: 无。
#[test]
fn system_summary_supports_non_linux_platforms() {
    for (data, expected) in [
        (
            json!({"ok":true,"platform":"macos","arch":"aarch64","macos":{"product_name":"macOS","product_version":"15.0"}}),
            "macOS 15.0",
        ),
        (
            json!({"ok":true,"platform":"windows","arch":"x86_64","env":{}}),
            "windows",
        ),
    ] {
        let view = view("check_os_info", json!({}), true, data);
        let text = plain(&render_expand::with_force_expand(|| {
            render(&view, ToolCallDisplayMode::Full)
        }));
        assert!(text.contains(expected), "{text}");
        assert!(!text.contains("Session"), "{text}");
    }
}

/// 【终端】【清理空结果测试】零条记录使用明确空状态，不声称删除日志。
/// 参数: 无；返回: 无。
#[test]
fn cleanup_empty_result_keeps_accurate_status() {
    let view = view(
        "background_command",
        json!({"action":"cleanup","remove_logs":true}),
        true,
        json!({"ok":true,"removed":[],"remaining":2}),
    );
    let text = plain(&render(&view, ToolCallDisplayMode::Full));
    assert!(
        text.contains("Cleared 0 tasks") && text.contains("No finished tasks"),
        "{text}"
    );
    assert!(!text.contains("Logs:"), "{text}");
}
