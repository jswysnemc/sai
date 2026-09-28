use super::*;

/// 工具清单的三个状态：隐藏、启用、延迟 load。
fn tool_states() -> [StateStyle; 3] {
    [
        StateStyle {
            mark: "○",
            label: t("hidden", "隐藏"),
            color: DIM,
        },
        StateStyle {
            mark: "●",
            label: t("enabled", "启用"),
            color: OK,
        },
        StateStyle {
            mark: "◐",
            label: t("deferred", "延迟"),
            color: VALUE,
        },
    ]
}

/// Skills 清单的三个状态：不暴露、完整、仅名称。
fn skill_states() -> [StateStyle; 3] {
    [
        StateStyle {
            mark: "○",
            label: t("off", "不暴露"),
            color: DIM,
        },
        StateStyle {
            mark: "●",
            label: t("full", "完整"),
            color: OK,
        },
        StateStyle {
            mark: "◐",
            label: t("name only", "仅名称"),
            color: VALUE,
        },
    ]
}

/// 在多态清单中编辑 Agent 工具白名单与延迟集合。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `paths`: Sai 路径
/// - `config`: 当前应用配置（枚举工具目录）
/// - `profile`: 待更新档案
///
/// 返回:
/// - 清单退出结果；保存时写回 `enabled_tools` / `deferred_tools`
pub(super) fn edit_agent_tools(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &AppConfig,
    profile: &mut AgentProfile,
) -> Result<()> {
    // 1. 枚举本地工具目录，按分组权重排序（基础最先，SSH 紧随其后）
    let mut catalog = crate::tools::tool_catalog(config, paths);
    catalog.sort_by(|left, right| {
        (left.group_rank, left.group, left.name.as_str()).cmp(&(
            right.group_rank,
            right.group,
            right.name.as_str(),
        ))
    });
    let known = catalog
        .iter()
        .map(|entry| entry.name.clone())
        .collect::<BTreeSet<_>>();
    let mut entries = catalog
        .into_iter()
        .map(|entry| {
            let description = if entry.group_hint.is_empty() {
                entry.description
            } else {
                format!(
                    "{}\n\n{}",
                    entry.description,
                    t(entry.group_hint_en, entry.group_hint)
                )
            };
            SelectEntry {
                state: initial_tool_state(profile, &entry.name),
                description,
                group_label: t(entry.group_label_en, entry.group_label).to_string(),
                key: entry.name,
            }
        })
        .collect::<Vec<_>>();
    // 2. 配置里已有但目录未注册的名称（MCP 动态工具、别名）保留展示，防止写回丢失
    let unknown = profile
        .enabled_tools
        .iter()
        .chain(profile.deferred_tools.iter())
        .filter(|name| name.as_str() != DEFERRED_ALL_NON_BASE && !known.contains(*name))
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in unknown {
        entries.push(SelectEntry {
            state: initial_tool_state(profile, &name),
            description: t(
                "Present in config but not currently registered (e.g. MCP dynamic tool). Kept as configured.",
                "存在于配置但当前未注册（如 MCP 动态工具），按原配置保留。",
            )
            .to_string(),
            group_label: t("Dynamic / unknown", "动态 / 未知").to_string(),
            key: name,
        });
    }
    let mut toggles = vec![
        HeaderToggle {
            label: t(
                "Whitelist mode (off = inherit all tools)",
                "白名单模式（关 = 继承全量工具）",
            )
            .to_string(),
            description: t(
                "On: only tools marked enabled/deferred below are available. Off: the agent inherits every tool; the enabled marks below are ignored and only deferred marks matter.",
                "开启：仅下方标记为启用/延迟的工具可用。关闭：继承全部工具，下方启用标记不生效，仅延迟标记有意义。",
            )
            .to_string(),
            value: !profile.enabled_tools.is_empty(),
        },
        HeaderToggle {
            label: t(
                "Defer all non-base tools (*)",
                "全部非基础工具延迟 load（*）",
            )
            .to_string(),
            description: t(
                "Writes the wildcard `*` into deferred tools: base tools stay visible, everything else must be loaded on demand. Per-tool deferred marks are ignored while this is on.",
                "向延迟集合写入通配符 `*`：基础工具直接可见，其余工具需模型按需 load。开启期间逐项延迟标记不生效。",
            )
            .to_string(),
            value: profile
                .deferred_tools
                .iter()
                .any(|name| name == DEFERRED_ALL_NON_BASE),
        },
    ];
    if run_multi_select(
        stdout,
        t(" AGENT TOOLS ", " AGENT 工具能力 "),
        &tool_states(),
        &mut toggles,
        &mut entries,
    )? {
        let whitelist = toggles[0].value;
        let wildcard = toggles[1].value;
        profile.enabled_tools = if whitelist {
            entries
                .iter()
                .filter(|entry| entry.state >= 1)
                .map(|entry| entry.key.clone())
                .collect()
        } else {
            Vec::new()
        };
        let deferred = if wildcard {
            vec![DEFERRED_ALL_NON_BASE.to_string()]
        } else {
            entries
                .iter()
                .filter(|entry| entry.state == 2)
                .map(|entry| entry.key.clone())
                .collect()
        };
        profile.deferred_tools = normalize_deferred_tools(&profile.enabled_tools, &deferred);
    }
    Ok(())
}

