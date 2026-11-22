use super::model::{
    ChannelBinding, GroupMember, MemberRole, SubscriptionMode, ThreadSubscription,
    TriggerDisposition,
};

/// 【超级智能体】【两级路由决策】对应论文 3.1/3.2 节：Fast Path 确定性过滤与订阅直通，Slow Path 交由 Leader 批处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// 确定性策略判定忽略（不消耗模型推理）
    Ignore { reason: String },
    /// Fast Path：直接投递给指定的一个或多个活跃 Worker(Runner)，不经 Leader 转发
    FastPathWorkers {
        worker_ids: Vec<String>,
        reason: String,
    },
    /// Slow Path：进入常驻 Leader 批处理队列进行语义研判与编排
    SlowPathLeader { reason: String },
}

impl RoutingDecision {
    /// 返回路由路径标识文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - `fast_path` / `slow_path` / `filtered`
    pub fn route_path(&self) -> &'static str {
        match self {
            Self::Ignore { .. } => "filtered",
            Self::FastPathWorkers { .. } => "fast_path",
            Self::SlowPathLeader { .. } => "slow_path",
        }
    }

    /// 返回初始 Trigger 去向状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 初始去向枚举
    pub fn initial_disposition(&self) -> TriggerDisposition {
        match self {
            Self::Ignore { .. } => TriggerDisposition::Ignored,
            Self::FastPathWorkers { .. } => TriggerDisposition::FannedOut,
            Self::SlowPathLeader { .. } => TriggerDisposition::Queued,
        }
    }
}

/// 从消息文本中提取开头或正文中的 `@目标` 名称列表。
///
/// 参数:
/// - `text`: 用户或成员消息正文
///
/// 返回:
/// - 去重后的被提及目标名称（不含 `@`）
pub fn extract_mentions(text: &str) -> Vec<String> {
    let mut mentions = Vec::new();
    for token in text.split_whitespace() {
        let Some(rest) = token.strip_prefix('@') else {
            continue;
        };
        // 1. 截断到首个中英文标点，避免 `@coder，请处理` 把逗号与后续正文吞进名称
        let cleaned = rest
            .split(|ch: char| {
                matches!(
                    ch,
                    ',' | '.' | ':' | ';' | '!' | '?' | '，' | '。' | '：' | '；' | '！' | '？' | '(' | ')' | '（' | '）'
                )
            })
            .next()
            .unwrap_or("")
            .trim();
        if !cleaned.is_empty()
            && !mentions
                .iter()
                .any(|item: &String| item.eq_ignore_ascii_case(cleaned))
        {
            mentions.push(cleaned.to_string());
        }
    }
    mentions
}

