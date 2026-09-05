use super::*;
use crate::config::AgentProfile;

/// 构造可委派档案；参数为标识和注册状态，返回独立档案。
fn profile(id: &str, registered: bool) -> AgentProfile {
    AgentProfile {
        id: id.to_string(),
        name: id.to_string(),
        register_to_main: registered,
        ..AgentProfile::default()
    }
}

/// 共享设置始终排在第一项，列表只包含可以委派的类型。
#[test]
fn list_contains_shared_defaults_and_registered_profiles() {
    let mut config = AppConfig::default();
    config.agents.push(profile("worker", true));
    config.agents.push(profile("main-only", false));
    let state = SubagentListState::new(&config, None);
    assert!(state.selected().profile_id.is_none());
    assert!(state
        .targets
        .iter()
        .any(|target| target.profile_id.as_deref() == Some("worker")));
    assert!(!state
        .targets
        .iter()
        .any(|target| target.profile_id.as_deref() == Some("main-only")));
}

/// 列表恢复上次类型，未知类型回到共享设置；移动不会越界。
#[test]
fn list_restores_selection_and_clamps_navigation() {
    let config = AppConfig::default();
    let state = SubagentListState::new(&config, Some("explore"));
    assert_eq!(state.selected().profile_id.as_deref(), Some("explore"));
    let mut state = SubagentListState::new(&config, Some("missing"));
    state.move_up();
    assert_eq!(state.index, 0);
    for _ in 0..state.targets.len() + 3 {
        state.move_down();
    }
    assert_eq!(state.index, state.targets.len() - 1);
}

/// 共享模型可以继承主对话，单类型选择器定位到已保存的模型和思考。
#[test]
fn choice_state_restores_independent_model_and_thinking() {
    let mut config = AppConfig::default();
    let shared = SubagentListState::new(&config, None);
    let picker = choice_state(&config, shared.selected());
    assert!(picker.selected_model().unwrap().provider_id.is_empty());
    let provider = config.providers[0].clone();
    config
        .set_subagent_model_choice(
            Some("explore"),
            SubagentModelChoice {
                provider_id: provider.id.clone(),
                model: provider.default_model.clone(),
                thinking_level: "high".to_string(),
            },
        )
        .unwrap();
    let state = SubagentListState::new(&config, Some("explore"));
    let picker = choice_state(&config, state.selected());
    assert_eq!(
        picker.selected_model().unwrap().model,
        provider.default_model
    );
    assert_eq!(picker.selected_level(), "high");
}
