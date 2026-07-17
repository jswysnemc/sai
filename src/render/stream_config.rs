#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningDisplayMode {
    Hidden,
    Summary,
    Full,
}

impl ReasoningDisplayMode {
    /// 从配置文本解析推理展示模式。
    ///
    /// 参数:
    /// - `value`: 配置值
    ///
    /// 返回:
    /// - 推理展示模式
    pub fn from_config(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "hidden" => Self::Hidden,
            "full" => Self::Full,
            _ => Self::Summary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCallDisplayMode {
    Hidden,
    Summary,
    Full,
}

impl ToolCallDisplayMode {
    /// 从配置文本解析工具调用展示模式。
    ///
    /// 参数:
    /// - `value`: 配置值
    ///
    /// 返回:
    /// - 工具调用展示模式
    pub fn from_config(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "hidden" => Self::Hidden,
            "full" => Self::Full,
            _ => Self::Summary,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StreamRenderOptions {
    pub readable_tool_names: bool,
    pub wait_model: Option<String>,
    pub wait_thinking_level: Option<String>,
}
