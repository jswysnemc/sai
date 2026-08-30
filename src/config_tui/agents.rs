use crate::config::{
    normalize_deferred_tools, AgentProfile, AppConfig, DEFAULT_AGENT_ID, DEFERRED_ALL_NON_BASE,
    EXPLORE_AGENT_ID, GATEWAY_AGENT_ID, GENERAL_AGENT_ID,
};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::collections::BTreeSet;
use std::io;

use super::form::{
    edit_textarea, parse_bool_field, parse_provider_model_choice, provider_model_choice_values,
    run_form, Field,
};
use super::input::read_key;
use super::multi_select::{run_multi_select, HeaderToggle, SelectEntry, StateStyle};
use super::theme::{DIM, OK, VALUE};
use super::ui::{draw_menu_with_details, message, truncate};

/// 编辑统一 Agent 档案和各运行入口默认项。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `paths`: Sai 路径（枚举工具与 Skills 目录）
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 编辑流程是否成功
pub(crate) fn edit_agents(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<()> {
    let mut selected = 0usize;
    let mut status = String::new();
    loop {
        let profiles = visible_profiles(config);
        let mut options = vec![t("Surface defaults", "入口默认 Agent").to_string()];
        options.extend(
            profiles
                .iter()
                .map(|profile| format!("{} [{}]", profile.name, profile.id)),
        );
        options.push(t("Add Agent", "新增 Agent").to_string());
        let mut details = vec![t(
            "Choose which Agent profile each surface (Web / TUI / CLI) uses by default.",
            "为 Web / TUI / CLI 各入口选择默认 Agent 档案。",
        )
        .to_string()];
        details.extend(profiles.iter().map(profile_overview));
        details.push(
            t(
                "Create a new custom Agent profile and open its editor.",
                "新建自定义 Agent 档案并打开编辑器。",
            )
            .to_string(),
        );
        draw_menu_with_details(
            stdout,
            t(" AGENTS ", " AGENT 配置 "),
            &options,
            &details,
            selected,
            &if status.is_empty() {
                super::theme::help_line(&[
                    ("Enter", t("edit", "编辑")),
                    ("d", t("delete custom Agent", "删除自定义 Agent")),
                    ("q", t("back", "返回")),
                ])
            } else {
                status.clone()
            },
            "",
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len().saturating_sub(1))
            }
            KeyCode::Char('d') if selected > 0 && selected <= profiles.len() => {
                let id = profiles[selected - 1].id.clone();
                if !is_builtin(&id) {
                    // 删除不可撤销，且 d 与 Enter 同区，默认必须停在「取消」
                    if super::ui::confirm_delete(
                        stdout,
                        &t(" DELETE AGENT ", " 删除自定义 Agent "),
                        &id,
                        &t(
                            "Removes this Agent profile permanently.",
                            "永久删除该 Agent 档案，无法恢复。",
                        ),
                    )? {
                        config.agents.retain(|profile| profile.id != id);
                        selected = selected.saturating_sub(1);
                        status = format!("{}: {id}", t("Removed agent", "已删除 Agent"));
                    } else {
                        status = t("Delete cancelled", "已取消删除").to_string();
                    }
                }
            }
            KeyCode::Enter if selected == 0 => edit_surface_defaults(stdout, config)?,
            KeyCode::Enter if selected == options.len() - 1 => {
                let profile = new_agent(config);
                let id = profile.id.clone();
                config.agents.push(profile);
                let profiles = visible_profiles(config);
                selected = profiles
                    .iter()
                    .position(|profile| profile.id == id)
                    .map(|index| index + 1)
                    .unwrap_or(0);
            }
            KeyCode::Enter if selected > 0 && selected <= profiles.len() => {
                edit_agent_profile(stdout, paths, config, profiles[selected - 1].clone())?;
            }
            _ => {}
        }
    }
}

/// Agent 列表右侧的档案概览。
fn profile_overview(profile: &AgentProfile) -> String {
    let kind = if is_builtin(&profile.id) {
        t("Built-in profile", "内置档案")
    } else {
        t("Custom profile — d to delete.", "自定义档案 — d 删除。")
    };
    format!(
        "{kind}\n\n{}\n{}",
        tools_summary(profile),
        skills_summary(profile)
    )
}

