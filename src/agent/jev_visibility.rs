use super::ToolVisibility;
use crate::config::{AppConfig, DEFERRED_ALL_NON_BASE};
use crate::jev::{Candidate, CandidateKind, Selection};
use crate::paths::SaiPaths;
use crate::tools::{self, ToolRegistry};
use anyhow::Result;
use serde_json::{json, Value};

impl ToolVisibility {
    /// 创建 Jev 暴露决策模式的可见性状态：基础工具常驻，其余工具一律延迟。
    ///
    /// 参数:
    /// - `configured`: Agent 原有的延迟配置；其中点名的基础工具仍保持延迟
    ///
    /// 返回:
    /// - 已开启 Jev 暴露决策的可见性状态
    pub(super) fn with_jev_routing(configured: &[String]) -> Self {
        let mut deferred = configured.to_vec();
        if !deferred.iter().any(|name| name == DEFERRED_ALL_NON_BASE) {
            deferred.push(DEFERRED_ALL_NON_BASE.to_string());
        }
        let mut visibility = Self::new(deferred);
        visibility.jev_routing = true;
        visibility
    }

    /// 判断当前会话是否由 Jev 决定额外资源的暴露。
    pub(crate) fn is_jev_routing(&self) -> bool {
        self.jev_routing
    }

    /// 判断工具当前是否已经暴露给模型（常驻或已加载）。
    ///
    /// 参数:
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 已暴露时返回 true
    pub(crate) fn is_tool_exposed(&self, name: &str) -> bool {
        !self.requires_load(name) || self.loaded.contains(name)
    }

    /// 收集尚未暴露、需要 Jev 判断的工具与 skill；已暴露的资源不会进入判断列表。
    ///
    /// 参数:
    /// - `registry`: 当前完整工具注册表
    /// - `config`: 当前应用配置
    /// - `paths`: 应用目录路径集合
    ///
    /// 返回:
    /// - 未暴露的候选列表；skill 目录读取失败时只返回工具候选
    pub(crate) fn jev_candidates(
        &self,
        registry: &ToolRegistry,
        config: &AppConfig,
        paths: &SaiPaths,
    ) -> Vec<Candidate> {
        // 1. 工具：只有延迟工具需要判断
        let mut all = registry
            .tool_infos()
            .into_iter()
            .filter(|info| self.requires_load(&info.name))
            .map(|info| Candidate::new(CandidateKind::Tool, info.name, &info.description))
            .collect::<Vec<_>>();
        // 2. skill：按 Agent 策略筛选后的目录
        if config.skills.enabled && config.has_visible_skills() {
            if let Ok(skills) = tools::visible_skill_catalog(config, paths) {
                all.extend(skills.into_iter().map(|entry| {
                    Candidate::new(CandidateKind::Skill, entry.name, &entry.description)
                }));
            }
        }
        // 3. 剔除已暴露的资源
        crate::jev::pending_candidates(all, |candidate| match candidate.kind {
            CandidateKind::Tool => {
                self.is_tool_exposed(&candidate.name)
                    || self.announced_tools.contains(&candidate.name)
            }
            CandidateKind::Skill => {
                self.loaded_skills.contains(&candidate.name)
                    || self.announced_skills.contains(&candidate.name)
            }
        })
    }

    /// 返回模型已经可以直接使用的工具与 skill 名称，作为 Jev 的判断背景。
    ///
    /// 参数:
    /// - `registry`: 当前完整工具注册表
    ///
    /// 返回:
    /// - 已暴露资源名称列表
    pub(crate) fn exposed_resource_names(&self, registry: &ToolRegistry) -> Vec<String> {
        let mut names = registry
            .tool_infos()
            .into_iter()
            .map(|info| info.name)
            .filter(|name| {
                name != tools::LOAD_NAME
                    && name != tools::INVOKE_NAME
                    && name != tools::REQUEST_CAPABILITY_NAME
                    && self.is_tool_exposed(name)
            })
            .collect::<Vec<_>>();
        names.extend(
            self.loaded_skill_order
                .iter()
                .map(|name| format!("skill:{name}")),
        );
        names
    }

