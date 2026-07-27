use super::capabilities::AcpCapabilities;
use agent_client_protocol::schema::v1::{AuthMethod, SessionConfigOption, SessionModeState};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// 外部 ACP 内核最近一次握手和会话协商结果。
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct AcpRuntimeState {
    pub(crate) capabilities: AcpCapabilities,
    pub(crate) auth_methods: Value,
    pub(crate) config_options: Value,
    pub(crate) modes: Value,
}

/// 返回进程内 ACP 运行状态表。
///
/// 返回:
/// - 以内核稳定名称为键的共享状态表
fn states() -> &'static Mutex<HashMap<String, AcpRuntimeState>> {
    static STATES: OnceLock<Mutex<HashMap<String, AcpRuntimeState>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 发布握手阶段得到的能力与认证方式。
///
/// 参数:
/// - `engine`: 内核稳定名称
/// - `capabilities`: agent 声明的能力
/// - `auth_methods`: agent 声明的认证方式
///
/// 返回:
/// - 无
pub(crate) fn publish_handshake(
    engine: &str,
    capabilities: &AcpCapabilities,
    auth_methods: &[AuthMethod],
) {
    let mut states = states().lock().unwrap();
    let state = states.entry(engine.to_string()).or_default();
    state.capabilities = capabilities.clone();
    state.auth_methods = serialize(auth_methods);
}

/// 发布会话创建或恢复后得到的配置与旧版模式。
///
/// 参数:
/// - `engine`: 内核稳定名称
/// - `config_options`: 标准配置项
/// - `modes`: 旧版会话模式
///
/// 返回:
/// - 无
pub(crate) fn publish_session(
    engine: &str,
    config_options: &[SessionConfigOption],
    modes: Option<&SessionModeState>,
) {
    let mut states = states().lock().unwrap();
    let state = states.entry(engine.to_string()).or_default();
    state.config_options = serialize(config_options);
    if let Some(modes) = modes {
        state.modes = serialize(modes);
    }
}

/// 更新 agent 通过通知推送的完整配置项。
///
/// 参数:
/// - `engine`: 内核稳定名称
/// - `config_options`: 最新完整配置项
///
/// 返回:
/// - 无
pub(crate) fn update_config_options(engine: &str, config_options: &[SessionConfigOption]) {
    let mut states = states().lock().unwrap();
    states.entry(engine.to_string()).or_default().config_options = serialize(config_options);
}

/// 更新旧版 session mode 的当前值。
///
/// 参数:
/// - `engine`: 内核稳定名称
/// - `mode_id`: agent 推送的当前模式标识
///
/// 返回:
/// - 无
pub(crate) fn update_current_mode(engine: &str, mode_id: &str) {
    let mut states = states().lock().unwrap();
    let state = states.entry(engine.to_string()).or_default();
    let object = state.modes.as_object_mut();
    match object {
        Some(object) => {
            object.insert("currentModeId".to_string(), Value::String(mode_id.to_string()));
        }
        None => {
            state.modes = serde_json::json!({ "currentModeId": mode_id });
        }
    }
}

/// 查询指定内核最近一次运行状态。
///
/// 参数:
/// - `engine`: 内核稳定名称
///
/// 返回:
/// - 已完成握手时返回状态快照
pub(crate) fn current(engine: &str) -> Option<AcpRuntimeState> {
    states().lock().unwrap().get(engine).cloned()
}

/// 将 SDK 类型转换成可直接返回给前端的 JSON。
///
/// 参数:
/// - `value`: 可序列化协议值
///
/// 返回:
/// - 序列化结果；失败时返回 null
fn serialize<T: Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::v1::{
        AuthMethodAgent, SessionConfigOption, SessionConfigOptionCategory,
    };

    /// 运行状态同时保留握手能力和动态配置项。
    #[test]
    fn combines_handshake_and_session_state() {
        let engine = "runtime-state-test";
        let capabilities = AcpCapabilities {
            prompt_image: true,
            ..Default::default()
        };
        publish_handshake(
            engine,
            &capabilities,
            &[AuthMethod::Agent(AuthMethodAgent::new("login", "Login"))],
        );
        let option = SessionConfigOption::select(
            "model",
            "Model",
            "sonnet",
            vec![agent_client_protocol::schema::v1::SessionConfigSelectOption::new(
                "sonnet", "Sonnet",
            )],
        )
        .category(SessionConfigOptionCategory::Model);
        publish_session(engine, &[option], None);

        let state = current(engine).unwrap();
        assert!(state.capabilities.prompt_image);
        assert_eq!(state.auth_methods[0]["id"], "login");
        assert_eq!(state.config_options[0]["category"], "model");
    }
}
