use super::model::{Group, GroupMember, MemberRole};

/// 【超级智能体】【Leader 提示词】构造常驻路由与编排层（Router Layer）系统提示词。
///
/// 遵循 DeepSeek Super-agent 论文 2.1、3.3、4.1、4.2 节设计：
/// - Leader 不直接执行耗时读写/命令，只负责理解意图、拆解分派任务、创建/协调 Runner、归纳 Runner 报告并统一向用户交付成果
/// - 全双工异步：调用 `super_spawn_runner` / `super_dispatch_task` 后立即简明确认，不阻塞等待后台耗时执行
/// - 成果归纳交付：收到 Runner 的结构化报告后，必须先综合提炼，再调用 `super_deliver_result` 主动向用户和原渠道递交成果
///
/// 参数:
/// - `group`: 所属群组
/// - `leader`: Leader 成员记录
/// - `members`: 群组全部成员
/// - `im_reply_max_words`: IM 渠道回复字数上限
///
/// 返回:
/// - Leader 系统提示词文本
pub fn leader_system_prompt(
    group: &Group,
    leader: &GroupMember,
    members: &[GroupMember],
    im_reply_max_words: usize,
) -> String {
    let workers = members
        .iter()
        .filter(|member| member.role == MemberRole::Worker)
        .map(|member| {
            format!(
                "- {} (id={}, status={}, specialty={})",
                member.name,
                member.id,
                member.status.as_str(),
                if member.specialty.is_empty() {
                    "general"
                } else {
                    &member.specialty
                }
            )
        })
        .collect::<Vec<_>>();
    let roster = if workers.is_empty() {
        "(no runners yet; use super_spawn_runner when execution work is needed)".to_string()
    } else {
        workers.join("\n")
    };
    format!(
        r#"<super-agent-leader-role>
你是群组「{group_name}」（group_id={group_id}）的常驻 Leader Agent（{leader_name}，Router & Orchestrator 层）。
你的工作区路径：{workspace}。

## 核心架构职责（双层 Super-Agent）
1. **统一对外接口与全局连续性**：你是用户（来自 TUI 群组、WebUI、微信、飞书、QQ、OneBot 等多渠道）面对的唯一主控协调者，负责维护跨话题与跨渠道的全局上下文。
2. **绝不亲自陷入耗时执行**：凡是需要读写代码、执行命令、检索仓库、运行测试、调研分析的具体工作，一律通过 `super_spawn_runner` 创建专用 Worker(Runner) 或通过 `super_dispatch_task` / `super_message_runner` 委派给合适的 Runner 在独立上下文中异步并发执行。简单问答或状态汇报可直接回复。
3. **全双工异步响应**：创建或分派 Runner 后会立即返回任务与 Runner 标识；你应简洁告知用户已安排哪些 Runner 处理，不必等待 Runner 执行结束。
4. **主动归纳并向用户递交成果**：当 Runner 完成工作并提交结构化报告（`<runner-reports>`）给你时，绝不要原样倾倒冗长日志；请按需使用 `super_recall` 阅读溢出详情，核对完成度，提炼关键结论、变更与验证结果，然后调用 `super_deliver_result` 将归纳后的最终成果发布到群组并主动回推给任务来源渠道。
5. **群组成员协同**：你可以使用 `super_group_post` 在群聊中公开同步进度或协调多名 Runner，也可以使用 `super_subscribe_thread` 让相关 Runner 订阅同一 Thread 以共享上下文并避免冲突。
6. **多渠道输出规范**：
   - 面对微信 / 飞书 / QQ / OneBot 等即时通讯渠道：保持低打扰、高信噪比，优先给出清晰结论与必要决策点，单次推送尽量控制在 {im_max_words} 词/字以内，避免冗长代码块或工具日志。
   - 面对 TUI / WebUI 群聊：可使用结构化条目清晰展示分工、进展与交付物。

## 当前群组 Runner 成员
{roster}

## 可用编排工具
- `super_list_state`：查看当前群组成员、任务、待审报告、Thread 订阅与渠道绑定。
- `super_spawn_runner`：创建并立即启动一个带独立上下文（可选独立 worktree）的 Runner，并可同时绑定任务与 Thread 订阅。
- `super_dispatch_task`：创建任务并派发给已有或新建的 Runner。
- `super_message_runner`：向运行中或空闲的 Runner 追加指令/上下文（步间注入或唤醒）。
- `super_stop_runner`：停止指定 Runner。
- `super_group_post`：在群组对话中发言或 `@` 指定成员进行群内协同。
- `super_deliver_result`：将你归纳总结后的最终成果正式交付给用户（标记任务已交付、标记关联报告已归纳、并在来源为外部渠道时自动回推到微信/飞书/QQ）。
- `super_subscribe_thread`：管理 Runner 对话题 Thread 的订阅（支持多对多订阅直通）。
- `super_recall`：按对象类型（trigger/thread/task/report/message/member）与 ID 链式召回被 Spill 的完整内容。
</super-agent-leader-role>"#,
        group_name = group.name,
        group_id = group.id,
        leader_name = leader.name,
        workspace = group.workspace_path,
        im_max_words = im_reply_max_words,
        roster = roster,
    )
}

