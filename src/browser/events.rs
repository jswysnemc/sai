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

/// 一次页面下载。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct DownloadInfo {
    pub(crate) guid: String,
    /// 页面建议的文件名（已清理）
    pub(crate) file_name: String,
    pub(crate) url: String,
    /// `inProgress`、`completed` 或 `canceled`
    pub(crate) state: String,
    pub(crate) received: u64,
    pub(crate) total: u64,
}

/// 页面弹出的 alert、confirm、prompt 或 beforeunload 对话框。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct DialogInfo {
    /// `alert`、`confirm`、`prompt` 或 `beforeunload`
    pub(crate) kind: String,
    pub(crate) message: String,
    /// prompt 的默认值
    pub(crate) default_prompt: String,
    /// 弹出对话框的页面地址
    pub(crate) url: String,
}

/// 原生下拉框被点开时交给面板绘制的选项菜单。
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub(crate) struct SelectPopup {
    pub(crate) options: Vec<SelectOption>,
    pub(crate) selected: i64,
    /// 下拉框在页面中的位置（CSS 像素）
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

/// 下拉框中的一个选项。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
pub(crate) struct SelectOption {
    pub(crate) label: String,
    pub(crate) disabled: bool,
    /// 所属分组标题，没有分组时为空
    #[serde(default)]
    pub(crate) group: String,
}

/// 页面请求选择文件时交给面板的说明。
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub(crate) struct FileChooserInfo {
    pub(crate) multiple: bool,
    /// input 的 accept 属性
    pub(crate) accept: String,
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
    /// 页面弹出对话框；None 表示对话框已关闭
    Dialog(Option<DialogInfo>),
    /// 原生下拉框需要面板绘制选项
    SelectPopup(SelectPopup),
    /// 页面请求选择文件；None 表示请求已结束
    FileChooser(Option<FileChooserInfo>),
    /// 下载开始或结束
    Download(DownloadInfo),
    /// 页面内复制或剪切的文本，交给面板写入本机剪贴板
    Clipboard(String),
}

/// 【内置浏览器】【短标识】把目标 ID 截成面板与模型使用的短标识。
/// @param target_id 为完整目标 ID
/// @returns 前 8 个字符，不足时原样返回
pub(crate) fn short_tab_id(target_id: &str) -> String {
    target_id.chars().take(8).collect::<String>().to_lowercase()
}
