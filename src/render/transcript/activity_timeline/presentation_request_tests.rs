use super::super::{cell::HistoryCell, tool_cell::ToolCell};
use super::node;
use crate::render::activity_animation::strip_ansi_for_test as plain;
use crate::render::tool_view::ToolView;
use serde_json::{json, Value};

/// 【终端】【展示回归】通过实际节点入口渲染工具，不绕过执行链标题和正文。
/// 参数: 工具名、参数、结果及展开状态；返回: 完整 ANSI 节点文本。
fn rendered(name: &str, args: Value, result: Value, open: bool) -> String {
    let mut view = ToolView::running(name.into(), args.to_string());
    view.finish(true, result.to_string());
    let cell = HistoryCell::Tool(ToolCell::Invocation(Box::new(view)));
    node::render_cell(&cell, 100, false, open)
        .lines
        .iter()
        .map(|line| line.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 【终端】【展示回归】搜索结果直接呈现命中，省略成功协议字段。
/// 参数: 无；返回: 无。
#[test]
fn requested_search_presentation() {
    let output = plain(&rendered(
        "grep",
        json!({"path":".test_temp","pattern":"delta"}),
        json!({"ok":true,"exit_code":0,"stdout":"./sample.txt:5:line 5: delta appended","stderr":"","truncated":false,"max_results":100}),
        true,
    ));
    assert!(output.contains("1 match"), "{output}");
    assert!(
        output.contains("sample.txt") && output.contains("5: line 5"),
        "{output}"
    );
    assert!(output.contains("line 5: delta appended"), "{output}");
    assert!(
        !output.contains("Success:") && !output.contains("Input:"),
        "{output}"
    );
}

/// 【终端】【展示回归】系统标题提供系统名，展开结果分离环境与系统详情。
/// 参数: 无；返回: 无。
#[test]
fn requested_system_presentation() {
    let result = json!({"ok":true,"platform":"linux","os":"linux","arch":"x86_64", "os_release":"NAME=Arch Linux\nPRETTY_NAME=\"Arch Linux\"", "hostname":"workstation", "username":"developer", "env":{"SHELL":"/bin/zsh","XDG_CURRENT_DESKTOP":"niri","XDG_SESSION_TYPE":"wayland"},"package_manager_guess":["pacman"]});
    let output = plain(&rendered("check_os_info", json!({}), result.clone(), false));
    assert!(
        output.contains("Arch Linux") && output.contains("x86_64"),
        "{output}"
    );
    let output = plain(&rendered("check_os_info", json!({}), result, true));
    assert!(
        output.contains("Session") && output.contains("zsh"),
        "{output}"
    );
    assert!(!output.contains("Success:"), "{output}");
}

/// 【终端】【展示回归】回收站结果突出可恢复性，不罗列布尔协议字段。
/// 参数: 无；返回: 无。
#[test]
fn requested_trash_presentation() {
    let output = plain(&rendered(
        "trash_path",
        json!({"path":".test_temp"}),
        json!({"ok":true,"kind":"directory","original_path":"/sandbox/.test_temp","existed_before":true,"exists_after":false,"trash_item_id":"/Trash/info/.test_temp.trashinfo","restore_hint":"Open system Trash to restore."}),
        true,
    ));
    assert!(
        output.contains("restore") && output.contains("/sandbox/.test_temp"),
        "{output}"
    );
    assert!(
        !output.contains("Exists after:") && !output.contains("Success:"),
        "{output}"
    );
}

/// 【终端】【展示回归】后台清理呈现数量与任务，不展示原始输入输出结构。
/// 参数: 无；返回: 无。
#[test]
fn requested_cleanup_presentation() {
    let output = plain(&rendered(
        "background_command",
        json!({"action":"cleanup","task_id":"task-1","remove_logs":true}),
        json!({"ok":true,"removed":["task-1"]}),
        true,
    ));
    assert!(
        output.contains("1 task") && output.contains("task-1"),
        "{output}"
    );
    assert!(
        !output.contains("Input:") && !output.contains("Success:"),
        "{output}"
    );
}

/// 【终端】【展示回归】删除待办的无后缀动词也能补全真实条目名称。
/// 参数: 无；返回: 无。
#[test]
fn requested_removed_todo_title() {
    let output = plain(&rendered(
        "todo",
        json!({"action":"remove","index":2}),
        json!({"ok":true,"changed":[{"text":"Remove obsolete fixture","status":"pending"}],"items":[]}),
        false,
    ));
    assert!(
        output.contains("Removed") && output.contains("Remove obsolete fixture"),
        "{output}"
    );
}

/// 【终端】【展示回归】展开和收起标题都保留真实增删统计及红绿颜色。
/// 参数: 无；返回: 无。
#[test]
fn requested_edit_count_colors() {
    for name in ["write_file", "str_replace"] {
        for open in [false, true] {
            let output = rendered(
                name,
                json!({"path":"sample.txt","content":"a\nb\nc\nd\n","old_string":"old","new_string":"new"}),
                json!({"ok":true,"changed_files":[{"path":"sample.txt","added":4,"removed":6}]}),
                open,
            );
            let title = output.lines().next().unwrap();
            assert!(
                title.contains("\x1b[32m+4") && title.contains("\x1b[31m-6"),
                "{title:?}"
            );
        }
    }
}

/// 【终端】【全屏补丁测试】真实记录入口转换为补丁节点后仍保留红绿计数。
/// 参数: 无；返回: 无。
#[test]
fn requested_diff_colors_through_transcript_store() {
    use super::super::{store::TranscriptStore, test_support::options};
    use crate::render::transcript::fullscreen_parts::ParagraphKey;
    use std::collections::HashSet;
    crate::render::tui_mode::set_override(Some(crate::config::TuiMode::Fullscreen));
    let mut store = TranscriptStore::new(200);
    store.push_history_tool_call(
        "str_replace".into(),
        json!({"path":"fixture.rs","old_string":"old","new_string":"new"}).to_string(),
    );
    store.push_tool_result("str_replace".into(), true, json!({"ok":true,"diff":"--- a/fixture.rs\n+++ b/fixture.rs\n@@ -1 +1 @@\n-old\n+new\n", "changed_files":[{"path":"fixture.rs","added":1,"removed":1}]}).to_string());
    for expanded in [HashSet::new(), HashSet::from([ParagraphKey::whole(0)])] {
        let document = store.render_fullscreen(100, &options(), &expanded);
        let title = document
            .lines
            .iter()
            .find(|line| line.as_str().contains("+1"))
            .unwrap();
        assert!(
            title.as_str().contains("\x1b[32m+1") && title.as_str().contains("\x1b[31m-1"),
            "{:?}",
            title.as_str()
        );
    }
    crate::render::tui_mode::set_override(None);
}

/// 【终端】【窄屏结果测试】专用视图在窄终端展开时保留正文，并限制每行宽度。
/// 参数: 无；返回: 无。
#[test]
fn requested_cards_fit_narrow_terminal_widths() {
    let mut view = ToolView::running(
        "grep".into(),
        json!({"path":".test_temp","pattern":"delta"}).to_string(),
    );
    view.finish(true, json!({"ok":true,"stdout":"./long-file-name.txt:5:中文 delta appended with a long continuation","truncated":false}).to_string());
    let cell = HistoryCell::Tool(ToolCell::Invocation(Box::new(view)));
    for width in [12, 20, 40, 80] {
        let node = node::render_cell(&cell, width, false, true);
        for line in node.lines {
            assert!(
                crate::render::ansi_clip::ansi_visible_width(line.as_str()) <= width,
                "{}",
                line.as_str()
            );
        }
    }
}

/// 【终端】【精简链路】目标、路径查找、MCP 管理与能力加载经执行链节点展开后不显示内部字段。
/// 参数: 无；返回: 无。
#[test]
fn concise_results_survive_activity_nodes() {
    let goal = json!({"id":"INTERNAL_GOAL","objective":"Complete validation","status":"active","created_at":"INTERNAL_TIME","updates":[{"kind":"progress","message":"Tests passed"}]});
    let cases = [
        ("create_goal", json!({}), goal, "Tests passed"),
        (
            "glob",
            json!({"path":"tool-test","pattern":"**/*e*"}),
            json!({"success":true,"exit_code":0,"stdout":"./fixtures/nested/beta.txt","stderr":"","truncated":false,"max_results":100}),
            "./fixtures/nested/beta.txt",
        ),
        (
            "mcp_manager",
            json!({"action":"test","server_id":"contextX"}),
            json!({"ok":true,"server_id":"contextX","tools_count":2,"tools":["grok_deep_search","grok_search"]}),
            "grok_search",
        ),
        (
            "load",
            json!({"type":"skill","keywords":["agent-browser"]}),
            json!({"ok":true,"router":"jev","skills":[{"name":"agent-browser","status":"loaded","content":"INTERNAL_DOCUMENT"}],"instruction":"INTERNAL_INSTRUCTION"}),
            "agent-browser",
        ),
    ];
    for (name, args, result, expected) in cases {
        let output = plain(&rendered(name, args, result, true));
        assert!(output.contains(expected), "{name}: {output}");
        for internal in [
            "INTERNAL_GOAL",
            "INTERNAL_TIME",
            "INTERNAL_DOCUMENT",
            "INTERNAL_INSTRUCTION",
            "Success:",
            "Exit code:",
            "Max results:",
            "Input:",
            "Router:",
            "Server id:",
        ] {
            assert!(!output.contains(internal), "{name}: {output}");
        }
    }
}