    /// 把 Jev 选中的资源标记为已暴露，并生成给模型的结果。
    ///
    /// 参数:
    /// - `registry`: 当前完整工具注册表
    /// - `selection`: Jev 选中的工具与 skill
    /// - `config`: 当前应用配置
    /// - `paths`: 应用目录路径集合
    ///
    /// 返回:
    /// - 包含工具 Schema 与 skill 全文的 JSON 文本
    pub(crate) fn expose_selection(
        &mut self,
        registry: &ToolRegistry,
        selection: &Selection,
        config: &AppConfig,
        paths: &SaiPaths,
    ) -> Result<String> {
        // 1. 工具：复用渐进网关的原子加载，只返回本次新暴露的 Schema
        let loaded = self.load_tools(registry, &selection.tools)?;
        let tools_json = loaded
            .newly_loaded_tools
            .iter()
            .filter_map(|name| {
                registry
                    .definition(name)
                    .map(|definition| json!({"name": name, "definition": definition}))
            })
            .collect::<Vec<_>>();
        // 2. skill：复用 load 的全文读取与已加载记录
        let skills_json = if selection.skills.is_empty() {
            Vec::new()
        } else {
            let output = self.load_skills(&selection.skills, config, paths)?;
            serde_json::from_str::<Value>(&output)?
                .get("skills")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        // 3. 组装统一结果
        let instruction = if tools_json.is_empty() && skills_json.is_empty() {
            "Jev found no additional tool or skill for this need. Continue with the tools already available, or call request_capability again with a more specific description."
        } else {
            "Jev exposed the resources below. Call exposed tools through invoke_tool with the exact tool name and arguments matching the returned schema; do not call them directly. Follow the exposed skill documents when they apply. These resources stay exposed for the rest of the conversation."
        };
        Ok(serde_json::to_string_pretty(&json!({
            "ok": true,
            "router": "jev",
            "tools": tools_json,
            "skills": skills_json,
            "instruction": instruction,
        }))?)
    }

    /// 预选注入工具 Schema，skill 只注入名称和描述。
    ///
    /// 工具会记成已 load，之后可直接 invoke_tool。skill 离开后续预选候选，
    /// 但不会记成已 load，模型要执行流程时仍调用 load 读取全文。
    ///
    /// 参数:
    /// - `registry`: 当前完整工具注册表
    /// - `selection`: Jev 选中的工具与 skill
    /// - `config`: 当前应用配置
    /// - `paths`: 应用目录路径集合
    ///
    /// 返回:
    /// - 工具带 Schema、skill 只有名称和描述的 JSON 文本
    pub(crate) fn announce_selection(
        &mut self,
        registry: &ToolRegistry,
        selection: &Selection,
        config: &AppConfig,
        paths: &SaiPaths,
    ) -> Result<String> {
        let loaded = self.load_tools(registry, &selection.tools)?;
        let tools_json = loaded
            .newly_loaded_tools
            .iter()
            .filter_map(|name| {
                registry
                    .definition(name)
                    .map(|definition| json!({"name": name, "definition": definition}))
            })
            .collect::<Vec<_>>();
        let catalog = if config.skills.enabled && !selection.skills.is_empty() {
            tools::visible_skill_catalog(config, paths).unwrap_or_default()
        } else {
            Vec::new()
        };
        let mut skills_json = Vec::new();
        for name in &selection.skills {
            if self.loaded_skills.contains(name) || self.announced_skills.contains(name) {
                continue;
            }
            let Some(entry) = catalog.iter().find(|entry| entry.name == *name) else {
                continue;
            };
            self.announced_skills.insert(name.clone());
            skills_json.push(json!({
                "name": entry.name,
                "description": entry.description,
            }));
        }
        let instruction = if tools_json.is_empty() && skills_json.is_empty() {
            "Jev found no additional tool or skill for this need. Continue with the tools already available, or call request_capability again with a more specific description."
        } else {
            "Jev exposed the tool schemas below. Call those tools through invoke_tool with the exact name and arguments matching the schema. Listed skills include only a name and description; call load with type skill and that exact name before following the document."
        };
        Ok(serde_json::to_string_pretty(&json!({
            "ok": true,
            "router": "jev",
            "tools": tools_json,
            "skills": skills_json,
            "instruction": instruction,
        }))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolSpec;

    fn registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        for name in ["read_file", "web_search", "analyze_image"] {
            registry.register(ToolSpec::new(
                name,
                format!("{name} description."),
                json!({"type":"object","properties":{},"additionalProperties":false}),
                |_| async { Ok("ok".to_string()) },
            ));
        }
        registry
    }

    #[test]
    fn jev_routing_defers_all_non_base_tools() {
        let visibility = ToolVisibility::with_jev_routing(&[]);
        assert!(visibility.is_jev_routing());
        assert!(visibility.is_tool_exposed("read_file"));
        assert!(!visibility.is_tool_exposed("web_search"));
        assert!(!visibility.requires_load(tools::REQUEST_CAPABILITY_NAME));
    }

    #[test]
    fn exposed_tools_leave_the_candidate_list() {
        let registry = registry();
        let temp = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(temp.path());
        let mut config = AppConfig::default();
        config.skills.enabled = false;
        let mut visibility = ToolVisibility::with_jev_routing(&[]);

        let before = visibility.jev_candidates(&registry, &config, &paths);
        let names = before
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["analyze_image", "web_search"]);

        let selection = Selection {
            tools: vec!["web_search".to_string()],
            skills: Vec::new(),
        };
        let output = visibility
            .expose_selection(&registry, &selection, &config, &paths)
            .unwrap();
        let output = serde_json::from_str::<Value>(&output).unwrap();
        assert_eq!(output["tools"][0]["name"], "web_search");
        assert!(visibility.is_visible("web_search"));

        let after = visibility.jev_candidates(&registry, &config, &paths);
        let names = after
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["analyze_image"]);
        assert!(visibility
            .exposed_resource_names(&registry)
            .contains(&"web_search".to_string()));
    }

