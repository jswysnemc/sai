use super::capabilities::AcpCapabilities;
use agent_client_protocol::schema::v1::{AuthMethod, SessionConfigOption, SessionModeState};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// ACP 子进程从启动到断开的连接阶段。
#[repr(u8)]
enum ConnectionPhase {
    Pending = 0,
    Active = 1,
    Disconnected = 2,
}

/// 外部 ACP 内核最近一次握手和会话协商结果。
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct AcpRuntimeState {
    pub(crate) connected: bool,
    #[serde(skip)]
    active_connections: usize,
    pub(crate) agent_name: String,
    pub(crate) agent_version: String,
    pub(crate) capabilities: AcpCapabilities,
    pub(crate) auth_methods: Value,
    pub(crate) config_options: Value,
    pub(crate) modes: Value,
    pub(crate) available_commands: Value,
    pub(crate) native_equivalents: Value,
}

/// 单个 ACP 子进程对应的连接状态追踪器。
///
/// 同一内核可能同时存在多个会话，因此每个传输实例独立追踪一次连接；
/// 标准输出关闭、显式关闭和析构都可以安全调用 `disconnect`，计数只会减少一次。
#[derive(Clone)]
pub(crate) struct AcpConnectionTracker {
    inner: Arc<AcpConnectionTrackerInner>,
}

struct AcpConnectionTrackerInner {
    engine: String,
    status: AtomicU8,
}

impl AcpConnectionTracker {
    /// 创建尚未完成握手的连接追踪器。
    ///
    /// 参数:
    /// - `engine`: 内核稳定名称
    ///
    /// 返回:
    /// - 待握手的连接追踪器
    pub(crate) fn new(engine: &str) -> Self {
        Self {
            inner: Arc::new(AcpConnectionTrackerInner {
                engine: engine.to_string(),
                status: AtomicU8::new(ConnectionPhase::Pending as u8),
            }),
        }
    }

    /// 标记当前子进程已经断开。
    ///
    /// 多条生命周期路径可以重复调用该方法，活动连接计数只会减少一次。
    ///
    /// 返回:
    /// - 无
    pub(crate) fn disconnect(&self) {
        let mut states = states().lock().unwrap();
        if self
            .inner
            .status
            .swap(ConnectionPhase::Disconnected as u8, Ordering::SeqCst)
            != ConnectionPhase::Active as u8
        {
            return;
        }
        mark_state_disconnected(states.get_mut(&self.inner.engine));
    }

    /// 将完成握手的连接加入运行状态计数。
    ///
    /// 参数:
    /// - `state`: 当前内核的共享运行状态
    ///
    /// 返回:
    /// - 无
    fn activate(&self, state: &mut AcpRuntimeState) {
        if self
            .inner
            .status
            .compare_exchange(
                ConnectionPhase::Pending as u8,
                ConnectionPhase::Active as u8,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok()
        {
            state.active_connections = state.active_connections.saturating_add(1);
        }
        state.connected = state.active_connections > 0;
    }

    /// 返回追踪器所属的内核稳定名称。
    ///
    /// 返回:
    /// - 内核稳定名称
    fn engine(&self) -> &str {
        &self.inner.engine
    }
}

impl Drop for AcpConnectionTrackerInner {
    /// 在所有连接追踪句柄都被丢弃时补齐断开状态。
    fn drop(&mut self) {
        let status = self.status.get_mut();
        if *status != ConnectionPhase::Active as u8 {
            return;
        }
        *status = ConnectionPhase::Disconnected as u8;
        if let Ok(mut states) = states().lock() {
            mark_state_disconnected(states.get_mut(&self.engine));
        }
    }
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
/// - `connection`: 当前子进程的连接状态追踪器
/// - `agent_name`: 握手返回的 agent 展示名称
/// - `agent_version`: 握手返回的 agent 版本
/// - `capabilities`: agent 声明的能力
/// - `auth_methods`: agent 声明的认证方式
/// - `native_equivalents`: agent 声明的原生等价能力
///
/// 返回:
/// - 无
pub(crate) fn publish_handshake(
    connection: &AcpConnectionTracker,
    agent_name: &str,
    agent_version: &str,
    capabilities: &AcpCapabilities,
    auth_methods: &[AuthMethod],
    native_equivalents: &Value,
) {
    let mut states = states().lock().unwrap();
    let state = states.entry(connection.engine().to_string()).or_default();
    connection.activate(state);
    state.agent_name = agent_name.to_string();
    state.agent_version = agent_version.to_string();
    state.capabilities = capabilities.clone();
    state.auth_methods = serialize(auth_methods);
    state.native_equivalents = native_equivalents.clone();
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
            object.insert(
                "currentModeId".to_string(),
                Value::String(mode_id.to_string()),
            );
        }
        None => {
            state.modes = serde_json::json!({ "currentModeId": mode_id });
        }
    }
}

