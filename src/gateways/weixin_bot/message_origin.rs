use crate::paths::SaiPaths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 【微信消息】【来源信息】宿主绑定的会话与子任务身份；不向微信泄露本地状态目录。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct MessageOrigin {
    pub(crate) session_id: String,
    pub(crate) title: String,
    pub(crate) subagent_id: Option<String>,
}

impl MessageOrigin {
    /// 【微信消息】【来源解析】参数 paths/args 为路径和可信参数，返回实际来源或无绑定状态。
    pub(crate) fn from_args(paths: &SaiPaths, args: &Value) -> Result<Option<Self>> {
        let origin = &args["_sai_channel_origin"];
        let Some(session_id) = origin["session_id"].as_str().filter(|id| !id.is_empty()) else {
            return Ok(None);
        };
        let state_dir = origin["state_dir"].as_str().unwrap_or_default();
        let title = crate::state::list_all_sessions(paths)?
            .into_iter()
            .find(|session| {
                session.info.id == session_id
                    && (state_dir.is_empty()
                        || session.state_dir == std::path::Path::new(state_dir))
            })
            .map(|session| session.info.title)
            .unwrap_or_else(|| "未命名会话".into());
        Ok(Some(Self {
            session_id: session_id.into(),
            title,
            subagent_id: origin["subagent_id"].as_str().map(str::to_string),
        }))
    }

    /// 【微信消息】【来源标记】参数 text 为原消息或媒体说明，返回可定位及回复的微信文本。
    pub(crate) fn decorate(&self, text: Option<String>) -> String {
        let title: String = self
            .title
            .chars()
            .filter(|ch| !ch.is_control())
            .take(48)
            .collect();
        let mut header = format!("[Sai · {title}]\n会话：{}", self.session_id);
        if let Some(id) = &self.subagent_id {
            header.push_str(&format!("\n子任务：{id}"));
        }
        header.push_str(&format!("\n回复此会话：回复 {}：内容", self.session_id));
        match text {
            Some(text) => format!("{header}\n\n{text}"),
            None => header,
        }
    }
}

#[cfg(test)]
#[path = "message_origin_tests.rs"]
mod tests;
