use super::*;
use crate::render::{render_expand, ToolCallDisplayMode};
use serde_json::json;

/// 【终端显示】【工具卡演示】创建并打印指定工具视图的展示效果。
///
/// 参数:
/// - `category`: 工具分类名称
/// - `view`: 待展示的工具卡模型
/// - `mode`: 展示模式（Full / Summary）
///
/// 返回:
/// - 无返回值，直接向标准输出输出渲染结果
fn print_demo_card(category: &str, view: &ToolView, mode: ToolCallDisplayMode) {
    println!("\x1b[1;35m══════════════════════════════════════════════════════════════\x1b[0m");
    println!("\x1b[1;33m[DEMO] {category} (mode: {mode:?})\x1b[0m");
    println!("\x1b[1;35m──────────────────────────────────────────────────────────────\x1b[0m");
    let rendered = render_expand::with_force_expand(|| render_framed(view, mode, 0));
    println!("{rendered}");
    println!();
}

/// 【终端显示】【工具卡展示】执行全量工具卡 TUI 渲染展示。
#[test]
fn test_all_tool_cards_demo() {
    // 1. 前台命令工具 (run_command) - 成功与失败
    let mut cmd_ok = ToolView::running(
        "run_command".into(),
        json!({"command": "cargo test --bin sai"}).to_string(),
    );
    cmd_ok.finish(
        true,
        json!({
            "mode": "foreground",
            "completed": true,
            "success": true,
            "exit_code": 0,
            "stdout": "running 12 tests\ntest result: ok. 12 passed; 0 failed; finished in 0.42s",
            "stderr": ""
        })
        .to_string(),
    );
    print_demo_card("1. 前台命令 (run_command · 成功)", &cmd_ok, ToolCallDisplayMode::Full);

    // 2. 命令超时提升为后台 (run_command -> background promotion)
    let mut cmd_promoted = ToolView::running(
        "run_command".into(),
        json!({"command": "pnpm build"}).to_string(),
    );
    cmd_promoted.finish(
        true,
        json!({
            "mode": "background",
            "ok": true,
            "promoted": true,
            "task_id": "1793294858-2109875",
            "waited_seconds": 15,
            "task": {
                "id": "1793294858-2109875",
                "command": "pnpm build",
                "status": "running"
            }
        })
        .to_string(),
    );
    print_demo_card("2. 命令提升后台 (run_command promotion)", &cmd_promoted, ToolCallDisplayMode::Full);

    // 3. 后台任务运行与日志输出 (background_command · running)
    let mut bg_cmd = ToolView::running(
        "background_command".into(),
        json!({"action": "start", "command": "docker compose up"}).to_string(),
    );
    bg_cmd.finish(
        true,
        json!({
            "ok": true,
            "task": {
                "id": "bg-task-8891",
                "command": "docker compose up",
                "status": "running"
            },
            "stdout": "[+] Running 3/3\nNetwork app-net Created\nContainer app-db Started",
            "stderr": ""
        })
        .to_string(),
    );
    print_demo_card("3. 后台任务运行 (background_command)", &bg_cmd, ToolCallDisplayMode::Full);

    // 4. 后台任务列表查询 (background_command · list)
    let mut bg_list = ToolView::running(
        "background_command".into(),
        json!({"action": "list"}).to_string(),
    );
    bg_list.finish(
        true,
        json!({
            "ok": true,
            "tasks": [
                {"id": "bg-101", "status": "running", "command": "cargo watch -x check"},
                {"id": "bg-102", "status": "completed", "command": "pytest -v"},
                {"id": "bg-103", "status": "stopped", "command": "npm run dev"}
            ]
        })
        .to_string(),
    );
    print_demo_card("4. 后台任务列表 (background_command · list)", &bg_list, ToolCallDisplayMode::Full);

    // 5. 后台任务清理 (background_command · cleanup)
    let mut bg_cleanup = ToolView::running(
        "background_command".into(),
        json!({"action": "cleanup", "remove_logs": true}).to_string(),
    );
    bg_cleanup.finish(
        true,
        json!({
            "ok": true,
            "removed": ["bg-102", "bg-103"],
            "remaining": 1
        })
        .to_string(),
    );
    print_demo_card("5. 后台清理 (background_command · cleanup)", &bg_cleanup, ToolCallDisplayMode::Full);

    // 6. 代码编辑 Diff (str_replace / write_file)
    let mut edit_view = ToolView::running(
        "str_replace".into(),
        json!({
            "path": "Cargo.toml",
            "old_string": "edition = \"2021\"",
            "new_string": "edition = \"2024\""
        })
        .to_string(),
    );
    edit_view.finish(
        true,
        json!({
            "ok": true,
            "changed_files": [{"path": "Cargo.toml", "added": 1, "removed": 1}]
        })
        .to_string(),
    );
    print_demo_card("6. 代码编辑 Diff (str_replace)", &edit_view, ToolCallDisplayMode::Full);

    // 7. 文件分页阅读 (read_file · 文本/代码)
    let mut read_view = ToolView::running(
        "read_file".into(),
        json!({"path": "src/main.rs"}).to_string(),
    );
    read_view.finish(
        true,
        "1\tfn main() {\n2\t    println!(\"Hello sai!\");\n3\t    run_agent();\n4\t}".to_string(),
    );
    print_demo_card("7. 代码阅读 (read_file · 带行号高亮)", &read_view, ToolCallDisplayMode::Full);

    // 8. 读图卡片 (read_file · 图片识别)
    let mut read_img = ToolView::running(
        "read_file".into(),
        json!({"path": "pics/sai-architecture.svg"}).to_string(),
    );
    read_img.finish(
        true,
        "[Image: source: pics/sai-architecture.svg, image/svg+xml]".to_string(),
    );
    print_demo_card("8. 读图识别 (read_file · 图片模式)", &read_img, ToolCallDisplayMode::Full);

    // 9. 交互式提问 (ask_question)
    let mut ask_view = ToolView::running(
        "ask_question".into(),
        json!({
            "questions": [{
                "header": "选择目标运行时",
                "question": "请选择当前工程使用的应用运行时架构：",
                "options": [
                    {"label": "Node.js (LTS)", "description": "标准稳定环境"},
                    {"label": "Bun (Fast)", "description": "快速原生运行时"},
                    {"label": "Rust Native", "description": "高性能编译二进制"}
                ]
            }]
        })
        .to_string(),
    );
    ask_view.finish(
        true,
        json!({
            "answers": [{
                "header": "选择目标运行时",
                "answer": "Rust Native"
            }]
        })
        .to_string(),
    );
    print_demo_card("9. 交互式提问 (ask_question)", &ask_view, ToolCallDisplayMode::Full);

    // 10. 待办事项清单 (todo)
    let mut todo_view = ToolView::running(
        "todo".into(),
        json!({"action": "update", "id": "2", "status": "in_progress"}).to_string(),
    );
    todo_view.finish(
        true,
        json!({
            "ok": true,
            "changed": [{"text": "实现核心解析引擎", "status": "in_progress"}],
            "items": [
                {"text": "初始化项目脚手架", "status": "completed"},
                {"text": "实现核心解析引擎", "status": "in_progress"},
                {"text": "编写回归测试套件", "status": "pending"},
                {"text": "废弃的原型模块", "status": "cancelled"}
            ]
        })
        .to_string(),
    );
    print_demo_card("10. 待办清单 (todo · Full 模式)", &todo_view, ToolCallDisplayMode::Full);
    print_demo_card("10. 待办清单 (todo · Summary 模式)", &todo_view, ToolCallDisplayMode::Summary);

    // 11. 文件内容检索 (grep / search_text)
    let mut grep_view = ToolView::running(
        "grep".into(),
        json!({"path": "src/", "pattern": "fn render_framed", "include": "*.rs"}).to_string(),
    );
    grep_view.finish(
        true,
        json!({
            "ok": true,
            "exit_code": 0,
            "stdout": "src/render/tool_view/formatter.rs:29:pub(crate) fn render_framed(view: &ToolView, mode: ToolCallDisplayMode, frame: usize) -> String {\nsrc/render/transcript/tool_cell.rs:38:        ToolCell::Invocation(view) => tool_view::render_framed(view, mode, frame),",
            "truncated": false
        })
        .to_string(),
    );
    print_demo_card("11. 内容检索 (grep)", &grep_view, ToolCallDisplayMode::Full);

    // 12. 路径通配匹配 (glob / find_files)
    let mut glob_view = ToolView::running(
        "glob".into(),
        json!({"path": "src/render", "pattern": "*status*.rs"}).to_string(),
    );
    glob_view.finish(
        true,
        json!({
            "ok": true,
            "exit_code": 0,
            "stdout": "src/render/status_style.rs\nsrc/render/work_status.rs\nsrc/render/stream_tool_status.rs\nsrc/render/live_tool_status.rs",
            "truncated": false
        })
        .to_string(),
    );
    print_demo_card("12. 路径匹配 (glob)", &glob_view, ToolCallDisplayMode::Full);

    // 13. 子智能体管理 (subagent)
    let mut sub_view = ToolView::running(
        "subagent".into(),
        json!({"action": "wait", "subagent_id": "subagent-arch-eval"}).to_string(),
    );
    sub_view.finish(
        true,
        json!({
            "ok": true,
            "subagent": {
                "id": "subagent-arch-eval",
                "description": "评估分布式缓存架构设计",
                "status": "completed",
                "result": "架构评审完毕：推荐采用 Redis 哨兵集群架构，读写吞吐可提升 4.2 倍。"
            }
        })
        .to_string(),
    );
    print_demo_card("13. 子智能体 (subagent)", &sub_view, ToolCallDisplayMode::Full);

    // 14. 网页浏览器 (browser)
    let mut browser_view = ToolView::running(
        "browser".into(),
        json!({"action": "screenshot", "ref": "div#dashboard"}).to_string(),
    );
    browser_view.finish(
        true,
        "[Image: source: browser screenshot of https://console.cloud.io/dashboard, image/png] 1280x800".to_string(),
    );
    print_demo_card("14. 浏览器控制 (browser)", &browser_view, ToolCallDisplayMode::Full);

    // 15. 桌面无障碍与自动化 (computer_use)
    let mut cu_view = ToolView::running(
        "computer_use".into(),
        json!({
            "action": "click",
            "app": "Code Editor",
            "window_id": 412,
            "target": "Button[name='Run Tests']"
        })
        .to_string(),
    );
    cu_view.finish(
        true,
        json!({"status": "clicked", "focused": true}).to_string(),
    );
    print_demo_card("15. 桌面自动化 (computer_use)", &cu_view, ToolCallDisplayMode::Full);

    // 16. 目标管理 (create_goal / update_goal)
    let mut goal_view = ToolView::running(
        "update_goal".into(),
        json!({"status": "complete", "note": "全部测试用例验证完毕"}).to_string(),
    );
    goal_view.finish(
        true,
        json!({
            "goal": {
                "id": "goal-deploy-v2",
                "objective": "完成生产环境平滑升级与回滚预案制定",
                "status": "complete",
                "token_budget": 50000,
                "tokens_used": 12840,
                "updates": [
                    {"kind": "progress", "message": "全量集成测试回归完成"},
                    {"kind": "progress", "message": "全部测试用例验证完毕"}
                ]
            }
        })
        .to_string(),
    );
    print_demo_card("16. 目标管理 (update_goal)", &goal_view, ToolCallDisplayMode::Full);

    // 17. 系统信息检查 (check_os_info)
    let mut os_view = ToolView::running(
        "check_os_info".into(),
        "{}".to_string(),
    );
    os_view.finish(
        true,
        json!({
            "ok": true,
            "os_release": "PRETTY_NAME=\"Arch Linux\"\nNAME=\"Arch Linux\"",
            "arch": "x86_64",
            "hostname": "workstation-arch",
            "username": "snemc",
            "package_manager_guess": ["pacman", "paru"],
            "env": {
                "SHELL": "/bin/zsh",
                "TERM": "xterm-kitty",
                "XDG_CURRENT_DESKTOP": "niri",
                "XDG_SESSION_TYPE": "wayland",
                "LANG": "zh_CN.UTF-8"
            }
        })
        .to_string(),
    );
    print_demo_card("17. 系统信息 (check_os_info)", &os_view, ToolCallDisplayMode::Full);

    // 18. 回收站管理 (trash_path)
    let mut trash_view = ToolView::running(
        "trash_path".into(),
        json!({"path": "tmp/deprecated_module.rs"}).to_string(),
    );
    trash_view.finish(
        true,
        json!({
            "ok": true,
            "original_path": "tmp/deprecated_module.rs",
            "kind": "file",
            "exists_after": false,
            "trash_item_id": "trash_rec_9921",
            "restore_hint": "gio trash --restore 或使用文件管理器还原"
        })
        .to_string(),
    );
    print_demo_card("18. 回收站 (trash_path)", &trash_view, ToolCallDisplayMode::Full);

    // 19. MCP 服务管理 (mcp_manager)
    let mut mcp_view = ToolView::running(
        "mcp_manager".into(),
        json!({"action": "list_tools", "server_id": "github-tools"}).to_string(),
    );
    mcp_view.finish(
        true,
        json!({
            "ok": true,
            "server_id": "github-tools",
            "tools": ["create_pull_request", "merge_pull_request", "inspect_ci_status"]
        })
        .to_string(),
    );
    print_demo_card("19. MCP 服务管理 (mcp_manager)", &mcp_view, ToolCallDisplayMode::Full);

    // 20. 动态能力路由 (request_capability)
    let mut cap_view = ToolView::running(
        "request_capability".into(),
        json!({"need": "操作桌面无障碍接口并截屏"}).to_string(),
    );
    cap_view.finish(
        true,
        json!({
            "ok": true,
            "router": "jev",
            "tools": [{"name": "computer_use"}],
            "skills": [{"name": "niri-use"}]
        })
        .to_string(),
    );
    print_demo_card("20. 动态能力路由 (request_capability)", &cap_view, ToolCallDisplayMode::Full);

    // 21. 图像生成 (generate_image)
    let mut img_gen = ToolView::running(
        "generate_image".into(),
        json!({"prompt": "赛博朋克风格终端操作员", "aspect_ratio": "16:9"}).to_string(),
    );
    img_gen.finish(
        true,
        json!({
            "type": "image_generation",
            "path": "/home/snemc/Pictures/sai/generated_cyberpunk.png"
        })
        .to_string(),
    );
    print_demo_card("21. 图像生成 (generate_image)", &img_gen, ToolCallDisplayMode::Full);

    // 22. 网页搜索 (web_search)
    let mut search_view = ToolView::running(
        "web_search".into(),
        json!({"query": "Rust 2024 edition new features"}).to_string(),
    );
    search_view.finish(
        true,
        "## Search results for: Rust 2024 edition\n**Provider**: DuckDuckGo\n\n### 1. The Rust 2024 Edition Guide\n**URL**: https://doc.rust-lang.org/edition-guide/rust-2024/\n**Snippet**: Summary of changes in Rust 2024 edition including RPITIT and async closures.\n\n### 2. Rust Blog: Announcing Rust 2024\n**URL**: https://blog.rust-lang.org/2024/release-notes.html\n**Snippet**: Official announcement covering stabilization timelines and migration tooling.".to_string(),
    );
    print_demo_card("22. 网页搜索 (web_search)", &search_view, ToolCallDisplayMode::Full);

    // 23. 上下文压缩 (compress_context)
    let mut comp_view = ToolView::running("compress_context".into(), "{}".to_string());
    comp_view.finish(
        true,
        json!({"before_tokens": 48200, "after_tokens": 1250}).to_string(),
    );
    print_demo_card("23. 上下文压缩 (compress_context)", &comp_view, ToolCallDisplayMode::Full);

    // 24. 会话排队与续轮通知 (conversation_notice)
    let mut conv_view = ToolView::running(
        "conversation_notice".into(),
        json!({"kind": "goal_continuation"}).to_string(),
    );
    conv_view.finish(
        true,
        "Goal 自动续轮：继续执行第二阶段基准性能压测".to_string(),
    );
    print_demo_card("24. 会话通知 (conversation_notice)", &conv_view, ToolCallDisplayMode::Full);

    // 25. 外部后台通知 (background_notice)
    let mut bg_notice = ToolView::running(
        "background_notice".into(),
        "".to_string(),
    );
    bg_notice.finish(
        true,
        "<external-completion-events>\n后台命令: rtk cargo check --tests (1793294858-2109875)\n状态: exited\n说明: 耗时 29.61s，构建成功\n</external-completion-events>".to_string(),
    );
    print_demo_card("25. 外部后台通知 (background_notice)", &bg_notice, ToolCallDisplayMode::Full);

    // 26. 通用扩展与权限审批回退卡片 (Fallback Tool)
    let mut custom_tool = ToolView::running(
        "custom_database_migrator".into(),
        json!({"target_schema": "v3.2", "dry_run": false}).to_string(),
    );
    custom_tool.request_permission("perm_req_7721".into());
    print_demo_card("26. 通用回退与权限审批 (Fallback + Permission)", &custom_tool, ToolCallDisplayMode::Full);
}
