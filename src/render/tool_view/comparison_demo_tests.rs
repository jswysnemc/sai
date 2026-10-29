use super::*;
use crate::render::{render_expand, ToolCallDisplayMode};
use serde_json::json;

/// 【终端显示】【设计对比】输出新旧工具卡视觉与排版对比块。
///
/// 参数:
/// - `index`: 对比项目序号
/// - `title`: 对比功能名称
/// - `dimension`: 核心优化维度简述
/// - `before`: 优化前（Current / Before）渲染文本
/// - `after`: 优化后（Proposed / After）渲染文本
///
/// 返回:
/// - 无返回值，直接向终端输出 ANSI 格式对比
fn print_comparison(
    index: usize,
    title: &str,
    dimension: &str,
    before: &str,
    after: &str,
) {
    println!("\x1b[1;36m┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓\x1b[0m");
    println!("\x1b[1;36m┃ 对比 {index}. {title} \x1b[0m");
    println!("\x1b[2;37m┃ 优化维度: {dimension}\x1b[0m");
    println!("\x1b[1;36m┣━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┫\x1b[0m");
    println!("\x1b[1;31m[优化前 / 当前版本现状]\x1b[0m");
    println!("{before}");
    println!("\x1b[1;32m[优化后 / 建议设计方案]\x1b[0m");
    println!("{after}");
    println!("\x1b[1;36m┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛\x1b[0m\n");
}

