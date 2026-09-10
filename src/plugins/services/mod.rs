mod model;
mod stream_budget;
mod tools;
mod vision;

pub(crate) use model::PluginModelSource;
pub(crate) use vision::PluginVisionSource;

use crate::llm::OpenAiCompatibleClient;
use crate::tools::ToolRegistry;
use anyhow::{bail, Result};
use async_trait::async_trait;
use sai_plugin_runtime::host::{HostTool, InvocationServices, ModelRequest, ModelResponse};
use sai_plugin_runtime::{InvocationContext, ProgressCallback};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;

tokio::task_local! {
    static CALL_CHAIN: Vec<CallFrame>;
}

static NEXT_INVOCATION: AtomicU64 = AtomicU64::new(1);

/// 【插件】【调用链】区分插件递归与同名插件的独立观察者实例，不持有祖先 VM。
#[derive(Clone)]
struct CallFrame {
    id: String,
    instance_id: u64,
}

/// 【插件】【调查上下文】独立实例承接嵌套工具和模型请求，权限仍来自父任务。
pub(crate) struct PluginServices {
    tools: ToolRegistry,
    model: Option<PluginModelSource>,
    vision: Option<PluginVisionSource>,
    client: OnceLock<OpenAiCompatibleClient>,
    progress: Option<ProgressCallback>,
    allowed: BTreeSet<String>,
    workdir: PathBuf,
    operation_id: String,
    chain: Vec<CallFrame>,
    rounds: AtomicUsize,
}

impl PluginServices {
    /// 【插件】【上下文创建】在进入 Lua 前复制插件状态，避免回调重入当前虚拟机。
    /// @param id 调用插件；tools 为当前 Agent 的工具表；model 为模型来源；allowed 为当前插件的工具授权；context 为可信调用上下文
    /// @returns 独立调用服务；递归和过深调用在获取 Lua 锁之前失败
    pub(crate) fn new(
        id: &str,
        mut tools: ToolRegistry,
        model: Option<PluginModelSource>,
        allowed: BTreeSet<String>,
        context: &InvocationContext,
    ) -> Result<Self> {
        Self::check_invocation(id)?;
        let mut chain = CALL_CHAIN.try_with(Clone::clone).unwrap_or_default();
        chain.push(CallFrame {
            id: id.to_string(),
            instance_id: tools.plugin_instance_id(id)?,
        });
        let serial = NEXT_INVOCATION.fetch_add(1, Ordering::Relaxed);
        tools.start_plugin_session(&format!("{}/plugin/{id}/{serial}", context.session_id))?;
        tools.inherit_plugin_storage_session(&context.storage_session_id);
        Ok(Self {
            tools,
            model,
            vision: None,
            client: OnceLock::new(),
            progress: context.progress.clone(),
            allowed,
            workdir: PathBuf::from(&context.workdir),
            operation_id: context.operation_id.clone(),
            chain,
            rounds: AtomicUsize::new(0),
        })
    }

    /// 【插件】【递归检查】在工具检查和结果事件之前拒绝祖先插件，避免旧注册表先等待活动 VM。
    /// @param id 即将调用的插件标识
    /// @returns 非递归且未超过调用深度时成功
    pub(crate) fn check_invocation(id: &str) -> Result<()> {
        let chain = CALL_CHAIN.try_with(Clone::clone).unwrap_or_default();
        if chain.iter().any(|ancestor| ancestor.id == id) {
            bail!("recursive plugin invocation is not allowed: {id}");
        }
        if chain.len() >= 8 {
            bail!("plugin invocation depth exceeds 8");
        }
        Ok(())
    }

    /// 【插件】【活动实例】向注册表提供需要隔离的祖先实例标识。
    /// @returns 当前调用链的 VM 标识，不暴露给 Lua 或持有实例引用
    pub(crate) fn active_instances() -> Vec<u64> {
        CALL_CHAIN
            .try_with(|chain| chain.iter().map(|frame| frame.instance_id).collect())
            .unwrap_or_default()
    }

    /// 【插件】【调查事件】取得独立调查会话的生命周期分发器。
    /// @returns 使用独立 Lua 实例的分发器
    pub(crate) fn events(&self) -> super::PluginEvents {
        self.tools.plugin_events()
    }
}

#[async_trait]
impl InvocationServices for PluginServices {
    /// 【插件视觉】【配置查询】返回宿主视觉配置的公开标识，不初始化模型客户端。
    /// @returns 已启用且配置有效时的模型标识
    fn vision_info(&self) -> Result<Option<sai_plugin_runtime::host::VisionModelInfo>> {
        self.vision
            .as_ref()
            .map(PluginVisionSource::info)
            .transpose()
            .map(Option::flatten)
    }

    /// 【插件视觉】【图片请求】使用独立授权的视觉模型分析当前缓冲。
    /// @param request 提示词；image 为本次图片字节；max_bytes 为文字大小上限
    /// @returns 模型正文和可信模型标识
    async fn analyze_image(
        &self,
        request: sai_plugin_runtime::host::VisionRequest,
        image: sai_plugin_runtime::host::BinaryData,
        max_bytes: usize,
    ) -> Result<sai_plugin_runtime::host::VisionResponse> {
        self.complete_vision(request, image, max_bytes).await
    }

    /// 【插件】【工具目录】只公开当前调用授权并且不会递归进入祖先插件的工具。
    /// @returns 与模型请求和实际执行共用的目录
    fn tools(&self) -> Result<Vec<HostTool>> {
        self.tool_catalog()
    }

    /// 【插件】【工具调用】通过原有工具权限和事件入口执行，保留任务目录和调用链。
    /// @param name 精确工具名称；arguments 为 JSON 参数文本
    /// @returns 工具文本结果或权限、执行错误
    async fn call_tool(&self, name: &str, arguments: &str) -> Result<String> {
        self.execute_tool(name, arguments).await
    }

    /// 【插件】【模型请求】单次请求使用当前客户端，调查流程由 Lua 决定。
    /// @param request 消息和工具名称；max_bytes 为输出字节上限
    /// @returns 单次模型响应，不自动执行工具
    async fn complete(&self, request: ModelRequest, max_bytes: usize) -> Result<ModelResponse> {
        self.complete_model(request, max_bytes).await
    }
}