/// 返回工具条目在清单中的初始状态。
///
/// 全量继承模式（白名单为空）下工具默认显示为启用，
/// 与运行时「全部可用」的实际语义一致。
pub(super) fn initial_tool_state(profile: &AgentProfile, name: &str) -> usize {
    if profile.deferred_tools.iter().any(|tool| tool == name) {
        2
    } else if profile.enabled_tools.is_empty()
        || profile.enabled_tools.iter().any(|tool| tool == name)
    {
        1
    } else {
        0
    }
}

/// 在多态清单中编辑 Agent 的 Skills 暴露级别。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `paths`: Sai 路径
/// - `config`: 当前应用配置（扫描 Skills 目录）
/// - `profile`: 待更新档案
///
/// 返回:
/// - 清单退出结果；保存时写回 `skills_full` / `skills_named`
pub(super) fn edit_agent_skills(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &AppConfig,
    profile: &mut AgentProfile,
) -> Result<()> {
    let catalog = crate::tools::skill_catalog(config, paths).unwrap_or_default();
    let known = catalog
        .iter()
        .map(|entry| entry.name.clone())
        .collect::<BTreeSet<_>>();
    let mut entries = catalog
        .into_iter()
        .map(|entry| SelectEntry {
            state: initial_skill_state(profile, &entry.name),
            description: entry.description,
            group_label: String::new(),
            key: entry.name,
        })
        .collect::<Vec<_>>();
    // 配置中残留但目录已不存在的 skill 名称保留展示
    let unknown = profile
        .skills_full
        .iter()
        .chain(profile.skills_named.iter())
        .filter(|name| !known.contains(*name))
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in unknown {
        entries.push(SelectEntry {
            state: initial_skill_state(profile, &name),
            description: t(
                "Configured but not found in the skills directory. Kept as configured.",
                "已配置但 Skills 目录中未找到，按原配置保留。",
            )
            .to_string(),
            group_label: t("Missing", "未找到").to_string(),
            key: name,
        });
    }
    if entries.is_empty() {
        message(
            stdout,
            t(
                "No skills installed. Manage skills from the main menu Skills page.",
                "尚未安装任何 Skill，可在主菜单 Skills 页安装与管理。",
            ),
        )?;
        return Ok(());
    }
    if run_multi_select(
        stdout,
        t(" AGENT SKILLS ", " AGENT SKILLS "),
        &skill_states(),
        &mut [],
        &mut entries,
    )? {
        profile.skills_full = entries
            .iter()
            .filter(|entry| entry.state == 1)
            .map(|entry| entry.key.clone())
            .collect();
        profile.skills_named = entries
            .iter()
            .filter(|entry| entry.state == 2)
            .map(|entry| entry.key.clone())
            .collect();
    }
    Ok(())
}

/// 返回 skill 条目在清单中的初始状态。
pub(super) fn initial_skill_state(profile: &AgentProfile, name: &str) -> usize {
    if profile.skills_full.iter().any(|skill| skill == name) {
        1
    } else if profile.skills_named.iter().any(|skill| skill == name) {
        2
    } else {
        0
    }
}
