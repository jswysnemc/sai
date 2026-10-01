//! 推送给 Web 工作台浏览器面板的状态与画面事件。

use serde::Serialize;
use std::sync::Arc;

/// 一个标签页的展示信息。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct TabInfo {
    /// 短标识，取目标 ID 前 8 位，供模型与面板引用
    pub(crate) id: String,
    /// 页面标题
    pub(crate) title: String,
    /// 当前地址
    pub(crate) url: String,
    /// 是否为当前操作的标签页
    pub(crate) active: bool,
}

/// 面板顶部地址栏与标签条所需的完整状态。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct BrowserState {
    /// 全部页面类标签
    pub(crate) tabs: Vec<TabInfo>,
    /// 当前标签地址
    pub(crate) url: String,
    /// 当前标签标题
    pub(crate) title: String,
    /// 页面是否仍在加载
    pub(crate) loading: bool,
    /// 历史记录可后退
    pub(crate) can_go_back: bool,
    /// 历史记录可前进
    pub(crate) can_go_forward: bool,
    /// 视口宽度（CSS 像素）
    pub(crate) width: u32,
    /// 视口高度（CSS 像素）
    pub(crate) height: u32,
}

/// 浏览器会话广播的事件。
#[derive(Clone, Debug)]
pub(crate) enum BrowserEvent {
    /// 地址、标题、标签或加载状态变化
    State(BrowserState),
    /// 一帧 JPEG 画面
    Frame(Arc<Vec<u8>>),
    /// Agent 工具执行的一步操作，面板据此提示“正在由 Agent 操作”
    Activity(String),
}

/// 【内置浏览器】【短标识】把目标 ID 截成面板与模型使用的短标识。
/// @param target_id 为完整目标 ID
/// @returns 前 8 个字符，不足时原样返回
pub(crate) fn short_tab_id(target_id: &str) -> String {
    target_id.chars().take(8).collect::<String>().to_lowercase()
}