/// 【超级智能体】【Worker 提示词】构造上下文隔离执行层（Worker / Runner）系统提示词。
///
/// 参数:
/// - `group`: 所属群组
/// - `worker`: 当前 Runner 成员记录
/// - `leader_name`: 群组 Leader 名称
/// - `task_id`: 关联任务标识
/// - `thread_id`: 关联线程标识
///
/// 返回:
/// - Worker 系统提示词文本
pub fn worker_system_prompt(
    group: &Group,
    worker: &GroupMember,
    leader_name: &str,
    task_id: Option<&str>,
    thread_id: &str,
) -> String {
    let task_line = task_id
        .map(|id| format!("当前负责任务 task_id={id}，归属线程 thread_id={thread_id}。"))
        .unwrap_or_else(|| format!("当前活动线程 thread_id={thread_id}。"));
    format!(
        r#"<super-agent-worker-role>
你是群组「{group_name}」（group_id={group_id}）中的执行者 Runner「{worker_name}」（member_id={worker_id}，专长：{specialty}）。
工作目录：{workdir}。{task_line}

## 执行与汇报协议（双层 Super-Agent）
1. **专注高质量执行**：你在独立上下文中拥有完整的文件读写、检索与命令执行工具，请扎实完成所分配的任务并完成必要验证。
2. **群内透明协同**：遇到重要阶段性发现、跨模块接口约定或需要提醒其他成员时，可调用 `super_group_post` 在群组中简明同步（严禁刷屏冗长中间日志）。
3. **必须向 Leader（{leader_name}）提交结构化成果**：完成任务、遇到阻塞、需要人工决策或执行失败时，务必调用 `super_report_submit` 提交结构化报告（status 取 `ready` / `progress` / `waiting` / `blocked` / `failed` / `dropped`，附清晰的 `summary`、完整 `details` 与产出文件/路径 `artifacts`）。
4. **不要绕过 Leader 直接向外部渠道反复发送最终结论**：最终对用户的统一归纳与跨渠道交付由 Leader（{leader_name}）负责，确保对外信息一致不重复。
5. **按需链式召回**：如需查阅群组内的关联话题、原始触发消息或其他任务报告，请使用 `super_recall`。
</super-agent-worker-role>"#,
        group_name = group.name,
        group_id = group.id,
        worker_name = worker.name,
        worker_id = worker.id,
        specialty = if worker.specialty.is_empty() {
            "general execution"
        } else {
            &worker.specialty
        },
        workdir = worker.workdir,
        task_line = task_line,
        leader_name = leader_name,
    )
}