/// 对一条新进入群组的用户/渠道事件执行两级过滤与路由决策。
///
/// 规则顺序（忠实于 DeepSeek 论文 3.1 & 3.2 节）：
/// 1. 程序层准入检查：若渠道绑定为 `ignore` 则直接忽略；若为 `explicit_only` 且未显式寻址则忽略
/// 2. 显式 `@member` 寻址：若用户显式 `@leader` 走 Slow Path；若显式 `@runner` 且目标存活走 Fast Path 直通
/// 3. 显式指定目标成员 ID 时按角色直接路由
/// 4. Thread 订阅直通（Fast Path）：若所在 Thread 已有活跃 `owner` 订阅的 Worker，直接扇出到所有订阅 Worker，跳过 Leader 转发开销
/// 5. 其余新话题或未订阅事件进入 Leader Slow Path 批处理
///
/// 参数:
/// - `content`: 消息正文
/// - `explicit_recipient_id`: 调用方显式指定的目标成员 ID
/// - `members`: 群组当前成员列表
/// - `thread_subscriptions`: 当前 Thread 的订阅列表
/// - `channel_binding`: 若来自外部渠道，传入命中的渠道绑定策略
///
/// 返回:
/// - 路由决策
pub fn evaluate_inbound_route(
    content: &str,
    explicit_recipient_id: Option<&str>,
    members: &[GroupMember],
    thread_subscriptions: &[ThreadSubscription],
    channel_binding: Option<&ChannelBinding>,
) -> RoutingDecision {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return RoutingDecision::Ignore {
            reason: "empty_message".to_string(),
        };
    }

    // 1. 程序层确定性准入过滤（Fast Path Gate）
    if let Some(binding) = channel_binding {
        if !binding.enabled || binding.admission_mode == "ignore" {
            return RoutingDecision::Ignore {
                reason: format!("channel_policy_{}", binding.admission_mode),
            };
        }
        if binding.admission_mode == "explicit_only"
            && explicit_recipient_id.is_none()
            && !contains_explicit_addressing(trimmed, members)
        {
            return RoutingDecision::Ignore {
                reason: "explicit_only_not_addressed".to_string(),
            };
        }
    }

    // 2. 显式目标成员 ID 优先
    if let Some(target_id) = explicit_recipient_id {
        if let Some(member) = members.iter().find(|item| item.id == target_id) {
            return match member.role {
                MemberRole::Leader => RoutingDecision::SlowPathLeader {
                    reason: "explicit_leader_target".to_string(),
                },
                MemberRole::Worker if member.status.is_live() => RoutingDecision::FastPathWorkers {
                    worker_ids: vec![member.id.clone()],
                    reason: format!("explicit_worker_target:{}", member.name),
                },
                MemberRole::Worker => RoutingDecision::SlowPathLeader {
                    reason: format!("worker_not_live_fallback_leader:{}", member.name),
                },
            };
        }
    }

    // 3. 解析正文中的 @mention 路由
    let mentions = extract_mentions(trimmed);
    if !mentions.is_empty() {
        if mentions
            .iter()
            .any(|name| name.eq_ignore_ascii_case("leader") || name == "主控" || name == "群主")
        {
            return RoutingDecision::SlowPathLeader {
                reason: "mentioned_leader".to_string(),
            };
        }
        if mentions
            .iter()
            .any(|name| name.eq_ignore_ascii_case("all") || name == "所有人")
        {
            return RoutingDecision::SlowPathLeader {
                reason: "mentioned_all_via_leader".to_string(),
            };
        }
        let matched_workers: Vec<String> = members
            .iter()
            .filter(|member| {
                member.role == MemberRole::Worker
                    && member.status.is_live()
                    && mentions
                        .iter()
                        .any(|mention| member.name.eq_ignore_ascii_case(mention) || member.id == *mention)
            })
            .map(|member| member.id.clone())
            .collect();
        if !matched_workers.is_empty() {
            return RoutingDecision::FastPathWorkers {
                worker_ids: matched_workers,
                reason: "mentioned_worker_fast_path".to_string(),
            };
        }
    }

    // 4. 线程订阅直通（论文 3.2 节：已订阅 Thread 的后续消息直通活跃 owner Worker，避免 Leader 成为瓶颈）
    let owner_workers: Vec<String> = thread_subscriptions
        .iter()
        .filter(|sub| sub.mode == SubscriptionMode::Owner)
        .filter_map(|sub| {
            members
                .iter()
                .find(|member| {
                    member.id == sub.member_id
                        && member.role == MemberRole::Worker
                        && member.status.is_live()
                })
                .map(|member| member.id.clone())
        })
        .collect();
    if !owner_workers.is_empty() {
        return RoutingDecision::FastPathWorkers {
            worker_ids: owner_workers,
            reason: "subscribed_thread_fan_out".to_string(),
        };
    }

    // 5. 未订阅的新话题或通用请求交由常驻 Leader 语义路由
    RoutingDecision::SlowPathLeader {
        reason: "unsubscribed_thread_to_leader".to_string(),
    }
}

/// 判断文本是否显式寻址了群组内智能体（`@leader`、`@runner`、`@sai` 或斜杠指令）。
///
/// 参数:
/// - `content`: 消息正文
/// - `members`: 群组成员列表
///
/// 返回:
/// - 命中显式寻址时返回 true
fn contains_explicit_addressing(content: &str, members: &[GroupMember]) -> bool {
    let mentions = extract_mentions(content);
    if mentions.iter().any(|name| {
        name.eq_ignore_ascii_case("sai")
            || name.eq_ignore_ascii_case("leader")
            || name.eq_ignore_ascii_case("all")
            || members
                .iter()
                .any(|member| member.name.eq_ignore_ascii_case(name))
    }) {
        return true;
    }
    let lowered = content.to_ascii_lowercase();
    lowered.starts_with("sai ") || lowered.starts_with("leader ") || content.starts_with('/')
}
