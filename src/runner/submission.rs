use crate::agent::AgentMode;
use crate::control_commands::ControlCommand;
use crate::render::{ReasoningDisplayMode, StreamRenderOptions, ToolCallDisplayMode};

/// runner 输入来源。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SubmissionSource {
    Command,
    Repl,
    Web,
    Gateway,
    ShellIntercept,
}

/// runner submission 的具体类型。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum RunnerSubmissionKind {
    UserInput(UserInputSubmission),
    Control(ControlSubmission),
}

/// 用户输入 submission。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct UserInputSubmission {
    pub(crate) input: String,
    pub(crate) image_urls: Vec<String>,
    pub(crate) turn_id: Option<String>,
    pub(crate) extra_system_prompt: Option<String>,
    pub(crate) mode: AgentMode,
    pub(crate) goal_continuation: bool,
    pub(crate) goal_event_prompt: Option<String>,
}

impl UserInputSubmission {
    /// 创建文本用户输入 submission。
    ///
    /// 参数:
    /// - `input`: 用户输入文本
    /// - `mode`: Agent 模式
    ///
    /// 返回:
    /// - 用户输入 submission
    pub(crate) fn new(input: impl Into<String>, mode: AgentMode) -> Self {
        Self {
            input: input.into(),
            image_urls: Vec::new(),
            turn_id: None,
            extra_system_prompt: None,
            mode,
            goal_continuation: false,
            goal_event_prompt: None,
        }
    }

    /// 设置当前轮图片 data URL。
    ///
    /// 参数:
    /// - `image_url`: 图片 data URL
    ///
    /// 返回:
    /// - 更新后的用户输入 submission
    pub(crate) fn with_image_url(mut self, image_url: impl Into<String>) -> Self {
        self.image_urls.push(image_url.into());
        self
    }

    /// 设置当前轮多张图片 data URL。
    ///
    /// 参数:
    /// - `image_urls`: 图片 data URL 列表
    ///
    /// 返回:
    /// - 更新后的用户输入 submission
    pub(crate) fn with_image_urls(mut self, image_urls: impl IntoIterator<Item = String>) -> Self {
        self.image_urls.extend(image_urls);
        self
    }

    /// 设置持久化轮次标识。
    ///
    /// 参数:
    /// - `turn_id`: 调用方生成的稳定轮次标识
    ///
    /// 返回:
    /// - 更新后的用户输入 submission
    pub(crate) fn with_turn_id(mut self, turn_id: impl Into<String>) -> Self {
        self.turn_id = Some(turn_id.into());
        self
    }

    /// 设置额外系统提示词。
    ///
    /// 参数:
    /// - `prompt`: 额外系统提示词
    ///
    /// 返回:
    /// - 更新后的用户输入 submission
    pub(crate) fn with_extra_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.extra_system_prompt = Some(prompt.into());
        self
    }

    /// 标记输入为内部 Goal 自动续轮。
    ///
    /// 返回:
    /// - 更新后的用户输入 submission
    pub(crate) fn with_goal_continuation(mut self) -> Self {
        self.goal_continuation = true;
        self
    }

    /// 附加外部完成事件，供 Goal 自动续轮消费。
    ///
    /// 参数:
    /// - `prompt`: 后台工作完成事件提示
    ///
    /// 返回:
    /// - 更新后的用户输入 submission
    pub(crate) fn with_goal_event_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.goal_event_prompt = Some(prompt.into());
        self
    }
}

/// 控制命令 submission。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ControlSubmission {
    pub(crate) command: ControlCommand,
}

impl ControlSubmission {
    /// 创建控制命令 submission。
    ///
    /// 参数:
    /// - `command`: 已解析的控制命令
    ///
    /// 返回:
    /// - 控制命令 submission
    pub(crate) fn new(command: ControlCommand) -> Self {
        Self { command }
    }
}

/// 渲染策略。
#[derive(Debug, Clone)]
pub(crate) struct RenderPolicy {
    pub(crate) plain: bool,
    pub(crate) reasoning_mode: ReasoningDisplayMode,
    pub(crate) tool_call_mode: ToolCallDisplayMode,
    pub(crate) stream_options: StreamRenderOptions,
}

