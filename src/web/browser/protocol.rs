//! 浏览器面板 WebSocket 消息协议。
//!
//! 文本消息为 JSON 控制与状态，二进制消息为一帧 JPEG 画面。

use crate::browser::{BrowserState, KeyInput, MouseInput};
use serde::{Deserialize, Serialize};

/// 面板发给服务端的控制消息。
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ClientMessage {
    /// 地址栏提交；非地址文本按搜索处理
    Navigate {
        url: String,
    },
    Back,
    Forward,
    Reload,
    Stop,
    /// 鼠标事件，坐标为页面 CSS 像素
    Mouse(MouseInput),
    /// 键盘事件
    Key(KeyInput),
    /// 粘贴或输入法提交的整段文本
    InsertText {
        text: String,
    },
    /// 面板视口尺寸变化
    Resize {
        width: u32,
        height: u32,
    },
    NewTab,
    SwitchTab {
        id: String,
    },
    CloseTab {
        id: String,
    },
}

/// 服务端发给面板的状态消息。
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ServerMessage<'a> {
    /// 地址、标题、标签与视口状态
    State { state: &'a BrowserState },
    /// Agent 的一步操作
    Activity { message: &'a str },
    /// 面板操作失败的原因
    Error { message: &'a str },
}

impl ClientMessage {
    /// 【浏览器面板】【耗时判断】导航类操作需要等待页面加载，放到独立任务执行。
    /// @returns 需要异步执行时为 true
    pub(super) fn is_slow(&self) -> bool {
        matches!(
            self,
            Self::Navigate { .. }
                | Self::Back
                | Self::Forward
                | Self::Reload
                | Self::NewTab
                | Self::SwitchTab { .. }
                | Self::CloseTab { .. }
        )
    }
}
