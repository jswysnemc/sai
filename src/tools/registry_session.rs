use super::ToolRegistry;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use std::path::Path;

impl ToolRegistry {
    /// 【会话工具】【身份重绑】同步交互工具回调和权限归属，保留已启用集合、顺序及扩展工具。
    /// 参数: config、paths 为当前配置与路径，session_id、state_dir 为 Agent 实际会话
    /// 返回: 无；非交互注册表或身份相同时不重建
    pub(crate) fn rebind_interactive_session(
        &mut self,
        config: &AppConfig,
        paths: &SaiPaths,
        session_id: &str,
        state_dir: &Path,
    ) {
        if self.session_key.is_empty()
            || (self.session_id == session_id
                && Path::new(&self.session_key) == state_dir
                && self.mesh_cross_session == config.mesh.cross_session)
        {
            return;
        }
        // 1. 【会话工具】【回调重建】排除旧子代理闭包，避免多次切换累积嵌套注册表
        let owner_key = state_dir.display().to_string();
        let mut refreshed = self.clone_excluding(&["subagent"]);
        refreshed.set_session_ownership(
            owner_key.clone(),
            session_id.into(),
            config.mesh.cross_session,
        );
        if let Some(profile) = refreshed.permission_profile.take() {
            refreshed.permission_profile = Some(profile.rebind_session(state_dir, session_id));
        }
        crate::tools::command::register_session_background(
            &mut refreshed,
            config,
            paths,
            session_id,
        );
        crate::ssh::register(&mut refreshed, paths, session_id);
        crate::tools::mesh::register(
            &mut refreshed,
            paths.clone(),
            owner_key.clone(),
            session_id.into(),
            config.mesh.cross_session,
        );
        // 2. 【会话工具】【范围保留】子代理只继承原先启用的工具；替换时不增加被过滤的工具
        let enabled = self
            .order
            .iter()
            .map(String::as_str)
            .filter(|name| *name != "subagent")
            .collect::<Vec<_>>();
        // 子代理初始注册发生在主会话网格、提问、目标与渐进网关之前，重绑保持同样边界
        let inherited = refreshed.clone_filtered(&enabled).clone_excluding(&[
            "session_probe",
            "agent_probe",
            "mesh_send",
            "session_create",
            "session_activate",
            "ask_question",
            "create_goal",
            "get_goal",
            "update_goal",
            crate::tools::LOAD_NAME,
            crate::tools::INVOKE_NAME,
        ]);
        if self.contains("subagent") {
            crate::tools::subagent::register(
                &mut refreshed,
                config.clone(),
                paths.clone(),
                inherited,
                owner_key.clone(),
                session_id.into(),
            );
        }
        for name in &self.order {
            if let Some(tool) = refreshed.tools.get(name) {
                self.tools.insert(name.clone(), tool.clone());
            }
        }
        // 3. 【会话工具】【权限重绑】切换信箱归属与审计位置，清除旧会话的一次性批准
        self.set_session_ownership(owner_key, session_id.into(), config.mesh.cross_session);
        self.permission_profile = refreshed.permission_profile;
    }
}