/// 更新 agent 通过通知公布的斜杠命令。
///
/// 参数:
/// - `engine`: 内核稳定名称
/// - `commands`: 最新完整命令数组
///
/// 返回:
/// - 无
pub(crate) fn update_available_commands(engine: &str, commands: &Value) {
    let mut states = states().lock().unwrap();
    states
        .entry(engine.to_string())
        .or_default()
        .available_commands = commands.clone();
}

/// 减少指定运行状态的活动连接计数，同时保留最近一次握手快照。
///
/// 参数:
/// - `state`: 待更新的运行状态
///
/// 返回:
/// - 无
fn mark_state_disconnected(state: Option<&mut AcpRuntimeState>) {
    if let Some(state) = state {
        state.active_connections = state.active_connections.saturating_sub(1);
        state.connected = state.active_connections > 0;
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
        let first_connection = AcpConnectionTracker::new(engine);
        let capabilities = AcpCapabilities {
            prompt_image: true,
            ..Default::default()
        };
        publish_handshake(
            &first_connection,
            "Test Agent",
            "1.0.0",
            &capabilities,
            &[AuthMethod::Agent(AuthMethodAgent::new("login", "Login"))],
            &serde_json::json!({ "subagents": "test-agent" }),
        );
        let option = SessionConfigOption::select(
            "model",
            "Model",
            "sonnet",
            vec![
                agent_client_protocol::schema::v1::SessionConfigSelectOption::new(
                    "sonnet", "Sonnet",
                ),
            ],
        )
        .category(SessionConfigOptionCategory::Model);
        publish_session(engine, &[option], None);

        let state = current(engine).unwrap();
        assert!(state.connected);
        assert!(state.capabilities.prompt_image);
        assert_eq!(state.agent_name, "Test Agent");
        assert_eq!(state.agent_version, "1.0.0");
        assert_eq!(state.auth_methods[0]["id"], "login");
        assert_eq!(state.native_equivalents["subagents"], "test-agent");
        assert_eq!(state.config_options[0]["category"], "model");

        update_available_commands(
            engine,
            &serde_json::json!([{ "name": "compact", "description": "Compact context" }]),
        );
        assert_eq!(
            current(engine).unwrap().available_commands[0]["name"],
            "compact"
        );

        let second_connection = AcpConnectionTracker::new(engine);
        publish_handshake(
            &second_connection,
            "Test Agent",
            "1.0.0",
            &capabilities,
            &[],
            &Value::Null,
        );
        first_connection.disconnect();
        assert!(current(engine).unwrap().connected);
        first_connection.disconnect();
        assert!(current(engine).unwrap().connected);
        second_connection.disconnect();
        let disconnected = current(engine).unwrap();
        assert!(!disconnected.connected);
        assert_eq!(disconnected.agent_name, "Test Agent");
    }

    /// 握手完成前已经关闭的进程不能留下虚假的连接状态。
    #[test]
    fn ignores_handshake_activation_after_early_disconnect() {
        let engine = "runtime-state-early-disconnect-test";
        let connection = AcpConnectionTracker::new(engine);
        connection.disconnect();

        publish_handshake(
            &connection,
            "Closed Agent",
            "1.0.0",
            &AcpCapabilities::default(),
            &[],
            &Value::Null,
        );

        let state = current(engine).unwrap();
        assert!(!state.connected);
        assert_eq!(state.agent_name, "Closed Agent");
    }
}
