use super::cards;
use super::channel_bridge::{default_channel_target, format_for_channel, ChannelInboundMessage};
use super::model::{
    MemberRole, MemberStatus, MessageKind, ReportStatus, SubscriptionMode, TaskStatus,
    TriggerDisposition, WatchdogSeverity,
};
use super::policy::{evaluate_inbound_route, extract_mentions, RoutingDecision};
use super::service::{SpawnRunnerRequest, SuperAgentService};
use crate::paths::SaiPaths;
use serde_json::json;

/// 构造隔离的测试路径集合。
fn test_paths(root: &std::path::Path) -> SaiPaths {
    SaiPaths::for_tests(root)
}

/// 【超级智能体测试】【群组生命周期】验证群组、Leader、Worker、消息与概览完整闭环。
#[tokio::test]
async fn group_and_member_lifecycle_persists_and_summarizes() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();

    let (group, leader) = service
        .create_group("Alpha Team", "Core delivery squad", "/tmp/demo", Some("Atlas"))
        .unwrap();
    assert_eq!(leader.role, MemberRole::Leader);
    assert_eq!(leader.name, "Atlas");

    let (worker, task) = service
        .spawn_runner(SpawnRunnerRequest {
            group_id: group.id.clone(),
            name: "backend-runner".to_string(),
            specialty: "Rust API".to_string(),
            subagent_type: "general".to_string(),
            task_title: "Implement endpoint".to_string(),
            prompt: "Add health endpoint".to_string(),
            thread_id: None,
            origin_channel: Some("weixin".to_string()),
            origin_target: Some(default_channel_target("wx-user-1", true)),
        })
        .await
        .unwrap();
    assert_eq!(worker.role, MemberRole::Worker);
    assert_eq!(task.assigned_member_id.as_deref(), Some(worker.id.as_str()));

    let detail = service.group_detail(&group.id, 50).unwrap();
    assert_eq!(detail.members.len(), 2);
    assert_eq!(detail.tasks.len(), 1);
    assert_eq!(detail.subscriptions.len(), 1);
    assert_eq!(detail.subscriptions[0].member_id, worker.id);

    let overview = service.overview().unwrap();
    assert_eq!(overview.group_count, 1);
    assert_eq!(overview.total_workers, 1);
}

/// 【超级智能体测试】【两级路由】验证 Fast Path（订阅直通 / @mention / 准入过滤）与 Slow Path（Leader 批处理）。
#[test]
fn two_stage_routing_distinguishes_fast_and_slow_paths() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _leader) = service
        .create_group("Routing", "", "/tmp/r", Some("Leader"))
        .unwrap();
    let worker = service
        .store()
        .create_worker_member(&group.id, "coder", "Rust", "general", "model", "/tmp/r", None)
        .unwrap();
    let members = service.store().list_members(&group.id).unwrap();

    // 1. 未订阅线程默认进入 Leader Slow Path
    let slow = evaluate_inbound_route("请帮我重构登录流程", None, &members, &[], None);
    assert!(matches!(slow, RoutingDecision::SlowPathLeader { .. }));

    // 2. 显式 @coder 直通 Fast Path Worker
    let mention_route = evaluate_inbound_route("@coder 请补充单元测试", None, &members, &[], None);
    assert_eq!(
        mention_route,
        RoutingDecision::FastPathWorkers {
            worker_ids: vec![worker.id.clone()],
            reason: "mentioned_worker_fast_path".to_string(),
        }
    );

    // 3. 建立 Thread owner 订阅后，同线程后续消息自动 Fast Path 扇出，跳过 Leader
    let sub = service
        .subscribe_thread(&group.id, &group.default_thread_id, &worker.id, SubscriptionMode::Owner)
        .unwrap();
    let fanout = evaluate_inbound_route("再检查下边界条件", None, &members, &[sub.clone()], None);
    assert_eq!(
        fanout,
        RoutingDecision::FastPathWorkers {
            worker_ids: vec![worker.id.clone()],
            reason: "subscribed_thread_fan_out".to_string(),
        }
    );

    // 4. 即便存在订阅，显式 @leader 仍回到 Leader Slow Path
    let to_leader = evaluate_inbound_route("@leader 总结一下进度", None, &members, &[sub], None);
    assert!(matches!(to_leader, RoutingDecision::SlowPathLeader { .. }));

    // 5. explicit_only 渠道策略过滤未寻址闲聊消息
    let binding = service
        .bind_channel(&group.id, "feishu", "oc_chat_1", "Team Chat", "explicit_only", true)
        .unwrap();
    let ignored = evaluate_inbound_route("中午吃什么", None, &members, &[], Some(&binding));
    assert!(matches!(ignored, RoutingDecision::Ignore { .. }));
    let addressed = evaluate_inbound_route("@sai 看下 CI 报错", None, &members, &[], Some(&binding));
    assert!(matches!(addressed, RoutingDecision::SlowPathLeader { .. }));
}

