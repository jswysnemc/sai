//! `/config` 主控制台面板：扁平化导航、顶栏状态胶囊与单键快捷切换。

use super::theme::{pill, save_state_pill, ACCENT, BRAND, DIM, MUTED, OK, PILL_BG, RESET};
use crate::config::{AppConfig, DefaultPermissionMode};
use crate::i18n::text as t;

/// 【配置界面】【主菜单动作】扁平化高频入口，消除三层嵌套深菜单。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MainAction {
    ActiveModel,
    Providers,
    Agents,
    PermissionsAndSandbox,
    DisplayAndContext,
    Tools,
    Skills,
    ModelEndpoints,
    Advanced,
    ExitOrSave,
}

/// 【配置界面】【主菜单项】封装左侧导航文案与右侧即时摘要。
pub(super) struct MainItem {
    pub(super) action: MainAction,
    pub(super) label: String,
    pub(super) detail: String,
}

/// 【配置界面】【顶栏摘要】渲染激活模型、权限、保护状态胶囊与瞬态反馈。
///
/// 参数:
/// - `config`: 当前内存配置
/// - `dirty`: 是否有未保存更改
/// - `notice`: 最近一次非阻塞保存或快捷操作提示
///
/// 返回:
/// - 带样式的顶栏摘要字符串
pub(super) fn header_subtitle(config: &AppConfig, dirty: bool, notice: Option<&str>) -> String {
    let active = active_model_label(config);
    let mode = config.permission.tui_mode().as_str();
    let save_pill = save_state_pill(dirty);
    let mut parts = vec![
        format!("{BRAND}◆{RESET} {ACCENT}{active}{RESET}"),
        pill(&format!("TUI: {mode}"), MUTED, PILL_BG),
        save_pill,
    ];
    if let Some(note) = notice.filter(|text| !text.is_empty()) {
        parts.push(pill(note, OK, PILL_BG));
    }
    parts.join(&format!(" {DIM}·{RESET} "))
}

/// 【配置界面】【导航构造】构造扁平主菜单及右侧即时参数预览。
///
/// 参数:
/// - `config`: 当前配置快照
/// - `dirty`: 是否存在未保存更改
///
/// 返回:
/// - 主菜单条目列表
pub(super) fn menu_items(config: &AppConfig, dirty: bool) -> Vec<MainItem> {
    let active = active_model_label(config);
    let enabled_providers = config.providers.iter().filter(|p| p.enabled).count();
    let tools_count = active_tools_count(config);
    let tui_mode = config.permission.tui_mode().as_str();
    let cli_mode = config.permission.cli_mode().as_str();
    let sandbox_state = on_off(config.sandbox.enabled);
    let view_mode = if config.display.fullscreen {
        t("fullscreen", "全屏")
    } else {
        t("inline", "内联")
    };

    let defs = [
        (
            MainAction::ActiveModel,
            t("Active configuration", "激活配置"),
            format!(
                "{}\n\n{}: {active}\n{}",
                t(
                    "Pick which provider/model is active for new chats.",
                    "选择新对话默认使用的供应商与模型。",
                ),
                t("Current", "当前"),
                t(
                    "Press Enter to switch immediately.",
                    "按 Enter 立即切换生效。",
                ),
            ),
        ),
        (
            MainAction::Providers,
            t("Providers and models", "供应商和模型"),
            format!(
                "{}\n\n{}: {} / {}\n{}",
                t(
                    "Browse providers, organizations and models. Press Space on a model to activate immediately.",
                    "浏览供应商、组织与模型；在模型列按 Space 或 u 可一键设为默认。",
                ),
                t("Enabled providers", "已启用供应商"),
                enabled_providers,
                config.providers.len(),
                t("Shortcut: Enter to browse", "快捷键: Enter 打开浏览"),
            ),
        ),
        (
            MainAction::Agents,
            t("Agents", "Agent 配置"),
            format!(
                "{}\n\n{}: {}",
                t(
                    "Create and edit agent profiles: model, prompt, tool capabilities and skills.",
                    "创建与编辑 Agent 档案：模型、提示词、工具能力与 Skills。",
                ),
                t("Custom profiles", "自定义档案数"),
                config.agents.len(),
            ),
        ),
        (
            MainAction::PermissionsAndSandbox,
            t("Permissions & sandbox", "权限与沙箱"),
            format!(
                "{}\n\nTUI: {tui_mode} · CLI: {cli_mode} · {}: {sandbox_state}\n\n{}",
                t(
                    "Default permission modes and OS command isolation.",
                    "默认权限模式与系统级命令沙箱隔离。",
                ),
                t("Sandbox", "沙箱"),
                t(
                    "Quick action: Space / ← → cycles TUI permission mode inline.",
                    "快捷操作: 按 Space 或 ← → 直接切换 TUI 默认权限模式，无需进入子菜单。",
                ),
            ),
        ),
        (
            MainAction::DisplayAndContext,
            t("Display & context", "显示与上下文"),
            format!(
                "{}\n\n{}: {view_mode} · {}: {} · {}: {}\n\n{}",
                t(
                    "Fullscreen vs inline layout, reasoning visibility, tool preview and context budget.",
                    "全屏/内联视图、思考过程、工具详情及上下文预算与压缩。",
                ),
                t("View", "视图"),
                t("Reasoning", "思考"),
                config.display.reasoning,
                t("Tools", "工具"),
                config.display.tool_calls,
                t(
                    "Quick action: Space / ← → toggles fullscreen/inline mode inline.",
                    "快捷操作: 按 Space 或 ← → 直接切换全屏/内联视图。",
                ),
            ),
        ),
        (
            MainAction::Tools,
            t("Tools", "助手工具"),
            format!(
                "{}\n\n{}: {tools_count}",
                t(
                    "Toggle and configure built-in assistant tools and web search.",
                    "启用与配置内置助手工具与 Web 搜索。",
                ),
                t("Enabled plugins", "已启用工具插件"),
            ),
        ),
        (
            MainAction::Skills,
            "Skills",
            format!(
                "{}\n\n{}: {} · {}: {}",
                t(
                    "Enable or disable installed skills and global skill switches.",
                    "启停已安装的 Skill 与全局 Skill 开关。",
                ),
                t("Master switch", "总开关"),
                on_off(config.skills.enabled),
                t("Allow commands", "允许执行命令"),
                on_off(config.skills.allow_command_execution),
            ),
        ),
        (
            MainAction::ModelEndpoints,
            t("Jev and image models", "Jev 与生图模型"),
            t(
                "Manage Jev routing/audit and image generation endpoints, credentials and default models.",
                "管理 Jev 路由审核与生图接口接入、密钥和默认模型。",
            )
            .to_string(),
        ),
        (
            MainAction::Advanced,
            t("Advanced settings", "高级设置"),
            t(
                "Knowledge bases, messaging gateways, global parameters, background limits and mesh.",
                "知识库管理、消息渠道接入、后台命令限制与会话网格。",
            )
            .to_string(),
        ),
        (
            MainAction::ExitOrSave,
            if dirty {
                t("Save and exit", "保存并退出")
            } else {
                t("Exit", "退出")
            },
            if dirty {
                t(
                    "Validate and write all changes immediately with one Enter (no extra popup).",
                    "按 Enter 立即校验并保存全部修改后退出（无二次确认弹窗）。",
                )
                .to_string()
            } else {
                t(
                    "No pending edits. Leave the configuration UI.",
                    "没有未保存的修改。离开配置界面。",
                )
                .to_string()
            },
        ),
    ];

    defs.into_iter()
        .enumerate()
        .map(|(index, (action, label, detail))| {
            let prefix = if index < 9 {
                format!("{} ", index + 1)
            } else {
                "0 ".to_string()
            };
            MainItem {
                action,
                label: format!("{prefix} {label}"),
                detail,
            }
        })
        .collect()
}

