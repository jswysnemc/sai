use super::tool_visibility::ToolVisibility;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::tools;
use anyhow::{bail, Result};
use serde_json::json;
use std::collections::BTreeSet;

impl ToolVisibility {
    /// 恢复本会话已经 load 过的 skill 名称。
    ///
    /// 参数:
    /// - `names`: 持久化的 skill 名称
    ///
    /// 返回:
    /// - 无
    pub(crate) fn restore_loaded_skills(&mut self, names: &[String]) {
        self.loaded_skills.clear();
        self.loaded_skill_order.clear();
        for name in names {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            if self.loaded_skills.insert(name.to_string()) {
                self.loaded_skill_order.push(name.to_string());
            }
        }
    }

    /// 返回本会话已 load 的 skill 名称（首次加载顺序）。
    ///
    /// 返回:
    /// - skill 名称列表
    pub(crate) fn loaded_skill_names(&self) -> Vec<String> {
        self.loaded_skill_order.clone()
    }

    /// 清空本会话 skill 载入记录。
    ///
    /// 压缩后历史里的正文可能被剪掉，必须允许再次全文 load。
    ///
    /// 返回:
    /// - 无
    pub(crate) fn clear_loaded_skills(&mut self) {
        self.loaded_skills.clear();
        self.loaded_skill_order.clear();
    }

    /// 加载多个 skill 文档；已在本会话 load 过的只回 already_loaded。
    ///
    /// 参数:
    /// - `keywords`: 要加载的 skill 名称
    /// - `config`: 当前应用配置
    /// - `paths`: 应用目录路径集合
    ///
    /// 返回:
    /// - 包含名称、状态和（仅新载入时）完整文档的 JSON
    pub(super) fn load_skills(
        &mut self,
        keywords: &[String],
        config: &AppConfig,
        paths: &SaiPaths,
    ) -> Result<String> {
        if !config.skills.enabled {
            bail!("skill loading is disabled");
        }
        // 1. 先校验全部名称，避免批量请求出现部分成功
        let mut documents = Vec::with_capacity(keywords.len());
        for name in keywords {
            let content = tools::load_installed_skill(name, config, paths)?;
            documents.push((name.clone(), content));
        }

        // 2. 按请求顺序分类：新载入回全文，已载入只回状态
        let mut newly_loaded = BTreeSet::new();
        let mut already_loaded = BTreeSet::new();
        let mut skills = Vec::with_capacity(documents.len());
        for (name, content) in documents {
            if self.loaded_skills.contains(&name) {
                already_loaded.insert(name.clone());
                skills.push(json!({
                    "name": name,
                    "status": "already_loaded",
                }));
                continue;
            }
            if self.loaded_skills.insert(name.clone()) {
                self.loaded_skill_order.push(name.clone());
            }
            newly_loaded.insert(name.clone());
            skills.push(json!({
                "name": name,
                "status": "loaded",
                "content": content,
            }));
        }
        let only_already = newly_loaded.is_empty() && !already_loaded.is_empty();
        let instruction = if only_already {
            "These skills are already loaded in this session. Their documents are in earlier tool results. Do not call load for them again."
        } else {
            "Newly loaded skills include full documents. Items marked already_loaded were not re-sent; use the earlier loaded-skill document. Do not call load again for already_loaded names."
        };
        Ok(serde_json::to_string_pretty(&json!({
            "ok": true,
            "skills": skills,
            "already_loaded": only_already,
            "currently_loaded_skills": self.loaded_skill_order,
            "instruction": instruction,
        }))?)
    }
}

/// 把已加载 skill 名称格式化为上下文资源正文。
///
/// 参数:
/// - `names`: 已加载名称
///
/// 返回:
/// - 短名单；空集合返回空串
pub(crate) fn loaded_skills_resource(names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let mut lines = vec![
        "Skills already loaded in this session. Documents are in earlier tool results; do not load them again unless compaction removed those results.".to_string(),
    ];
    for name in names {
        lines.push(format!("- {name}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use crate::agent::tool_visibility::ToolVisibility;
    use crate::config::AppConfig;
    use crate::paths::SaiPaths;
    use crate::tools::ToolRegistry;
    use serde_json::{json, Value};

    fn test_paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    fn write_skill(paths: &SaiPaths, name: &str) {
        let skill_dir = paths.skills_dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: demo\n---\n\nbody of {name}\n"),
        )
        .unwrap();
    }

    #[test]
    fn second_skill_load_skips_document_and_sets_already_loaded() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        write_skill(&paths, "gpu-passthrough");
        let registry = ToolRegistry::new();
        let config = AppConfig::default();
        let mut visibility = ToolVisibility::new(Vec::new());
        let args = r#"{"type":"skill","keywords":["gpu-passthrough"]}"#;

        let first = visibility
            .load_from_arguments(&registry, args, &config, &paths)
            .unwrap();
        let first = serde_json::from_str::<Value>(&first).unwrap();
        assert_eq!(first["already_loaded"], json!(false));
        assert_eq!(first["skills"][0]["status"], json!("loaded"));
        assert!(first["skills"][0]["content"]
            .as_str()
            .unwrap()
            .contains("body of gpu-passthrough"));

        let second = visibility
            .load_from_arguments(&registry, args, &config, &paths)
            .unwrap();
        let second = serde_json::from_str::<Value>(&second).unwrap();
        assert_eq!(second["already_loaded"], json!(true));
        assert_eq!(second["skills"][0]["status"], json!("already_loaded"));
        assert!(second["skills"][0].get("content").is_none());
        assert!(second["instruction"]
            .as_str()
            .unwrap()
            .contains("Do not call load"));
    }

    #[test]
    fn clearing_loaded_skills_allows_full_reload() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        write_skill(&paths, "gpu-passthrough");
        let registry = ToolRegistry::new();
        let config = AppConfig::default();
        let mut visibility = ToolVisibility::new(Vec::new());
        let args = r#"{"type":"skill","keywords":["gpu-passthrough"]}"#;
        visibility
            .load_from_arguments(&registry, args, &config, &paths)
            .unwrap();
        visibility.clear_loaded_skills();

        let again = visibility
            .load_from_arguments(&registry, args, &config, &paths)
            .unwrap();
        let again = serde_json::from_str::<Value>(&again).unwrap();
        assert_eq!(again["already_loaded"], json!(false));
        assert!(again["skills"][0]["content"]
            .as_str()
            .unwrap()
            .contains("body of gpu-passthrough"));
    }
}
