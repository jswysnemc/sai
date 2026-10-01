//! 浏览器面板 WebSocket 消息协议。
//!
//! 文本消息为 JSON 控制与状态，二进制消息为一帧 JPEG 画面。

use crate::browser::{
    BrowserState, DialogInfo, DownloadInfo, FileChooserInfo, KeyInput, MouseInput, SelectPopup,
};
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
    /// 面板视口尺寸与设备像素比变化
    Resize {
        width: u32,
        height: u32,
        /// 面板所在屏幕的 devicePixelRatio，旧面板不发送时按 1 处理
        #[serde(default = "default_scale")]
        scale: f64,
    },
    NewTab,
    SwitchTab {
        id: String,
    },
    CloseTab {
        id: String,
    },
    /// 回复页面对话框；prompt 时附带输入文本
    DialogReply {
        accept: bool,
        #[serde(default)]
        prompt_text: Option<String>,
    },
    /// 在面板绘制的下拉菜单中选中一项
    SelectReply {
        index: i64,
    },
    /// 回复文件选择；空列表表示取消
    FileChooserReply {
        #[serde(default)]
        uploads: Vec<String>,
    },
    /// 开始元素选择，labels 为信息卡的背景、颜色、字体标签
    PickStart {
        #[serde(default)]
        labels: Option<[String; 3]>,
    },
    PickCancel,
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
    /// 连接信息：调试工具地址与用户目录类型
    Info {
        devtools_url: Option<String>,
        persistent_profile: bool,
    },
    /// 页面对话框；dialog 为空表示已关闭
    Dialog { dialog: Option<&'a DialogInfo> },
    /// 面板需要绘制的原生下拉菜单
    SelectPopup { popup: &'a SelectPopup },
    /// 页面请求选择文件；chooser 为空表示请求已结束
    FileChooser {
        chooser: Option<&'a FileChooserInfo>,
    },
    /// 下载开始或结束
    Download { download: &'a DownloadInfo },
    /// 页面内复制的文本
    Clipboard { text: &'a str },
    /// 元素选择结果；element 为空表示已取消
    Picked {
        element: Option<&'a serde_json::Value>,
    },
}

/// 【浏览器面板】【像素比默认值】旧版面板不上报像素比时按 1 处理。
/// @returns 1.0
fn default_scale() -> f64 {
    1.0
}

impl ClientMessage {
    /// 【浏览器面板】【插队判断】回复对话框与取消元素选择要立即执行。
    ///
    /// 页面弹出对话框时，触发它的点击或按键在浏览器里一直未完成，输入队列被它占住；
    /// 回复排在队列后面会互相等待。取消元素选择同理，选择本身占着一个长时间求值。
    ///
    /// @returns 需要绕过输入队列时为 true
    pub(super) fn bypasses_queue(&self) -> bool {
        matches!(self, Self::DialogReply { .. } | Self::PickCancel)
    }

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
                | Self::PickStart { .. }
                | Self::FileChooserReply { .. }
        )
    }
}