impl RenderPolicy {
    /// 创建渲染策略。
    ///
    /// 参数:
    /// - `plain`: 是否使用纯文本输出
    /// - `reasoning_mode`: 推理内容显示方式
    /// - `tool_call_mode`: 工具调用显示方式
    /// - `stream_options`: 流式渲染选项
    ///
    /// 返回:
    /// - 渲染策略
    pub(crate) fn new(
        plain: bool,
        reasoning_mode: ReasoningDisplayMode,
        tool_call_mode: ToolCallDisplayMode,
        stream_options: StreamRenderOptions,
    ) -> Self {
        Self {
            plain,
            reasoning_mode,
            tool_call_mode,
            stream_options,
        }
    }
}

/// 渠道 submission 元数据。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ChannelSubmission {
    pub(crate) channel: String,
    pub(crate) inbound_marker: Option<String>,
    pub(crate) extra_loaded_tools: Vec<String>,
}

impl ChannelSubmission {
    /// 创建渠道 submission 元数据。
    ///
    /// 参数:
    /// - `channel`: 渠道名称
    ///
    /// 返回:
    /// - 渠道 submission 元数据
    pub(crate) fn new(channel: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            inbound_marker: None,
            extra_loaded_tools: Vec::new(),
        }
    }

    /// 设置渠道入站标记。
    ///
    /// 参数:
    /// - `marker`: 渠道入站标记
    ///
    /// 返回:
    /// - 更新后的渠道 submission 元数据
    pub(crate) fn with_inbound_marker(mut self, marker: impl Into<String>) -> Self {
        self.inbound_marker = Some(marker.into());
        self
    }

    /// 增加渠道必须预加载的工具。
    ///
    /// 参数:
    /// - `tool_name`: 工具名称
    ///
    /// 返回:
    /// - 更新后的渠道 submission 元数据
    pub(crate) fn with_extra_loaded_tool(mut self, tool_name: impl Into<String>) -> Self {
        self.extra_loaded_tools.push(tool_name.into());
        self
    }
}

/// runner 统一 submission。
#[derive(Debug, Clone)]
pub(crate) struct RunnerSubmission {
    pub(crate) session_id: Option<String>,
    pub(crate) source: SubmissionSource,
    pub(crate) mode: AgentMode,
    pub(crate) kind: RunnerSubmissionKind,
    pub(crate) show_final_summary: bool,
    pub(crate) render_policy: Option<RenderPolicy>,
    pub(crate) channel: Option<ChannelSubmission>,
}

impl RunnerSubmission {
    /// 创建用户输入 runner submission。
    ///
    /// 参数:
    /// - `source`: 输入来源
    /// - `input`: 用户输入 submission
    ///
    /// 返回:
    /// - runner submission
    pub(crate) fn user_input(source: SubmissionSource, input: UserInputSubmission) -> Self {
        Self {
            session_id: None,
            source,
            mode: input.mode,
            kind: RunnerSubmissionKind::UserInput(input),
            show_final_summary: false,
            render_policy: None,
            channel: None,
        }
    }

    /// 创建控制命令 runner submission。
    ///
    /// 参数:
    /// - `source`: 输入来源
    /// - `mode`: Agent 模式
    /// - `control`: 控制命令 submission
    ///
    /// 返回:
    /// - runner submission
    pub(crate) fn control(
        source: SubmissionSource,
        mode: AgentMode,
        control: ControlSubmission,
    ) -> Self {
        Self {
            session_id: None,
            source,
            mode,
            kind: RunnerSubmissionKind::Control(control),
            show_final_summary: false,
            render_policy: None,
            channel: None,
        }
    }

    /// 设置 session id。
    ///
    /// 参数:
    /// - `session_id`: 会话 ID
    ///
    /// 返回:
    /// - 更新后的 runner submission
    pub(crate) fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// 设置是否输出最终会话摘要。
    ///
    /// 参数:
    /// - `show_final_summary`: 是否输出最终会话摘要
    ///
    /// 返回:
    /// - 更新后的 runner submission
    pub(crate) fn with_final_summary(mut self, show_final_summary: bool) -> Self {
        self.show_final_summary = show_final_summary;
        self
    }

    /// 设置渲染策略。
    ///
    /// 参数:
    /// - `render_policy`: 渲染策略
    ///
    /// 返回:
    /// - 更新后的 runner submission
    pub(crate) fn with_render_policy(mut self, render_policy: RenderPolicy) -> Self {
        self.render_policy = Some(render_policy);
        self
    }

    /// 设置渠道元数据。
    ///
    /// 参数:
    /// - `channel`: 渠道元数据
    ///
    /// 返回:
    /// - 更新后的 runner submission
    pub(crate) fn with_channel(mut self, channel: ChannelSubmission) -> Self {
        self.channel = Some(channel);
        self
    }
}