/// 编辑 Web、TUI 与 CLI 默认 Agent。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单编辑结果
fn edit_surface_defaults(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let choices = agent_choice_ids(config);
    let mut fields = vec![
        Field::new(
            t("Web default Agent", "Web 默认 Agent"),
            config
                .default_agent
                .clone()
                .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
        )
        .choices_owned(choices.clone()),
        Field::new(
            t("TUI default Agent", "TUI 默认 Agent"),
            config
                .tui_agent
                .clone()
                .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
        )
        .choices_owned(choices.clone()),
        Field::new(
            t("CLI default Agent", "CLI 默认 Agent"),
            config
                .cli_agent
                .clone()
                .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
        )
        .choices_owned(choices.clone()),
        Field::new(
            t("Gateway default Agent", "网关默认 Agent"),
            config
                .gateway_agent
                .clone()
                .unwrap_or_else(|| GATEWAY_AGENT_ID.to_string()),
        )
        .choices_owned(choices),
    ];
    if run_form(
        stdout,
        t(" AGENT DEFAULTS ", " AGENT 入口默认值 "),
        &mut fields,
    )? {
        config.default_agent = optional_agent_id(&fields[0].value);
        config.tui_agent = optional_agent_id(&fields[1].value);
        config.cli_agent = optional_agent_id(&fields[2].value);
        config.gateway_agent = optional_agent_id(&fields[3].value);
    }
    Ok(())
}

