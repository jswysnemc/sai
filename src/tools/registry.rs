use super::descriptions::tool_description;
use crate::llm::{FunctionDefinition, ToolDefinition};
use crate::permission::PermissionProfile;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;

pub type ToolFuture = Pin<Box<dyn Future<Output = Result<ToolOutput>> + Send>>;
pub type ToolHandler = Arc<dyn Fn(Value, ToolProgress) -> ToolFuture + Send + Sync>;

/// Provider-facing dsh bash is executed by run_command with a trusted shell override.
pub(crate) const DSH_BASH_EXECUTION_ALIAS: &str = "__sai_dsh_bash";

/// 工具希望在下一次模型请求中附加的图片。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolModelAttachment {
    pub(crate) image_url: String,
    pub(crate) source: String,
    pub(crate) prompt: String,
}

impl ToolModelAttachment {
    /// 创建模型图片附件。
    ///
    /// 参数:
    /// - `image_url`: 图片 data URL 或远程 URL
    /// - `source`: 图片来源路径或标识
    /// - `prompt`: 当前模型分析图片时使用的提示
    ///
    /// 返回:
    /// - 模型图片附件
    pub(crate) fn new(
        image_url: impl Into<String>,
        source: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            image_url: image_url.into(),
            source: source.into(),
            prompt: prompt.into(),
        }
    }
}

/// 工具文本结果和仅供下一次模型请求使用的附件。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolOutput {
    pub(crate) content: String,
    pub(crate) model_attachments: Vec<ToolModelAttachment>,
}

impl ToolOutput {
    /// 创建不包含模型附件的普通工具结果。
    ///
    /// 参数:
    /// - `content`: 工具文本结果
    ///
    /// 返回:
    /// - 普通工具结果
    pub(crate) fn text(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            model_attachments: Vec::new(),
        }
    }

    /// 为工具结果附加下一次模型请求使用的图片。
    ///
    /// 参数:
    /// - `attachments`: 图片附件列表
    ///
    /// 返回:
    /// - 包含模型图片附件的工具结果
    pub(crate) fn with_model_attachments(
        mut self,
        attachments: impl IntoIterator<Item = ToolModelAttachment>,
    ) -> Self {
        self.model_attachments.extend(attachments);
        self
    }
}

#[derive(Clone, Default)]
pub struct ToolProgress {
    sender: Option<mpsc::UnboundedSender<String>>,
}

impl ToolProgress {
    pub fn new(sender: mpsc::UnboundedSender<String>) -> Self {
        Self {
            sender: Some(sender),
        }
    }

    pub fn report(&self, message: impl Into<String>) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(message.into());
        }
    }
}

#[derive(Clone)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub permission: ToolPermission,
    handler: ToolHandler,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolPermission {
    ReadOnly,
    Writes,
}

#[derive(Clone, Debug)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub permission: ToolPermission,
}

impl ToolSpec {
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        let name = name.into();
        let fallback_description = description.into();
        let description = tool_description(&name, &fallback_description);
        Self {
            name,
            description,
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: Arc::new(move |args, _progress| {
                let future = handler(args);
                Box::pin(async move { future.await.map(ToolOutput::text) })
            }),
        }
    }

    /// 创建可以返回下一次模型请求附件的工具。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `description`: 工具说明
    /// - `parameters`: JSON Schema 参数定义
    /// - `handler`: 返回结构化工具结果的异步处理函数
    ///
    /// 返回:
    /// - 工具定义
    pub(crate) fn new_with_output<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolOutput>> + Send + 'static,
    {
        let name = name.into();
        let fallback_description = description.into();
        let description = tool_description(&name, &fallback_description);
        Self {
            name,
            description,
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: Arc::new(move |args, _progress| Box::pin(handler(args))),
        }
    }

    pub fn new_with_progress<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value, ToolProgress) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String>> + Send + 'static,
    {
        let name = name.into();
        let fallback_description = description.into();
        let description = tool_description(&name, &fallback_description);
        Self {
            name,
            description,
            parameters,
            permission: ToolPermission::ReadOnly,
            handler: Arc::new(move |args, progress| {
                let future = handler(args, progress);
                Box::pin(async move { future.await.map(ToolOutput::text) })
            }),
        }
    }

    pub fn writes(mut self) -> Self {
        self.permission = ToolPermission::Writes;
        self
    }

    pub fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function",
            function: FunctionDefinition {
                name: self.name.clone(),
                description: self.description.clone(),
                parameters: self.parameters.clone(),
            },
        }
    }

    async fn call(&self, args: Value, progress: ToolProgress) -> Result<ToolOutput> {
        (self.handler)(args, progress).await
    }
}