    #[test]
    fn exposed_skills_leave_the_candidate_list() {
        let registry = registry();
        let temp = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(temp.path());
        let skill_dir = paths.skills_dir.join("drawio");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: drawio\ndescription: Draw diagrams\n---\n\nUse drawio.",
        )
        .unwrap();
        let config = AppConfig::default();
        let mut visibility = ToolVisibility::with_jev_routing(&[]);

        let has_skill = |visibility: &ToolVisibility| {
            visibility
                .jev_candidates(&registry, &config, &paths)
                .iter()
                .any(|item| item.kind == CandidateKind::Skill && item.name == "drawio")
        };
        assert!(has_skill(&visibility));
        let selection = Selection {
            tools: Vec::new(),
            skills: vec!["drawio".to_string()],
        };
        let output = visibility
            .expose_selection(&registry, &selection, &config, &paths)
            .unwrap();
        assert!(output.contains("Use drawio."));
        assert!(!has_skill(&visibility));
    }

    #[test]
    fn empty_selection_reports_no_match() {
        let registry = registry();
        let temp = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(temp.path());
        let mut visibility = ToolVisibility::with_jev_routing(&[]);
        let output = visibility
            .expose_selection(
                &registry,
                &Selection::default(),
                &AppConfig::default(),
                &paths,
            )
            .unwrap();
        assert!(output.contains("found no additional"));
    }

    #[test]
    fn preselect_announces_name_and_description_without_the_document() {
        let registry = registry();
        let temp = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(temp.path());
        let skill_dir = paths.skills_dir.join("design-taste-frontend");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: design-taste-frontend\ndescription: Frontend design rules\n---\n\nFULL SKILL BODY THAT MUST STAY OUT\n",
        )
        .unwrap();
        let config = AppConfig::default();
        let mut visibility = ToolVisibility::with_jev_routing(&[]);
        let selection = Selection {
            tools: vec!["web_search".to_string()],
            skills: vec!["design-taste-frontend".to_string()],
        };
        let output = visibility
            .announce_selection(&registry, &selection, &config, &paths)
            .unwrap();
        let parsed = serde_json::from_str::<Value>(&output).unwrap();
        assert_eq!(parsed["tools"][0]["name"], "web_search");
        assert!(parsed["tools"][0]["definition"]["function"]["parameters"].is_object());
        assert!(visibility.is_tool_exposed("web_search"));
        assert_eq!(parsed["skills"][0]["name"], "design-taste-frontend");
        assert_eq!(parsed["skills"][0]["description"], "Frontend design rules");
        assert!(parsed["skills"][0].get("content").is_none());
        assert!(!output.contains("FULL SKILL BODY"));
        assert!(!visibility
            .jev_candidates(&registry, &config, &paths)
            .iter()
            .any(|item| item.name == "design-taste-frontend" || item.name == "web_search"));

        let loaded = visibility
            .load_skills(
                &["design-taste-frontend".to_string()],
                &config,
                &paths,
            )
            .unwrap();
        assert!(loaded.contains("FULL SKILL BODY"));
    }
}
