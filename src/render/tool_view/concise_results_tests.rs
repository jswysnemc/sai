use super::{render, ToolView};
use crate::render::{
    activity_animation::strip_ansi_for_test as plain, render_expand, ToolCallDisplayMode,
};
use serde_json::{json, Value};

/// 【终端】【精简回归】通过完整工具渲染入口获取展开文本，不执行真实工具。
/// 参数: 工具名、参数及结果；返回: 去除 ANSI 的结果视图。
fn expanded(name: &str, args: Value, output: Value) -> String {
    let mut view = ToolView::running(name.into(), args.to_string());
    view.finish(true, output.to_string());
    plain(&render_expand::with_force_expand(|| {
        render(&view, ToolCallDisplayMode::Full)
    }))
}

/// 【终端】【目标精简】保留目标、状态和最近进度，不显示标识、时间戳和更新历史。
/// 参数: 无；返回: 无。
#[test]
fn concise_goal_results() {
    let goal = json!({"id":"INTERNAL_GOAL","objective":"Complete validation","status":"active","tokens_used":500,"time_used_seconds":90,"token_budget":null,"created_at":"INTERNAL_TIME","updated_at":"INTERNAL_TIME","updates":[{"kind":"progress","message":"OLD_PROGRESS"},{"kind":"progress","message":"Tests passed"}]});
    for name in ["create_goal", "get_goal", "update_goal"] {
        let output = if name == "get_goal" {
            json!({"goal":goal.clone()})
        } else {
            goal.clone()
        };
        let text = expanded(name, json!({}), output);
        assert!(
            text.lines().next().unwrap().contains("Complete validation"),
            "{text}"
        );
        assert!(text.contains("Tests passed"), "{text}");
        for internal in [
            "INTERNAL_GOAL",
            "INTERNAL_TIME",
            "OLD_PROGRESS",
            "Tokens used:",
            "Time used",
        ] {
            assert!(!text.contains(internal), "{text}");
        }
    }
}

/// 【终端】【路径精简】文件查找只展示范围、数量和路径。
/// 参数: 无；返回: 无。
#[test]
fn concise_glob_results() {
    let text = expanded(
        "glob",
        json!({"path":"tool-test","pattern":"**/*e*"}),
        json!({"success":true,"exit_code":0,"stdout":"./fixtures/nested/beta.txt","stderr":"","truncated":false,"max_results":100}),
    );
    assert!(
        text.contains("1 file") && text.contains("./fixtures/nested/beta.txt"),
        "{text}"
    );
    for internal in [
        "Success:",
        "Exit code:",
        "Error output:",
        "Max results:",
        "Input:",
    ] {
        assert!(!text.contains(internal), "{text}");
    }
}

/// 【终端】【管理精简】MCP 测试展示服务、连接结果和工具名单。
/// 参数: 无；返回: 无。
#[test]
fn concise_mcp_results() {
    let text = expanded(
        "mcp_manager",
        json!({"action":"test","server_id":"contextX"}),
        json!({"ok":true,"server_id":"contextX","tools_count":2,"tools":["grok_deep_search","grok_search"]}),
    );
    assert!(text.lines().next().unwrap().contains("contextX"), "{text}");
    assert!(
        text.contains("2 tools") && text.contains("grok_search"),
        "{text}"
    );
    assert!(
        !text.contains("Success:") && !text.contains("Server id:") && !text.contains("Action:"),
        "{text}"
    );
}