/// Agent 编辑菜单：基本信息、系统提示词、工具与 Skills 分区编辑。
///
/// 修改先落在本地档案副本上，选择「保存修改」才写入内存配置；
/// q 返回则放弃全部修改。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `paths`: Sai 路径
/// - `config`: 待更新应用配置
/// - `profile`: 当前 Agent 档案副本
///
/// 返回:
/// - 编辑流程是否成功
fn edit_agent_profile(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
    mut profile: AgentProfile,
) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let options = vec![
            t("Basic info", "基本信息").to_string(),
            t("System prompt", "系统提示词").to_string(),
            t("Tool capabilities", "工具能力").to_string(),
            "Skills".to_string(),
            t("Save changes", "保存修改").to_string(),
        ];
        let details = vec![
            basics_summary(&profile),
            prompt_summary(&profile),
            tools_summary(&profile),
            skills_summary(&profile),
            t(
                "Write this profile into the in-memory config. Use Save & Exit on the main menu to persist to disk. q discards edits.",
                "把档案写入内存配置；主菜单「保存并退出」才会落盘。q 放弃本次修改。",
            )
            .to_string(),
        ];
        let subtitle = format!("{} [{}]", profile.name, profile.id);
        draw_menu_with_details(
            stdout,
            t(" EDIT AGENT ", " 编辑 AGENT "),
            &options,
            &details,
            selected,
            &super::theme::help_line(&[
                ("Enter", t("open", "进入")),
                ("s", t("save", "保存")),
                ("q", t("discard", "放弃")),
            ]),
            &subtitle,
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len().saturating_sub(1))
            }
            KeyCode::Char('s') => {
                upsert_agent(config, profile);
                return Ok(());
            }
            KeyCode::Enter => match selected {
                0 => edit_agent_basics(stdout, config, &mut profile)?,
                1 => edit_textarea(stdout, &mut profile.system_prompt)?,
                2 => edit_agent_tools(stdout, paths, config, &mut profile)?,
                3 => edit_agent_skills(stdout, paths, config, &mut profile)?,
                4 => {
                    upsert_agent(config, profile);
                    return Ok(());
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// 编辑 Agent 基本信息（名称、描述、模型、思考等级与注册开关）。
fn edit_agent_basics(
    stdout: &mut io::Stdout,
    config: &AppConfig,
    profile: &mut AgentProfile,
) -> Result<()> {
    let model_value = if profile.provider_id.is_empty() || profile.model.is_empty() {
        String::new()
    } else {
        format!("{}\t{}", profile.provider_id, profile.model)
    };
    let mut fields = vec![
        Field::new(t("Display name", "显示名称"), profile.name.clone()),
        Field::new(t("Description", "用途描述"), profile.description.clone()),
        Field::new(t("Provider/model", "供应商/模型"), model_value)
            .choices_owned(provider_model_choice_values(config, false))
            .empty_choice_label(t("Inherit current model", "沿用当前模型")),
        Field::new(
            t("Thinking level", "思考等级"),
            profile.thinking_level.clone(),
        )
        .choices(&["auto", "none", "low", "medium", "high", "xhigh", "max"]),
        Field::boolean(
            t("Register to main Agent", "向主 Agent 注册"),
            profile.register_to_main,
        ),
        Field::boolean(
            t("Load AGENT.md instruction files", "加载 AGENT.md 指令文件"),
            profile.load_instruction_files,
        ),
    ];
    loop {
        if !run_form(stdout, t(" AGENT BASICS ", " AGENT 基本信息 "), &mut fields)? {
            return Ok(());
        }
        // 布尔字段由表单开关保证合法，仍统一解析以防手输异常值
        let (register_to_main, load_instruction_files) = match parse_bool_field(&fields[4].value)
            .and_then(|register| Ok((register, parse_bool_field(&fields[5].value)?)))
        {
            Ok(values) => values,
            Err(err) => {
                message(
                    stdout,
                    &format!("{}: {err}", t("Invalid input", "输入无效")),
                )?;
                continue;
            }
        };
        profile.name = fields[0].value.trim().to_string();
        profile.description = fields[1].value.trim().to_string();
        (profile.provider_id, profile.model) = parse_provider_model_choice(&fields[2].value);
        profile.thinking_level = fields[3].value.trim().to_string();
        profile.register_to_main = register_to_main;
        profile.load_instruction_files = load_instruction_files;
        return Ok(());
    }
}

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
fn edit_agent_tools(
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
fn initial_tool_state(profile: &AgentProfile, name: &str) -> usize {
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
fn edit_agent_skills(
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
fn initial_skill_state(profile: &AgentProfile, name: &str) -> usize {
    if profile.skills_full.iter().any(|skill| skill == name) {
        1
    } else if profile.skills_named.iter().any(|skill| skill == name) {
        2
    } else {
        0
    }
}

/// 基本信息分区的当前值摘要。
fn basics_summary(profile: &AgentProfile) -> String {
    let model = if profile.provider_id.is_empty() {
        t("inherit current model", "沿用当前模型").to_string()
    } else if profile.model.is_empty() {
        profile.provider_id.clone()
    } else {
        format!("{} / {}", profile.provider_id, profile.model)
    };
    let description = if profile.description.trim().is_empty() {
        t("(no description)", "（无描述）").to_string()
    } else {
        profile.description.clone()
    };
    format!(
        "{description}\n\n{}: {model}\n{}: {}\n{}: {} · AGENT.md: {}",
        t("Model", "模型"),
        t("Thinking", "思考等级"),
        profile.thinking_level,
        t("Register to main", "注册主 Agent"),
        bool_text(profile.register_to_main),
        bool_text(profile.load_instruction_files),
    )
}

/// 系统提示词分区的当前值摘要。
fn prompt_summary(profile: &AgentProfile) -> String {
    let prompt = profile.system_prompt.trim();
    if prompt.is_empty() {
        t(
            "Empty — the agent uses the built-in prompt. Enter opens $EDITOR.",
            "为空 — 使用内置提示词。Enter 打开 $EDITOR 编辑。",
        )
        .to_string()
    } else {
        format!(
            "{} {} · {}\n\n{}",
            prompt.chars().count(),
            t("chars", "字符"),
            t("Enter opens $EDITOR", "Enter 打开 $EDITOR"),
            truncate(&prompt.replace('\n', " "), 160)
        )
    }
}

/// 工具能力分区的当前值摘要。
fn tools_summary(profile: &AgentProfile) -> String {
    let wildcard = profile
        .deferred_tools
        .iter()
        .any(|name| name == DEFERRED_ALL_NON_BASE);
    let deferred_count = profile
        .deferred_tools
        .iter()
        .filter(|name| name.as_str() != DEFERRED_ALL_NON_BASE)
        .count();
    let base = if profile.enabled_tools.is_empty() {
        t("Tools: inherit all", "工具：继承全量").to_string()
    } else {
        format!(
            "{}: {} {}",
            t("Tools", "工具"),
            profile.enabled_tools.len(),
            t("whitelisted", "项白名单")
        )
    };
    let deferred = if wildcard {
        t("all non-base tools deferred (*)", "非基础工具全部延迟（*）").to_string()
    } else if deferred_count > 0 {
        format!("{deferred_count} {}", t("deferred", "项延迟"))
    } else {
        t("none deferred", "无延迟").to_string()
    };
    format!("{base} · {deferred}")
}

/// Skills 分区的当前值摘要。
fn skills_summary(profile: &AgentProfile) -> String {
    if profile.skills_full.is_empty() && profile.skills_named.is_empty() {
        t(
            "Skills: not restricted (all visible when no capability override is set)",
            "Skills：未单独配置（无其他能力限制时全部可见）",
        )
        .to_string()
    } else {
        format!(
            "Skills: {} {} · {} {}",
            profile.skills_full.len(),
            t("full", "完整"),
            profile.skills_named.len(),
            t("name only", "仅名称")
        )
    }
}

fn bool_text(value: bool) -> &'static str {
    if value {
        t("yes", "是")
    } else {
        t("no", "否")
    }
}

/// 返回 TUI 可编辑的默认、内置和自定义 Agent。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - Agent 档案列表
fn visible_profiles(config: &AppConfig) -> Vec<AgentProfile> {
    let mut profiles = config.resolved_agent_profiles();
    if !profiles
        .iter()
        .any(|profile| profile.id == DEFAULT_AGENT_ID)
    {
        profiles.insert(
            0,
            AgentProfile {
                id: DEFAULT_AGENT_ID.to_string(),
                name: t("Default Agent", "默认 Agent").to_string(),
                description: t("Inherit global configuration", "继承全局配置").to_string(),
                ..AgentProfile::default()
            },
        );
    }
    profiles
}

/// 创建不与现有标识冲突的自定义 Agent。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 新 Agent 档案
fn new_agent(config: &AppConfig) -> AgentProfile {
    let used = config
        .resolved_agent_profiles()
        .into_iter()
        .map(|profile| profile.id)
        .collect::<std::collections::HashSet<_>>();
    let mut index = 1usize;
    while used.contains(&format!("agent-{index}")) {
        index += 1;
    }
    AgentProfile {
        id: format!("agent-{index}"),
        name: format!("{} {index}", t("New Agent", "新 Agent")),
        thinking_level: "auto".to_string(),
        ..AgentProfile::default()
    }
}

/// 写入或替换指定 Agent 档案。
///
/// 参数:
/// - `config`: 待更新应用配置
/// - `profile`: Agent 档案
///
/// 返回:
/// - 无
fn upsert_agent(config: &mut AppConfig, profile: AgentProfile) {
    if let Some(existing) = config
        .agents
        .iter_mut()
        .find(|existing| existing.id == profile.id)
    {
        *existing = profile;
    } else {
        config.agents.push(profile);
    }
}

/// 返回所有入口默认项可以选择的 Agent 标识。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 去重后的 Agent 标识
fn agent_choice_ids(config: &AppConfig) -> Vec<String> {
    let mut ids = vec![DEFAULT_AGENT_ID.to_string()];
    ids.extend(
        config
            .resolved_agent_profiles()
            .into_iter()
            .map(|profile| profile.id),
    );
    ids.sort();
    ids.dedup();
    ids
}

/// 将默认 Agent 标识转换为空配置值。
///
/// 参数:
/// - `value`: 表单 Agent 标识
///
/// 返回:
/// - 非默认 Agent 标识
fn optional_agent_id(value: &str) -> Option<String> {
    let value = value.trim();
    (value != DEFAULT_AGENT_ID && !value.is_empty()).then(|| value.to_string())
}

/// 判断 Agent 是否为不可删除的内置档案。
///
/// 参数:
/// - `id`: Agent 标识
///
/// 返回:
/// - 是否为内置档案
fn is_builtin(id: &str) -> bool {
    matches!(
        id,
        DEFAULT_AGENT_ID | GENERAL_AGENT_ID | EXPLORE_AGENT_ID | GATEWAY_AGENT_ID
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证入口默认选项包含虚拟默认项和内置 Agent。
    #[test]
    fn agent_choices_include_default_and_builtins() {
        let choices = agent_choice_ids(&AppConfig::default());

        assert!(choices.contains(&DEFAULT_AGENT_ID.to_string()));
        assert!(choices.contains(&GENERAL_AGENT_ID.to_string()));
        assert!(choices.contains(&EXPLORE_AGENT_ID.to_string()));
        assert!(choices.contains(&GATEWAY_AGENT_ID.to_string()));
    }

    /// 全量继承模式下工具默认显示为启用，延迟标记优先。
    #[test]
    fn tool_state_reflects_inherit_and_deferred() {
        let mut profile = AgentProfile::default();
        assert_eq!(initial_tool_state(&profile, "read_file"), 1);

        profile.deferred_tools = vec!["show_meme".to_string()];
        assert_eq!(initial_tool_state(&profile, "show_meme"), 2);

        profile.enabled_tools = vec!["read_file".to_string(), "show_meme".to_string()];
        assert_eq!(initial_tool_state(&profile, "read_file"), 1);
        assert_eq!(initial_tool_state(&profile, "web_search"), 0);
    }

    /// Skill 状态映射：完整优先于仅名称，未配置为不暴露。
    #[test]
    fn skill_state_maps_full_and_named() {
        let profile = AgentProfile {
            skills_full: vec!["research".to_string()],
            skills_named: vec!["drawio".to_string()],
            ..AgentProfile::default()
        };

        assert_eq!(initial_skill_state(&profile, "research"), 1);
        assert_eq!(initial_skill_state(&profile, "drawio"), 2);
        assert_eq!(initial_skill_state(&profile, "other"), 0);
    }

    /// 工具摘要区分全量继承、白名单与通配延迟。
    #[test]
    fn tools_summary_covers_modes() {
        let mut profile = AgentProfile::default();
        let inherit = tools_summary(&profile);
        assert!(
            inherit.contains("inherit all") || inherit.contains("继承全量"),
            "unexpected inherit summary: {inherit}"
        );

        profile.deferred_tools = vec![DEFERRED_ALL_NON_BASE.to_string()];
        assert!(tools_summary(&profile).contains('*'));

        profile.enabled_tools = vec!["read_file".to_string(), "grep".to_string()];
        profile.deferred_tools = vec!["grep".to_string()];
        let summary = tools_summary(&profile);
        assert!(summary.contains('2'));
        assert!(summary.contains('1'));
    }
}
