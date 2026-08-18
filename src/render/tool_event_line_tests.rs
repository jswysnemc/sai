use super::*;
use crate::render::terminal_text as t;

/// 验证命令全文保留多行内容和较长参数。
#[test]
fn command_full_text_keeps_multiline_and_long_command() {
    assert_eq!(
        tool_command_full_text(
            "run_command",
            Some(r#"{"command":"cargo test\ncargo build"}"#)
        )
        .as_deref(),
        Some("cargo test\ncargo build")
    );
    let long = "a".repeat(80);
    let args = format!(r#"{{"command":"{long}"}}"#);
    assert_eq!(
        tool_command_full_text("run_command", Some(&args)).as_deref(),
        Some(long.as_str())
    );
    let title = tool_command_title_colored("run_command", Some(r#"{"command":"echo hello"}"#));
    assert!(title.contains("Running"));
    assert!(title.contains("echo") || title.contains("\x1b["));
}

/// 验证前台与后台命令使用进行时标签。
#[test]
fn command_tools_use_run_label() {
    assert_eq!(
        tool_event_label("run_command", Some(r#"{"command":"date"}"#)),
        "Running date"
    );
    assert_eq!(
        tool_event_label("run_command", Some(r#"{"command":"cargo bui"#)),
        "Running cargo bui"
    );
    assert_eq!(
        tool_event_label(
            "run_command",
            Some(r#"{"command":"cargo test\ncargo build"}"#)
        ),
        "Running cargo test"
    );
    assert_eq!(
        tool_event_label(
            "background_command",
            Some(r#"{"action":"start","command":"sleep 1"}"#)
        ),
        format!("{} sleep 1", t("Background start", "启动后台命令"))
    );
    assert_eq!(
        tool_event_label("background_command", Some(r#"{"action":"list"}"#)),
        t("Background list", "后台命令列表")
    );
}

/// 完成后工具卡动词切到过去式。
#[test]
fn tool_labels_use_perfect_tense_when_done() {
    assert_eq!(
        tool_event_label_tense(
            "write_file",
            Some(r#"{"path":"notes.md","content":"x"}"#),
            ToolVerbTense::Perfect
        ),
        "Wrote notes.md"
    );
    assert_eq!(
        tool_event_label_tense(
            "run_command",
            Some(r#"{"command":"date"}"#),
            ToolVerbTense::Perfect
        ),
        "Ran date"
    );
    assert_eq!(
        tool_event_label_tense(
            "str_replace",
            Some(r#"{"path":"a.rs","old_string":"a","new_string":"b"}"#),
            ToolVerbTense::Perfect
        ),
        "Replaced a.rs"
    );
    assert_eq!(
        retarget_label_tense("write_file", "Writing notes.md", ToolVerbTense::Perfect),
        "Wrote notes.md"
    );
}

/// 验证文件工具标签包含目标文件名。
#[test]
fn file_tools_include_basename() {
    assert_eq!(
        tool_event_label(
            "edit_file",
            Some(
                r#"{"patch":"*** Begin Patch\n*** Update File: src/render/stream.rs\n@@\n-a\n+b\n*** End Patch"}"#
            )
        ),
        "Editing stream.rs"
    );
    assert_eq!(
        tool_event_label(
            "read_file",
            Some(r#"{"files":[{"path":"src/a.rs"},{"path":"src/b.rs"}]}"#)
        ),
        "Reading a.rs b.rs"
    );
    assert_eq!(
        tool_event_label(
            "read_file",
            Some(r#"{"path":"src/render/stream.rs","offset":12,"limit":80}"#)
        ),
        "Reading stream.rs:12+80"
    );
    assert_eq!(
        tool_event_label("read_file", Some(r#"{"path":"src/a.rs","limit":40}"#)),
        "Reading a.rs:1+40"
    );
}

/// 验证未闭合参数仍可提取工具目标。
#[test]
fn partial_arguments_extract_target() {
    assert_eq!(
        tool_event_label(
            "edit_file",
            Some(r#"{"patch":"*** Begin Patch\n*** Update File: src/main.rs\n@@\n-unfinished"#)
        ),
        "Editing main.rs"
    );
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_search"#)),
        "Loading tool web_search"
    );
    assert_eq!(
        tool_event_label(
            "write_file",
            Some(r#"{"path":"src/main.rs","content":"fn "#)
        ),
        "Writing main.rs"
    );
}

/// 验证渐进加载工具使用资源类型和名称标签。
#[test]
fn load_uses_load_label() {
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_search"]}"#)),
        "Loading tool web_search"
    );
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"tool","keywords":["web_fetch"]}"#)),
        "Loading tool web_fetch"
    );
    assert_eq!(
        tool_event_label("load", Some(r#"{"type":"skill","keywords":["yce"]}"#)),
        "Loading skill yce"
    );
}

/// 验证子智能体工具按 action 选择动词，并展示描述或操作目标。
#[test]
fn subagent_uses_description_label() {
    assert_eq!(
        tool_event_label("subagent", Some(r#"{"description":"scan code"}"#)),
        "Delegating scan code"
    );
    // 只有 start 是委派，其余 action 各用各的动词，否则 wait / status 会和委派混作一谈
    assert_eq!(
        tool_event_label(
            "subagent",
            Some(r#"{"action":"status","subagent_id":"subagent_1"}"#)
        ),
        "Checking subagent_1"
    );
    assert_eq!(
        tool_event_label(
            "subagent",
            Some(r#"{"action":"wait","subagent_id":"subagent_1"}"#)
        ),
        "Awaiting subagent_1"
    );
    assert_eq!(
        tool_event_label("subagent", Some(r#"{"action":"list"}"#)),
        "Listing"
    );
}

/// 验证管理类工具标签包含操作和目标。
#[test]
fn management_tools_include_action_and_target() {
    assert_eq!(
        tool_event_label("todo", Some(r#"{"action":"add","text":"检查测试"}"#)),
        "Updating add 检查测试"
    );
    assert_eq!(
        tool_event_label("cron", Some(r#"{"action":"remove","id":"cron_1"}"#)),
        "Scheduling remove cron_1"
    );
}

/// 验证事件文本不再附加冗余工具前缀。
#[test]
fn event_text_omits_tool_prefix() {
    let output = tool_event_text("Wrote main.rs", "ok");
    let plain = crate::render::activity_animation::strip_ansi_for_test(&output);

    assert!(plain.starts_with("• Wrote main.rs "));
    assert!(!plain.contains("tool:"));
}

/// 统一标题排版：状态色圆点 + 粗体动词 + 常规对象 + 语义色徽标。
#[test]
fn event_text_uses_unified_title_hierarchy() {
    // 成功：绿色圆点 + 绿色 ok
    let ok = tool_event_text("Wrote main.rs", "ok");
    assert!(ok.starts_with("\x1b[1m\x1b[32m•\x1b[0m "));
    assert!(ok.contains("\x1b[1mWrote\x1b[0m main.rs"));
    assert!(ok.ends_with("\x1b[32mok\x1b[0m"));

    // 失败：红色圆点 + 红色 err
    let err = tool_event_text("Read a.rs", "err");
    assert!(err.starts_with("\x1b[1m\x1b[31m•\x1b[0m "));
    assert!(err.ends_with("\x1b[31merr\x1b[0m"));

    // 进行中：弱化圆点 + 黄色 run
    let run = tool_event_text("Reading a.rs", "run");
    assert!(run.starts_with("\x1b[2m•\x1b[0m "));
    assert!(run.contains("\x1b[33mrun\x1b[0m"));

    // 自定义徽标（编辑类 +N -M）：语义由调用方显式给出
    let stat = tool_status_line(
        "Wrote a.rs",
        "\x1b[32m+2\x1b[0m \x1b[31m-0\x1b[0m",
        ToolHealth::Ok,
    );
    assert!(stat.starts_with("\x1b[1m\x1b[32m•\x1b[0m "));
    assert!(stat.contains("+2"));

    // 空徽标：无行尾状态
    let bare = tool_status_line("Writing a.rs", "", ToolHealth::Pending);
    assert!(bare.starts_with("\x1b[2m•\x1b[0m "));
    assert!(!bare.ends_with(' '));
}

/// 验证未知工具使用通用工具标签。
#[test]
fn unknown_tools_use_tool_label() {
    let label = tool_event_label("custom_tool", Some(r#"{"value":1}"#));
    let output = tool_event_text(&label, "err");
    let plain = crate::render::activity_animation::strip_ansi_for_test(&output);

    assert_eq!(label, "Running custom_tool");
    assert!(plain.contains("Running custom_tool"));
    assert!(output.contains("\x1b[31merr\x1b[0m"));
    assert_eq!(
        tool_event_label_tense("custom_tool", None, ToolVerbTense::Perfect),
        "Ran custom_tool"
    );
}