/// 【超级智能体测试】【链式召回】验证 Inline & Spill 截断和按需分页召回。
#[tokio::test]
async fn inline_spill_and_chain_of_recall_work_across_entities() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service.create_group("Recall", "", "/tmp", None).unwrap();

    let long_text = "A".repeat(800);
    let (preview, spilled) = cards::inline_or_spill(&long_text, 100);
    assert!(spilled);
    assert!(preview.contains("super_recall"));

    let (msg, _) = service
        .post_user_message(&group.id, "Alice", &long_text, None, None, "web", false)
        .await
        .unwrap();
    let recalled = cards::recall_object(service.store(), &group.id, "message", &msg.id, 0, 250).unwrap();
    assert_eq!(recalled["total_chars"], 800);
    assert_eq!(recalled["next_offset"], 250);
    assert_eq!(recalled["content"].as_str().unwrap().len(), 250);
}

/// 【超级智能体测试】【渠道到交付全链路】验证外部渠道入站 -> 派发 Runner -> Runner 提交报告 -> Leader 主动归纳并递交回原渠道。
#[tokio::test]
async fn channel_inbound_report_and_leader_proactive_delivery_loop() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service
        .create_group("Channel Group", "Weixin bound", "/tmp/ch", Some("Leader"))
        .unwrap();
    service
        .bind_channel(&group.id, "weixin", "wx_owner", "Owner WeChat", "auto", true)
        .unwrap();

    // 1. 渠道入站（模拟出站避免网络调用）
    let (routed_gid, inbound_msg, decision) = service
        .ingest_channel_message(
            ChannelInboundMessage {
                channel: "weixin".to_string(),
                external_chat_id: "wx_owner".to_string(),
                actor: "Alice".to_string(),
                content: "帮我检查登录接口性能瓶颈并给出修复建议".to_string(),
                group_id: None,
                reply_target: json!({ "external_chat_id": "wx_owner", "simulate": true }),
                simulate_outbound: true,
            },
            false,
        )
        .await
        .unwrap();
    assert_eq!(routed_gid, group.id);
    assert_eq!(inbound_msg.kind, MessageKind::ChannelIn);
    assert!(matches!(decision, RoutingDecision::SlowPathLeader { .. }));

    // 2. 创建 Worker 与任务
    let worker = service
        .store()
        .create_worker_member(&group.id, "perf-runner", "Performance", "general", "m", "/tmp/ch", None)
        .unwrap();
    let task = service
        .store()
        .create_task(super::store::NewSuperTask {
            group_id: group.id.clone(),
            thread_id: inbound_msg.thread_id.clone(),
            title: "Audit login latency".to_string(),
            objective: "Profile login endpoint".to_string(),
            assigned_member_id: Some(worker.id.clone()),
            assigned_member_name: Some(worker.name.clone()),
            origin_channel: "weixin".to_string(),
            origin_target: json!({ "external_chat_id": "wx_owner", "simulate": true }),
        })
        .unwrap();

    // 3. Runner 提交结构化成果报告给 Leader
    let report = service
        .submit_runner_report(
            &group.id,
            &inbound_msg.thread_id,
            Some(&task.id),
            &worker.id,
            &worker.name,
            ReportStatus::Ready,
            "发现登录接口每次都重建数据库连接池，改为复用后 P95 降低 68%",
            "详细压测与改动说明...",
            vec!["src/auth/login.rs".to_string()],
            None,
        )
        .await
        .unwrap();
    let updated_task = service.store().get_task(&task.id).unwrap().unwrap();
    assert_eq!(updated_task.status, TaskStatus::WaitingReview);
    assert!(!report.synthesized_by_leader);

    // 4. Leader 归纳并向用户和来源微信渠道递交最终成果
    let (delivery_msg, receipt) = service
        .deliver_leader_result(
            &group.id,
            &inbound_msg.thread_id,
            Some(&task.id),
            &[report.id.clone()],
            "已完成排查：登录接口已改为复用连接池，P95 延迟降低 68%。",
            "## 交付成果\n已完成登录接口性能排查与优化，改动文件 `src/auth/login.rs`。",
            None,
        )
        .await
        .unwrap();
    assert_eq!(delivery_msg.kind, MessageKind::Delivery);
    assert_eq!(receipt.channel, "weixin");
    assert_eq!(receipt.status, "simulated");

    let final_task = service.store().get_task(&task.id).unwrap().unwrap();
    assert_eq!(final_task.status, TaskStatus::Delivered);
    let final_report = service.store().get_report(&report.id).unwrap().unwrap();
    assert!(final_report.synthesized_by_leader);
}