/// 【配置界面】【行内快切】直接在主菜单按 Space / 左右键切换高频选项，无需进入子表单。
///
/// 参数:
/// - `config`: 待修改的应用配置
/// - `action`: 当前选中的菜单动作
/// - `forward`: 切换方向
///
/// 返回:
/// - 若完成切换返回简短状态提示，否则返回 None
pub(super) fn quick_cycle(
    config: &mut AppConfig,
    action: MainAction,
    forward: bool,
) -> Option<String> {
    match action {
        MainAction::PermissionsAndSandbox => {
            let modes = [
                DefaultPermissionMode::Yolo,
                DefaultPermissionMode::Audited,
                DefaultPermissionMode::AutoAudit,
            ];
            let current = modes
                .iter()
                .position(|item| *item == config.permission.tui_mode())
                .unwrap_or(0);
            let next = if forward {
                (current + 1) % modes.len()
            } else {
                (current + modes.len() - 1) % modes.len()
            };
            let target = modes[next];
            config.permission.tui_mode = Some(target);
            config.permission.default_mode = target;
            Some(format!("TUI -> {}", target.as_str()))
        }
        MainAction::DisplayAndContext => {
            config.display.fullscreen = !config.display.fullscreen;
            let label = if config.display.fullscreen {
                t("fullscreen", "全屏")
            } else {
                t("inline", "内联")
            };
            Some(format!("{} -> {label}", t("View", "视图")))
        }
        _ => None,
    }
}

/// 【配置界面】【激活模型摘要】返回当前默认供应商与模型的组合显示名称。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 格式化标签
pub(super) fn active_model_label(config: &AppConfig) -> String {
    config
        .provider(None)
        .map(|provider| format!("{} / {}", provider.display_name, provider.default_model))
        .unwrap_or_else(|_| t("not configured", "未配置").to_string())
}

fn active_tools_count(config: &AppConfig) -> usize {
    ["web", "vision", "memory", "calculator"]
        .into_iter()
        .filter(|id| super::plugins::plugin_enabled(config, id))
        .count()
}

fn on_off(value: bool) -> &'static str {
    if value {
        t("on", "启用")
    } else {
        t("off", "关闭")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【配置控制台】【主菜单测试】验证权限与显示选项已前置到主菜单且状态胶囊正确生成。
    #[test]
    fn flattened_dashboard_exposes_quick_settings_and_cycles_inline() {
        let mut config = AppConfig::default();
        let items = menu_items(&config, true);
        assert!(items
            .iter()
            .any(|item| item.action == MainAction::PermissionsAndSandbox));
        assert!(items
            .iter()
            .any(|item| item.action == MainAction::DisplayAndContext));
        let before_mode = config.permission.tui_mode();
        assert!(quick_cycle(&mut config, MainAction::PermissionsAndSandbox, true).is_some());
        assert_ne!(config.permission.tui_mode(), before_mode);
        let before_view = config.display.fullscreen;
        assert!(quick_cycle(&mut config, MainAction::DisplayAndContext, true).is_some());
        assert_ne!(config.display.fullscreen, before_view);
        assert!(!header_subtitle(&config, true, Some("Saved")).is_empty());
    }
}