#[test]
fn test_comparison_demo() {
    // 1. 【系统/渲染】【对比演示】前台命令卡片 (run_command)
    // 现状：无耗时指示，固定截取前5行，折叠时关键结果丢失
    let mut cmd_view = ToolView::running(
        "run_command".into(),
        json!({"command": "cargo test --bin sai"}).to_string(),
    );
    cmd_view.finish(
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
    let before_cmd = render_expand::with_force_expand(|| render_framed(&cmd_view, ToolCallDisplayMode::Full, 0));
    let after_cmd = format!(
        "\x1b[1m\x1b[32m\x1b[0m  \x1b[1mRan\x1b[0m \x1b[35m$\x1b[0m \x1b[38;2;156;207;216mcargo\x1b[0m \x1b[38;5;189mtest\x1b[0m --\x1b[38;5;189mbin\x1b[0m \x1b[38;5;189msai\x1b[0m \x1b[2m[0.42s]\x1b[0m\n\x1b[2m  └ running 12 tests\n    test result: ok. 12 passed; 0 failed; finished in 0.42s\x1b[0m"
    );
    print_comparison(
        1,
        "前台命令卡 (run_command)",
        "补充耗时度量 [0.42s] 与尾部总结保障",
        &before_cmd,
        &after_cmd,
    );

    // 2. 【系统/渲染】【对比演示】后台命令与提升卡片 (background_command)
    // 现状：20位长哈希 ID 造成视觉噪音，`/ps` 语义晦涩
    let mut bg_view = ToolView::running(
        "background_command".into(),
        json!({"action": "start", "command": "pnpm build"}).to_string(),
    );
    bg_view.finish(
        true,
        json!({
            "ok": true,
            "task": {
                "id": "1793294858-2109875",
                "command": "pnpm build",
                "status": "running"
            },
            "stdout": "",
            "stderr": ""
        })
        .to_string(),
    );
    let before_bg = render_expand::with_force_expand(|| render_framed(&bg_view, ToolCallDisplayMode::Full, 0));
    let after_bg = format!(
        "\x1b[2m\x1b[0m  \x1b[1mRunning in background\x1b[0m \x1b[35m$\x1b[0m \x1b[38;2;156;207;216mpnpm\x1b[0m \x1b[38;5;189mbuild\x1b[0m \x1b[2m#210987\x1b[0m\n\x1b[2m  └ 进程正常运行中 · 输入 \x1b[36m/ps\x1b[2m 查看日志输出\x1b[0m"
    );
    print_comparison(
        2,
        "后台任务管理 (background_command)",
        "长哈希精简为短标识 #210987，提供明确的人类交互引导",
        &before_bg,
        &after_bg,
    );

    // 3. 【系统/渲染】【对比演示】代码编辑 Diff 卡片 (str_replace)
    // 现状：无所属函数上下文，无编辑器行号跳转链接
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
    let before_edit = render_expand::with_force_expand(|| render_framed(&edit_view, ToolCallDisplayMode::Full, 0));
    let after_edit = format!(
        "\x1b[1m\x1b[32m\x1b[0m  \x1b[1mReplaced\x1b[0m \x1b[36mCargo.toml:4:1\x1b[0m \x1b[32m+1\x1b[0m \x1b[31m-1\x1b[0m\n  \x1b[2m@@ [package] @@\x1b[0m\n  \x1b[38;5;244m  3    \x1b[0mversion = \"0.2.12\"\n  \x1b[48;5;52m\x1b[38;5;250m  4 \x1b[38;5;174m-  \x1b[39medition = \x1b[48;5;88m\"2021\"\x1b[0m\n  \x1b[48;5;22m\x1b[38;5;250m  4 \x1b[38;5;108m+  \x1b[39medition = \x1b[48;5;28m\"2024\"\x1b[0m\n  \x1b[38;5;244m  5    \x1b[0mlicense = \"MIT\""
    );
    print_comparison(
        3,
        "代码编辑与 Git Diff (str_replace)",
        "补充路径行号跳转 (Cargo.toml:4:1) 与 Hunk 作用域 (@@ [package] @@)",
        &before_edit,
        &after_edit,
    );

    // 4. 【系统/渲染】【对比演示】交互式提问卡片 (ask_question)
    // 现状：标题与题干双重重复，历史中依然展开全量未选选项
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
    let before_ask = render_expand::with_force_expand(|| render_framed(&ask_view, ToolCallDisplayMode::Full, 0));
    let after_ask = format!(
        "\x1b[1m\x1b[32m\x1b[0m  \x1b[1mAsked\x1b[0m 选择目标运行时 \x1b[2m→\x1b[0m \x1b[1;36mRust Native\x1b[0m \x1b[2m(单选已确认)\x1b[0m"
    );
    print_comparison(
        4,
        "交互式提问决策 (ask_question)",
        "完结后自动收敛为决策单行，剔除冗余题干与干扰性未选选项",
        &before_ask,
        &after_ask,
    );

    // 5. 【系统/渲染】【对比演示】待办清单卡片 (todo)
    // 现状：符号杂乱无序（→, ○, ✓, –, 删除线），纯文字统计缺乏视觉进度
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
    let before_todo = render_expand::with_force_expand(|| render_framed(&todo_view, ToolCallDisplayMode::Full, 0));
    let after_todo = format!(
        "\x1b[1m\x1b[32m\x1b[0m  \x1b[1mTodo\x1b[0m \x1b[36m[■■□□] 50%\x1b[0m \x1b[2m· 1 进行中 / 1 待办 / 1 已完成\x1b[0m\n  \x1b[36m● \x1b[1m实现核心解析引擎\x1b[0m\n  \x1b[2m○ 编写回归测试套件\x1b[0m\n  \x1b[32m✓\x1b[0m \x1b[2m初始化项目脚手架\x1b[0m\n  \x1b[2m✕ \x1b[9m废弃的原型模块\x1b[0m"
    );
    print_comparison(
        5,
        "待办清单卡片 (todo)",
        "引入紧凑字符进度条 [■■□□] 与标准化状态指示符",
        &before_todo,
        &after_todo,
    );

    // 6. 【系统/渲染】【对比演示】代码内容检索卡片 (grep)
    // 现状：长路径可能折行破损，正文中未对用户输入的 pattern 施加视觉高亮
    let mut grep_view = ToolView::running(
        "grep".into(),
        json!({"path": "src/", "pattern": "render_framed", "include": "*.rs"}).to_string(),
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
    let before_grep = render_expand::with_force_expand(|| render_framed(&grep_view, ToolCallDisplayMode::Full, 0));
    let after_grep = format!(
        "\x1b[1m\x1b[32m\x1b[0m  \x1b[1mSearched\x1b[0m \x1b[36m*.rs\x1b[0m in \x1b[2msrc/\x1b[0m · 2 matches\n  \x1b[36msrc/.../formatter.rs:29\x1b[0m\n    pub(crate) fn \x1b[1;30;43mrender_framed\x1b[0m(view: &ToolView, mode: ToolCallDisplayMode, frame: usize) -> String {{\n  \x1b[36msrc/.../tool_cell.rs:38\x1b[0m\n    ToolCell::Invocation(view) => tool_view::\x1b[1;30;43mrender_framed\x1b[0m(view, mode, frame),"
    );
    print_comparison(
        6,
        "代码检索卡片 (grep)",
        "长路径中段智能折叠，并在匹配文本中施加黄色高亮关键词",
        &before_grep,
        &after_grep,
    );

    // 7. 【系统/渲染】【对比演示】子智能体卡片 (subagent)
    // 现状：只有结果正文，黑盒运行缺乏调用步数、耗时与 Token 等可审计凭据
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
    let before_sub = render_expand::with_force_expand(|| render_framed(&sub_view, ToolCallDisplayMode::Full, 0));
    let after_sub = format!(
        "\x1b[1m\x1b[32m\x1b[0m  \x1b[1mSubagent\x1b[0m 评估分布式缓存架构设计 \x1b[2m· 4 步调用 · 1.2k tokens · 4.8s\x1b[0m\n  \x1b[2m└\x1b[0m 架构评审完毕：推荐采用 Redis 哨兵集群架构，读写吞吐可提升 4.2 倍。"
    );
    print_comparison(
        7,
        "子智能体卡片 (subagent)",
        "增强运行透明度，透出调用步数、Token 消耗与耗时凭据",
        &before_sub,
        &after_sub,
    );

    // 8. 【系统/渲染】【对比演示】回收站卡片 (trash_path)
    // 现状：恢复指令混排在自然语言提示中，无法一键双击复制
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
    let before_trash = render_expand::with_force_expand(|| render_framed(&trash_view, ToolCallDisplayMode::Full, 0));
    let after_trash = format!(
        "\x1b[1m\x1b[32m\x1b[0m  \x1b[1mTrashed\x1b[0m deprecated_module.rs \x1b[2m· 可随时安全恢复\x1b[0m\n  \x1b[2m原路径: tmp/deprecated_module.rs\x1b[0m\n  \x1b[35m$ \x1b[38;2;156;207;216mgio\x1b[0m \x1b[38;5;189mtrash\x1b[0m --\x1b[38;5;189mrestore\x1b[0m \x1b[38;2;166;214;160mtmp/deprecated_module.rs\x1b[0m \x1b[2m(双击复制执行)\x1b[0m"
    );
    print_comparison(
        8,
        "安全回收站 (trash_path)",
        "分离自然语言与 Shell 恢复代码，支持双击快速复制还原",
        &before_trash,
        &after_trash,
    );

    // 9. 【系统/渲染】【对比演示】通用权限审批卡片 (Permission)
    // 现状：选项裸露无容器，视觉拦截感弱，易在滚屏中被忽视
    let mut perm_view = ToolView::running(
        "custom_database_migrator".into(),
        json!({"target_schema": "v3.2"}).to_string(),
    );
    perm_view.request_permission("perm_req_7721".into());
    let before_perm = render_expand::with_force_expand(|| render_framed(&perm_view, ToolCallDisplayMode::Full, 0));
    let after_perm = format!(
        "\x1b[1;33m╔════ 权限审批请求 · custom_database_migrator ═════════════════╗\x1b[0m\n\x1b[1;33m║\x1b[0m 参数: {{\"target_schema\": \"v3.2\"}}\n\x1b[1;33m║\x1b[0m   \x1b[1;36m[y] 允许本次 (Allow once)\x1b[0m\n\x1b[1;33m║\x1b[0m   \x1b[2m[n] 拒绝执行 (Deny)\x1b[0m\n\x1b[1;33m║\x1b[0m   \x1b[2m[a] 始终信任 (Allow all)\x1b[0m\n\x1b[1;33m║\x1b[0m \x1b[2m快捷键: y 批准 · n 拒绝 · Shift+Tab 全局放行\x1b[0m\n\x1b[1;33m╚══════════════════════════════════════════════════════════════╝\x1b[0m"
    );
    print_comparison(
        9,
        "通用权限审批 (Permission Request)",
        "增加模态聚焦边框容器与键盘快捷键标记，避免重要授权被滚屏淹没",
        &before_perm,
        &after_perm,
    );
}
