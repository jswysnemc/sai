use super::subagent_feed::STATS_PREFIX;
use super::{readable_tool_name, ToolProgress};
use crate::i18n::is_zh;
use serde_json::{json, Value};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ProgressMode {
    Hidden,
    Summary,
    Full,
}

#[derive(Clone)]
pub(crate) struct SubagentProgress {
    progress: ToolProgress,
    mode: ProgressMode,
    enabled: bool,
}

impl SubagentProgress {
    /// 创建子代理进度回调封装。
    ///
    /// 参数:
    /// - `progress`: 宿主工具进度发送器
    /// - `mode`: 展示模式
    /// - `enabled`: 是否展示进度
    ///
    /// 返回:
    /// - 子代理进度对象
    pub(crate) fn new(progress: ToolProgress, mode: ProgressMode, enabled: bool) -> Self {
        Self {
            progress,
            mode,
            enabled,
        }
    }

    /// 上报阶段进度信息。
    ///
    /// 参数:
    /// - `message`: 阶段文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn phase(&self, message: impl Into<String>) {
        if self.enabled && self.mode != ProgressMode::Hidden {
            self.progress.report(message.into());
        }
    }

    /// 上报累计用量快照。
    ///
    /// 与阶段文本不同，用量不受展示模式限制：进度被隐藏时底部面板
    /// 仍要能读到实时 token，否则长任务期间数字一直是空的。
    ///
    /// 参数:
    /// - `stats`: `SubagentStats::public()` 生成的用量 JSON
    ///
    /// 返回:
    /// - 无
    pub(crate) fn stats(&self, stats: Value) {
        self.progress.report(format!("{STATS_PREFIX}{stats}"));
    }

    /// 上报子代理推理文本。
    ///
    /// 参数:
    /// - `text`: 推理文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn reasoning(&self, text: &str) {
        if self.enabled && self.mode != ProgressMode::Hidden {
            self.progress
                .report(format!("__subagent_reasoning__{}", text));
        }
    }

    /// 上报子智能体正文流分片。
    ///
    /// 参数:
    /// - `text`: 模型实时返回的正文分片
    ///
    /// 返回:
    /// - 无
    pub(crate) fn content(&self, text: &str) {
        if self.enabled && self.mode == ProgressMode::Full && !text.is_empty() {
            self.progress.report(format!("__subagent_text__{}", text));
        }
    }

    /// 上报子工具开始运行。
    ///
    /// 参数:
    /// - `step`: 当前工具调用序号
    /// - `name`: 子工具名称
    ///
    /// 返回:
    /// - 无
    pub(crate) fn tool_start(&self, step: usize, name: &str) {
        if !self.enabled || self.mode == ProgressMode::Hidden {
            return;
        }
        if self.mode == ProgressMode::Summary {
            self.progress.report(if is_zh() {
                format!("工具 #{step}：{} 运行中", readable_tool_name(name))
            } else {
                format!("tool #{step}: {name} running")
            });
        }
    }

    /// 上报子工具调用参数。
    ///
    /// 参数:
    /// - `name`: 子工具名称
    /// - `args`: 子工具参数 JSON
    ///
    /// 返回:
    /// - 无
    pub(crate) fn tool_call_detail(&self, name: &str, args: &str) {
        if self.enabled && self.mode == ProgressMode::Full {
            self.progress.report(format!(
                "__subtool_call__{}",
                json!({ "name": name, "args": args })
            ));
        }
    }

    /// 上报子工具完成状态。
    ///
    /// 参数:
    /// - `step`: 当前工具调用序号
    /// - `name`: 子工具名称
    /// - `ok`: 是否成功
    /// - `output`: 子工具输出
    ///
    /// 返回:
    /// - 无
    pub(crate) fn tool_end(&self, step: usize, name: &str, ok: bool, output: &str) {
        if !self.enabled || self.mode == ProgressMode::Hidden {
            return;
        }
        if self.mode == ProgressMode::Summary {
            self.progress.report(if is_zh() {
                format!("工具 #{step}：{} ok", readable_tool_name(name))
            } else {
                format!("tool #{step}: {name} ok")
            });
        }
        if self.mode == ProgressMode::Full {
            self.progress.report(format!(
                "__subtool_result__{}",
                json!({ "name": name, "ok": ok, "output": output })
            ));
        }
    }
}