/// 【终端】【加载精简】参数定义与能力文档不进入用户视图。
/// 参数: 无；返回: 无。
#[test]
fn concise_loading_results() {
    for name in ["load", "request_capability"] {
        let text = expanded(
            name,
            json!({"need":"Draw a flowchart","type":"skill","keywords":["agent-browser"]}),
            json!({"ok":true,"router":"jev","tools":[{"name":"browser","definition":"INTERNAL_SCHEMA"}],"skills":[{"name":"agent-browser","status":"loaded","content":"INTERNAL_DOCUMENT"}],"instruction":"INTERNAL_INSTRUCTION"}),
        );
        assert!(text.contains("agent-browser"), "{text}");
        for internal in [
            "INTERNAL_SCHEMA",
            "INTERNAL_DOCUMENT",
            "INTERNAL_INSTRUCTION",
            "Success:",
            "Router:",
        ] {
            assert!(!text.contains(internal), "{text}");
        }
    }
}

/// 【终端】【子任务精简】子任务首行包含描述，展开显示结论而不是运行期快照。
/// 参数: 无；返回: 无。
#[test]
fn concise_subagent_results() {
    let text = expanded(
        "subagent",
        json!({}),
        json!({"ok":true,"subagent":{"id":"INTERNAL_ID","description":"Read-only search test","status":"completed","max_steps":0,"started_at":123,"updated_at":456,"step":2,"result":"**Conclusion**: two files found."}}),
    );
    assert!(
        text.lines()
            .next()
            .unwrap()
            .contains("Read-only search test"),
        "{text}"
    );
    assert!(text.contains("two files found"), "{text}");
    for internal in [
        "INTERNAL_ID",
        "Max steps:",
        "Started at:",
        "Updated at:",
        "Success:",
    ] {
        assert!(!text.contains(internal), "{text}");
    }
}

/// 【终端】【提问标题】问题主题进入首行，收起后仍可识别具体问题。
/// 参数: 无；返回: 无。
#[test]
fn concise_question_title() {
    let text = expanded(
        "ask_question",
        json!({"questions":[{"header":"Change scope","question":"Which modules should change?","options":[]}]}),
        json!({"answers":[{"header":"Change scope","question":"Which modules should change?","answer":"UI only"}]}),
    );
    assert!(
        text.lines().next().unwrap().contains("Change scope"),
        "{text}"
    );
}

/// 【终端】【网页精简】展开网页搜索显示直达地址与正文摘要，不输出重定向包装。
/// 参数: 无；返回: 无。
#[test]
fn concise_web_results() {
    let mut view = ToolView::running(
        "web_search".into(),
        json!({"query":"pnpm catalogs"}).to_string(),
    );
    view.finish(true, "## Search results for: pnpm catalogs\n**Provider**: DuckDuckGo\n\n### 1. Catalogs - pnpm\n**URL**: //duckduckgo.com/l/?uddg=https%3A%2F%2Fpnpm.io%2Fcatalogs&rut=TRACKING\n**Snippet**: Share dependency versions.".into());
    let text = plain(&render_expand::with_force_expand(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert!(
        text.contains("https://pnpm.io/catalogs") && text.contains("Share dependency versions"),
        "{text}"
    );
    for internal in [
        "TRACKING",
        "**URL**",
        "## Search results",
        "args:",
        "output:",
    ] {
        assert!(!text.contains(internal), "{text}");
    }
}

/// 【终端】【截图精简】截图仅保留对象、尺寸和页面地址，隐藏模型坐标说明。
/// 参数: 无；返回: 无。
#[test]
fn concise_browser_screenshot() {
    let mut view = ToolView::running(
        "browser".into(),
        json!({"action":"screenshot","ref":"e3"}).to_string(),
    );
    view.finish(true, "[Image: source: browser screenshot (element button [ref=e3]) of http://127.0.0.1:8321/, image/jpeg, 903 bytes, 43x24]\nCoordinates in this image are page CSS pixels when not resized; use browser with action=snapshot refs to interact.".into());
    let text = plain(&render_expand::with_force_expand(|| {
        render(&view, ToolCallDisplayMode::Full)
    }));
    assert!(text.contains("43x24") && text.contains("e3"), "{text}");
    for internal in ["Coordinates", "[Image:", "image/jpeg", "args:"] {
        assert!(!text.contains(internal), "{text}");
    }
}