#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolSpec>,
    /// 工具注册顺序；请求里的 tools 数组按它输出，保证前缀稳定以命中供应商缓存
    order: Vec<String>,
    permission_profile: Option<PermissionProfile>,
    /// 当前会话状态目录（会话唯一身份），供网格目标归属判定
    session_key: String,
    /// 当前会话 id
    session_id: String,
    /// 是否允许网格工具跨越会话边界（`mesh.cross_session`）
    mesh_cross_session: bool,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: ToolSpec) {
        if !self.tools.contains_key(&tool.name) {
            self.order.push(tool.name.clone());
        }
        self.tools.insert(tool.name.clone(), tool);
    }

    /// 绑定当前会话使用的权限配置。
    ///
    /// 参数:
    /// - `profile`: 权限模式、工作区和审计日志
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_permission_profile(&mut self, profile: PermissionProfile) {
        self.permission_profile = Some(self.apply_session_ownership(profile));
    }

    /// 绑定当前会话身份与跨会话开关。
    ///
    /// 网格工具的归属判定依赖会话身份，因此注册交互式工具时必须先绑定；
    /// 权限配置可以在这之前或之后挂上，两种顺序都要生效。
    ///
    /// 参数:
    /// - `session_key`: 当前会话状态目录
    /// - `session_id`: 当前会话 id
    /// - `cross_session`: 是否允许跨会话投递
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_session_ownership(
        &mut self,
        session_key: String,
        session_id: String,
        cross_session: bool,
    ) {
        self.session_key = session_key;
        self.session_id = session_id;
        self.mesh_cross_session = cross_session;
        if let Some(profile) = self.permission_profile.take() {
            self.permission_profile = Some(self.apply_session_ownership(profile));
        }
    }

    /// 把注册表持有的会话身份与跨会话开关写进权限配置。
    ///
    /// 参数:
    /// - `profile`: 待补充的权限配置
    ///
    /// 返回:
    /// - 补充后的权限配置
    fn apply_session_ownership(&self, profile: PermissionProfile) -> PermissionProfile {
        profile
            .with_session(&self.session_key, &self.session_id)
            .with_cross_session(self.mesh_cross_session)
    }

    /// 返回当前权限配置的副本。
    ///
    /// 外部对话内核需要用同一份配置校验它自己发起的文件写入与命令执行，
    /// 否则治理只覆盖 sai 自带工具，换内核后就出现缺口。
    ///
    /// 返回:
    /// - 权限配置；YOLO 等未绑定审计的场景为 None
    pub(crate) fn permission_profile(&self) -> Option<PermissionProfile> {
        self.permission_profile.clone()
    }

    /// 立即更新权限模式（热切换，无需重建注册表）。
    ///
    /// 参数:
    /// - `mode`: 新权限模式
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_permission_mode(&self, mode: crate::permission::PermissionProfileMode) {
        if let Some(profile) = &self.permission_profile {
            profile.set_mode(mode);
        }
    }

    /// 返回权限模式原子句柄（运行中热切换）。
    pub(crate) fn permission_mode_handle(
        &self,
    ) -> Option<std::sync::Arc<std::sync::atomic::AtomicU8>> {
        self.permission_profile
            .as_ref()
            .map(|profile| profile.mode_handle())
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.ordered_tools().map(ToolSpec::definition).collect()
    }

    pub fn definitions_for_names(&self, names: &BTreeSet<String>) -> Vec<ToolDefinition> {
        // 1. 按注册顺序输出，保持过滤前后工具定义的相对顺序
        self.ordered_tools()
            .filter(|tool| names.contains(&tool.name))
            .map(ToolSpec::definition)
            .collect()
    }

    /// 返回指定工具的供应商定义。
    ///
    /// 参数:
    /// - `name`: 本地工具名称
    ///
    /// 返回:
    /// - 工具存在时返回完整定义，否则返回 None
    pub(crate) fn definition(&self, name: &str) -> Option<ToolDefinition> {
        let tool = self.tools.get(local_tool_name(name))?;
        Some(tool.definition())
    }

    pub fn definitions_except(&self, excluded: &[&str]) -> Vec<ToolDefinition> {
        self.ordered_tools()
            .filter(|tool| !excluded.iter().any(|name| *name == tool.name))
            .map(ToolSpec::definition)
            .collect()
    }

    /// 按注册顺序遍历工具。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 注册顺序的工具引用迭代器；顺序表缺失条目时跳过
    fn ordered_tools(&self) -> impl Iterator<Item = &ToolSpec> {
        self.order.iter().filter_map(|name| self.tools.get(name))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 判断模型给出的工具名能否解析到已注册工具。
    ///
    /// 与 `contains` 的区别是先还原协议别名。
    ///
    /// 参数:
    /// - `name`: 模型给出的工具名
    ///
    /// 返回:
    /// - 能否解析到已注册工具
    pub(crate) fn resolves(&self, name: &str) -> bool {
        self.tools.contains_key(local_tool_name(name))
    }

    /// 校验工具参数是否为可解析的 JSON 对象。
    ///
    /// 流式返回被截断或模型拼错括号时参数无法解析，此处提前判定，
    /// 让调用方把错误回传给模型而不是中断整轮。
    ///
    /// 参数:
    /// - `arguments`: 原始参数文本，空字符串按空对象处理
    ///
    /// 返回:
    /// - 参数合法时为 Ok，否则为可读的解析失败原因
    pub(crate) fn check_arguments(&self, arguments: &str) -> Result<()> {
        let parsed = parse_arguments(arguments)?;
        if !parsed.is_object() {
            bail!(
                "arguments must be a JSON object, got {}",
                value_kind(&parsed)
            )
        }
        Ok(())
    }

    /// 按工具注册时的 JSON Schema 校验参数。
    ///
    /// 参数:
    /// - `name`: 本地工具名称
    /// - `arguments`: 原始 JSON 参数
    ///
    /// 返回:
    /// - 参数满足真实工具契约时成功，否则返回具体校验错误
    pub(crate) fn validate_arguments(&self, name: &str, arguments: &str) -> Result<()> {
        let name = local_tool_name(name);
        let tool = self
            .tools
            .get(name)
            .with_context(|| format!("unknown tool: {name}"))?;
        let instance = parse_arguments(arguments)?;
        let validator = jsonschema::validator_for(&tool.parameters)
            .with_context(|| format!("invalid registered schema for tool {name}"))?;
        validator
            .validate(&instance)
            .map_err(|error| anyhow::anyhow!("{error}"))
            .with_context(|| format!("arguments do not match schema for tool {name}"))
    }

    pub fn tool_infos(&self) -> Vec<ToolInfo> {
        let mut infos = self
            .tools
            .values()
            .map(|tool| ToolInfo {
                name: tool.name.clone(),
                description: tool.description.clone(),
                permission: tool.permission,
            })
            .collect::<Vec<_>>();
        infos.sort_by(|left, right| left.name.cmp(&right.name));
        infos
    }

    /// 克隆指定名称集合中的工具。
    ///
    /// 参数:
    /// - `allowed`: 允许复制到新注册表的工具名称
    ///
    /// 返回:
    /// - 仅包含允许工具的新注册表
    pub fn clone_filtered(&self, allowed: &[&str]) -> ToolRegistry {
        let wanted = allowed.iter().copied().collect::<BTreeSet<_>>();
        let mut registry = ToolRegistry::new();
        // 1. 按来源注册顺序复制，避免过滤后的工具数组因白名单顺序变化而重排
        for tool in self.ordered_tools() {
            if wanted.contains(tool.name.as_str()) {
                registry.register(tool.clone());
            }
        }
        registry
    }

    /// 克隆排除指定名称后的工具注册表。
    ///
    /// 参数:
    /// - `excluded`: 不复制到新注册表的工具名称
    ///
    /// 返回:
    /// - 保留原注册顺序和权限配置的新注册表
    pub(crate) fn clone_excluding(&self, excluded: &[&str]) -> ToolRegistry {
        let excluded = excluded.iter().copied().collect::<BTreeSet<_>>();
        let mut registry = ToolRegistry::new();
        registry.permission_profile = self.permission_profile.clone();
        // 1. 按来源注册顺序复制，确保供应商工具定义顺序稳定
        for tool in self.ordered_tools() {
            if !excluded.contains(tool.name.as_str()) {
                registry.register(tool.clone());
            }
        }
        registry
    }

    /// 从另一个注册表复制指定工具。
    ///
    /// 参数:
    /// - `source`: 来源工具注册表
    /// - `name`: 工具名称
    ///
    /// 返回:
    /// - 工具不存在时返回错误
    pub(crate) fn register_from(&mut self, source: &ToolRegistry, name: &str) -> Result<()> {
        let tool = source
            .tools
            .get(name)
            .with_context(|| format!("unknown tool: {name}"))?;
        self.register(tool.clone());
        Ok(())
    }

    pub fn permission(&self, name: &str) -> Result<ToolPermission> {
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        Ok(tool.permission)
    }

    /// 判断工具执行前是否需要交互式权限审计。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始 JSON 参数
    ///
    /// 返回:
    /// - 当前权限配置要求等待用户决定时返回 `true`
    pub(crate) fn requires_permission(&self, name: &str, arguments: &str) -> Result<bool> {
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let arguments = parse_arguments(arguments)?;
        Ok(self.permission_profile.as_ref().is_some_and(|profile| {
            profile.requires_interactive_audit(name, tool.permission, &arguments)
        }))
    }

    /// 记录工具权限请求已经展示给用户。
    pub(crate) fn record_permission_requested(&self, name: &str, arguments: &str) -> Result<()> {
        let arguments = parse_arguments(arguments)?;
        if let Some(profile) = &self.permission_profile {
            profile.record_requested(local_tool_name(name), &arguments);
        }
        Ok(())
    }

    /// 记录用户已经批准工具权限。
    ///
    /// 参数:
    /// - `name`: 工具名称
    /// - `arguments`: 原始工具参数
    ///
    /// 返回:
    /// - 参数解析和审计写入结果
    pub(crate) fn record_permission_approved(
        &self,
        name: &str,
        arguments: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        let arguments = parse_arguments(arguments)?;
        if let Some(profile) = &self.permission_profile {
            profile.record_approved(local_tool_name(name), &arguments, detail);
        }
        Ok(())
    }

    /// 记录用户拒绝工具权限及可选回复。
    pub(crate) fn record_permission_denied(
        &self,
        name: &str,
        arguments: &str,
        reply: Option<&str>,
    ) -> Result<()> {
        let arguments = parse_arguments(arguments)?;
        if let Some(profile) = &self.permission_profile {
            profile.record_denied(local_tool_name(name), &arguments, reply);
        }
        Ok(())
    }

    pub async fn call(&self, name: &str, arguments: &str) -> Result<String> {
        let requested_name = name;
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let mut args = parse_arguments(arguments)?;
        Ok(self
            .call_authorized(
                tool,
                name,
                &mut args,
                ToolProgress::default(),
                false,
                requested_name == DSH_BASH_EXECUTION_ALIAS,
            )
            .await?
            .content)
    }

    pub async fn call_with_progress(
        &self,
        name: &str,
        arguments: &str,
        sender: mpsc::UnboundedSender<String>,
    ) -> Result<ToolOutput> {
        let requested_name = name;
        let name = local_tool_name(name);
        let Some(tool) = self.tools.get(name) else {
            bail!("unknown tool: {name}");
        };
        let mut args = parse_arguments(arguments)?;
        self.call_authorized(
            tool,
            name,
            &mut args,
            ToolProgress::new(sender),
            true,
            requested_name == DSH_BASH_EXECUTION_ALIAS,
        )
        .await
    }

    /// 统一完成权限判定、沙盒标记注入和审计结果记录。
    ///
    /// 参数:
    /// - `tool`: 待执行工具定义
    /// - `name`: 本地工具名称
    /// - `args`: 已解析工具参数
    /// - `progress`: 工具进度通道
    /// - `accept_model_attachments`: 调用方是否会把临时附件提交给模型
    ///
    /// 返回:
    /// - 工具执行结果
    async fn call_authorized(
        &self,
        tool: &ToolSpec,
        name: &str,
        args: &mut Value,
        progress: ToolProgress,
        accept_model_attachments: bool,
        use_dsh_bash: bool,
    ) -> Result<ToolOutput> {
        if let Some(profile) = &self.permission_profile {
            // 网格工具按目标地址判定归属：投给别人的会话默认直接拒绝
            let scope = super::mesh::session_scope_for_call(
                name,
                args,
                &self.session_key,
                &self.session_id,
            );
            let sandboxed = profile.authorize_scoped(name, tool.permission, args, scope)?;
            if sandboxed {
                args.as_object_mut()
                    .context("tool arguments must be a JSON object")?
                    .insert("_sai_sandbox".to_string(), Value::Bool(true));
            }
        }
        if accept_model_attachments && name == "read_file" {
            args.as_object_mut()
                .context("tool arguments must be a JSON object")?
                .insert("_sai_model_attachments".to_string(), Value::Bool(true));
        }
        if use_dsh_bash {
            args.as_object_mut()
                .context("tool arguments must be a JSON object")?
                .insert(
                    "_sai_command_shell".to_string(),
                    Value::String(dsh_bash_shell()),
                );
        }
        let result = tool.call(args.clone(), progress).await;
        if let Some(profile) = &self.permission_profile {
            profile.record_result(
                name,
                args,
                result.as_ref().map(|output| output.content.as_str()),
            );
        }
        result
    }
}