/// 【超级智能体测试】【Watchdog 巡检】验证健康巡检与提及解析。
#[tokio::test]
async fn watchdog_check_and_channel_formatting_work() {
    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    let service = SuperAgentService::open(&paths).unwrap();
    let (group, _) = service.create_group("Ops", "", "/tmp", None).unwrap();

    // 验证 @mention 提取去重且清理标点
    assert_eq!(
        extract_mentions("@coder，请和 @reviewer: 对齐一下 @coder"),
        vec!["coder".to_string(), "reviewer".to_string()]
    );

    // 验证 IM 渠道输出裁剪
    let long_im = "很长的分析内容。".repeat(80);
    let formatted = format_for_channel("weixin", &long_im, 60);
    assert!(formatted.contains("Sai 群组面板"));

    // 验证手动 Watchdog 巡检写入健康记录
    let events = service.run_watchdog_check().await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].severity, WatchdogSeverity::Healthy);

    // 验证 Trigger 去向更新
    let (_, _) = service
        .post_user_message(&group.id, "Bob", "check status", None, None, "tui", false)
        .await
        .unwrap();
    let queued = service.store().list_queued_leader_triggers(&group.id, 10).unwrap();
    assert_eq!(queued.len(), 1);
    service
        .store()
        .update_trigger_disposition(&[queued[0].id.clone()], TriggerDisposition::Evaluated, Some("ok"))
        .unwrap();
    assert!(service
        .store()
        .list_queued_leader_triggers(&group.id, 10)
        .unwrap()
        .is_empty());

    // 验证停止 Runner
    let worker = service
        .store()
        .create_worker_member(&group.id, "w1", "", "general", "m", "/tmp", None)
        .unwrap();
    let stopped = service.stop_runner(&group.id, &worker.id, "done").unwrap();
    assert_eq!(stopped.status, MemberStatus::Stopped);
}

/// 【超级智能体测试】【多渠道网关桥接】验证渠道侧群组命令与自动路由决策。
#[tokio::test]
async fn gateway_bridge_commands_and_routing_work() {
    use super::gateway_bridge::{try_handle_gateway_message, GatewayRouteOutcome};

    let dir = tempfile::tempdir().unwrap();
    let paths = test_paths(dir.path());
    crate::config::AppConfig::init_files(&paths).unwrap();
    let mut cfg = crate::config::AppConfig::load_or_default(&paths).unwrap();
    cfg.super_agent.channel_leader_routing = false;
    cfg.save(&paths).unwrap();

    // 1. 关闭自动路由且未绑定时普通消息透传
    let pass = try_handle_gateway_message(
        &paths,
        "feishu",
        "oc_chat_1",
        "Alice",
        "普通提问",
        json!({ "chat_id": "oc_chat_1", "simulate": true }),
    )
    .await
    .unwrap();
    assert_eq!(pass, GatewayRouteOutcome::PassThrough);

    // 2. 通过渠道命令 /group new 创建并绑定群组
    let created = try_handle_gateway_message(
        &paths,
        "feishu",
        "oc_chat_1",
        "Alice",
        "/group new 飞书研发群",
        json!({ "chat_id": "oc_chat_1" }),
    )
    .await
    .unwrap();
    match created {
        GatewayRouteOutcome::Handled {
            immediate_reply: Some(text),
        } => assert!(text.contains("飞书研发群")),
        other => panic!("unexpected outcome: {other:?}"),
    }

    // 3. 通过 /runners 命令检查当前绑定群组的 Runner 与任务摘要
    let runners = try_handle_gateway_message(
        &paths,
        "feishu",
        "oc_chat_1",
        "Alice",
        "/runners",
        json!({ "chat_id": "oc_chat_1" }),
    )
    .await
    .unwrap();
    match runners {
        GatewayRouteOutcome::Handled {
            immediate_reply: Some(text),
        } => {
            assert!(text.contains("飞书研发群"));
            assert!(text.contains("Runners"));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }

    // 4. /group unbind 解除绑定后恢复透传
    let unbound = try_handle_gateway_message(
        &paths,
        "feishu",
        "oc_chat_1",
        "Alice",
        "/group unbind",
        json!({ "chat_id": "oc_chat_1" }),
    )
    .await
    .unwrap();
    assert!(matches!(
        unbound,
        GatewayRouteOutcome::Handled {
            immediate_reply: Some(_)
        }
    ));
}
