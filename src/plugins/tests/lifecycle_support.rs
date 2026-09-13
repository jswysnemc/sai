use super::services_support::ModelFixture;
use crate::agent::{Agent, AgentMode};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use crate::plugins::discovery::{find, PluginSource};
use crate::plugins::private::PrivatePluginHost;
use crate::plugins::registry::register_descriptor;
use crate::plugins::{self, GrantUpdate};
use crate::state::StateStore;
use crate::tools::{ToolRegistry, ToolSpec};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

pub(super) const ID: &str = "lifecycle-observer";
pub(super) const PROBE: &str = "lifecycle_probe";

/// 【生命周期验收】【独立环境】持有真实外部安装、应用路径及原生工具执行记录
pub(super) struct LifecycleFixture {
    pub root: tempfile::TempDir,
    pub paths: SaiPaths,
    pub config: AppConfig,
    pub probes: Arc<Mutex<Vec<Value>>>,
}

impl LifecycleFixture {
    /// 【生命周期验收】【外部安装】把命令与监听器包安装到隔离目录并显式授权
    /// @param model 本地模型服务，用于生成独立供应商配置
    /// @returns 使用正式安装和授权入口的环境，不读取用户配置
    pub fn new(model: &ModelFixture) -> Self {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(&root.path().join("app"));
        let mut config = model.config("lifecycle-main");
        config.agent_runtime = Some(crate::config::AgentRuntimeOverride {
            enabled_tools: vec![PROBE.into()],
            exclusive: true,
            ..Default::default()
        });
        let source = root.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let manifest = json!({
            "api_version":1, "id":ID, "version":"1.0.0", "name":"Lifecycle observer",
            "description":"Observe public lifecycle contracts", "entry":"init.lua",
            "capabilities":{"model":true,"tools":[PROBE],
                "system":{"session_storage":true,"plugin_storage":true}}
        });
        std::fs::write(source.join("sai-plugin.json"), manifest.to_string()).unwrap();
        std::fs::write(
            source.join("init.lua"),
            include_str!("fixtures/lifecycle_observer.lua"),
        )
        .unwrap();
        plugins::install(&source, &paths, false).unwrap();
        plugins::set_enabled(&config, &paths, ID, true, GrantUpdate::Declared).unwrap();
        Self {
            root,
            paths,
            config,
            probes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 【生命周期验收】【正式宿主】加载已安装包，仅增加一个可观察的原生工具
    /// @returns 带真实私有存储宿主的工具表，插件自身没有模型工具
    pub fn registry(&self) -> ToolRegistry {
        let descriptor = find(&self.config, &self.paths, ID).unwrap();
        assert!(matches!(descriptor.source, PluginSource::Installed(_)));

        let effective = descriptor.capabilities().intersection(&descriptor.grants());
        assert!(effective.model && effective.tools.contains(PROBE));
        assert!(effective.system.session_storage && effective.system.plugin_storage);
        let host = PrivatePluginHost::for_descriptor(&self.paths, &descriptor).unwrap();
        let mut registry = ToolRegistry::new();
        registry.configure_plugin_model(&self.config, &self.paths);
        register_descriptor(&mut registry, descriptor, Arc::new(host), false).unwrap();
        assert!(registry.definitions().is_empty());
        let probes = self.probes.clone();
        registry.register(ToolSpec::new(
            PROBE,
            "Read lifecycle fixture arguments.",
            json!({"type":"object","properties":{"value":{"type":"string"},
                "deny":{"type":"boolean"}},"required":["value"]}),
            move |arguments| {
                probes.lock().unwrap().push(arguments.clone());
                async move { Ok(format!("probe:{}", arguments["value"].as_str().unwrap())) }
            },
        ));
        registry
    }

    /// 【生命周期验收】【真实主请求】构造完整 Agent，独占白名单只公开原生探针
    /// @param model 本地模型服务
    /// @returns 已初始化状态存储并使用正式客户端的 Agent
    pub fn agent(&self, model: &ModelFixture) -> Agent {
        let state = StateStore::new(&self.paths).unwrap();
        state.init_files().unwrap();
        Agent::new(
            self.config.clone(),
            &self.paths,
            state,
            model.client("lifecycle-main", &self.paths),
            self.registry(),
            AgentMode::Yolo,
        )
        .unwrap()
    }

    /// 【生命周期验收】【主实例查询】通过正式命令适配读取 Agent 当前插件的观察结果
    /// @param agent 已经执行或取消请求的 Agent
    /// @returns 同一实例的事件及授权检查结果
    pub async fn report_agent(&self, agent: &Agent) -> Value {
        let (registry, name) = agent.plugin_command_registry(ID, "report").unwrap();
        let text = crate::runtime_cwd::scope(
            self.root.path().to_path_buf(),
            registry.call(&name, r#"{"arguments":""}"#),
        )
        .await
        .unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// 【生命周期验收】【子实例查询】复用直接用户命令入口读取指定注册表的实例
    /// @param registry 保留子任务或父任务实例的注册表
    /// @returns 对应实例的事件及授权检查结果
    pub async fn report_registry(&self, registry: &ToolRegistry) -> Value {
        let mut registry = registry.clone();
        let command = registry.plugin_command(ID, "report").unwrap();
        let name = command.name.clone();
        registry.register(command);
        let text = crate::runtime_cwd::scope(
            self.root.path().to_path_buf(),
            registry.call(&name, r#"{"arguments":""}"#),
        )
        .await
        .unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// 【生命周期验收】【宿主事实】检查事件只读、调用服务隔离、真实会话和工作目录
    /// @param report 命令快照；session 为宿主创建的会话标识
    /// @returns 无，任何越权、陈旧回调或未知事件字段均使测试失败
    pub fn assert_contexts(&self, report: &Value, session: &str) {
        assert_eq!(report["old_progress_allowed"], false);
        assert_eq!(report["session_record"], Value::Null);
        assert_eq!(report["plugin_record"], Value::Null);
        for row in report["rows"].as_array().unwrap() {
            assert_eq!(row["session_id"], session);
            assert_eq!(row["workdir"], self.root.path().to_string_lossy().as_ref());
            assert!(!row["operation_id"].as_str().unwrap().is_empty());
            for key in [
                "allow_writes",
                "model_allowed",
                "tools_allowed",
                "session_write_allowed",
                "plugin_write_allowed",
            ] {
                assert_eq!(row[key], false, "event allowed {key}: {row}");
            }
            let allowed = match row["kind"].as_str().unwrap() {
                "agent_start" => &["turn_id", "kind"][..],
                "agent_end" => &["turn_id", "kind", "ok"][..],
                "turn_start" | "message_start" => &["turn_id", "kind", "round"][..],
                "turn_end" | "message_end" => &["turn_id", "kind", "round", "ok"][..],
                "tool_call" => &["name", "arguments"][..],
                "tool_result" => &["name", "ok", "output"][..],
                kind => panic!("unexpected lifecycle event: {kind}"),
            };
            assert!(row["data"]
                .as_object()
                .unwrap()
                .keys()
                .all(|key| allowed.contains(&key.as_str())));
        }
        assert!(!report.to_string().contains("fixture-only"));
    }
}

/// 【生命周期验收】【事件顺序】从查询结果提取事件名称，供完整流程断言使用
/// @param report 命令返回的观察快照
/// @returns 按实际接收顺序排列的事件名称
pub(super) fn kinds(report: &Value) -> Vec<&str> {
    report["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["kind"].as_str().unwrap())
        .collect()
}
