use base64::Engine;

include!(concat!(env!("OUT_DIR"), "/default_sai_prompt.rs"));

pub const YOLO_REMINDER: &str = include_str!("prompts/yolo.md");
pub const AUDITED_REMINDER: &str = include_str!("prompts/audited.md");
pub const PLAN_REMINDER: &str = include_str!("prompts/plan.md");
pub const MEME_DESCRIPTION_PROMPT: &str = include_str!("prompts/meme-description.md");
pub const INPUT_METHOD_DIAGNOSIS_PROMPT: &str =
    include_str!("prompts/linux-input-method-diagnose.md");
pub const GAME_COMPATIBILITY_PROMPT: &str = include_str!("prompts/linux-game-compatibility.md");

pub fn default_system_prompt() -> String {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(OBFUSCATED_DEFAULT_SYSTEM_PROMPT)
        .expect("embedded default prompt must be valid base64");
    let decoded = bytes
        .into_iter()
        .enumerate()
        .map(|(index, byte)| byte ^ PROMPT_MASK[index % PROMPT_MASK.len()])
        .collect::<Vec<_>>();
    String::from_utf8(decoded).expect("embedded default prompt must be valid utf-8")
}

#[cfg(test)]
mod tests {
    use super::default_system_prompt;

    /// 默认系统提示不应再写死人物关系或工具清单；工具说明交给 load 与配置过滤后的注册表。
    #[test]
    fn default_system_prompt_omits_shorin_and_tool_catalog() {
        let prompt = default_system_prompt();
        assert!(!prompt.to_lowercase().contains("shorin"));
        assert!(!prompt.contains("review_aur_package"));
        assert!(!prompt.contains("install_aur_package"));
        assert!(!prompt.contains("inspect_issue"));
        assert!(!prompt.contains("deep_research"));
        assert!(!prompt.contains("protondb_query"));
        assert!(!prompt.contains("linux_input_method_diagnose"));
        assert!(!prompt.contains("remember_fact"));
        assert!(!prompt.contains("draw_zhouyi_hexagram"));
        assert!(!prompt.contains("Available groups"));
        assert!(prompt.contains("Sai"));
    }
}