/// 解析工具参数，空参数按空对象处理。
///
/// 解析规则统一在 [`crate::agent::first_json_object`]：严格解析优先，失败时
/// 退回取第一个完整的 JSON 对象。模型流式吐参数时偶尔会在有效 JSON 后面多带
/// 一段（另一个 JSON、说明文字、或截断后拼接的残片），严格解析会让整次工具
/// 调用失败并拖垮正在跑的子代理，而入参本身是完好的。
fn parse_arguments(arguments: &str) -> Result<Value> {
    let trimmed = arguments.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    crate::agent::first_json_object(trimmed).context("tool arguments are not valid JSON")
}

/// 返回 JSON 值的类型名称，用于参数校验错误说明。
///
/// 参数:
/// - `value`: 已解析的 JSON 值
///
/// 返回:
/// - 类型名称
fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// 将协议层工具别名还原为本地注册名称。
///
/// 只处理协议前缀差异；工具改名不在此列，模型拿到的就是当前名。
fn local_tool_name(name: &str) -> &str {
    match name {
        "sai_web_search" => "web_search",
        DSH_BASH_EXECUTION_ALIAS => "run_command",
        _ => name,
    }
}

fn dsh_bash_shell() -> String {
    #[cfg(windows)]
    {
        let git_bash = r"C:\Program Files\Git\bin\bash.exe";
        if std::path::Path::new(git_bash).is_file() {
            return git_bash.to_string();
        }
    }
    "bash".to_string()
}

pub fn empty_parameters() -> Value {
    json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false,
    })
}

#[cfg(test)]
#[path = "registry_schema_tests.rs"]
mod schema_tests;

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
