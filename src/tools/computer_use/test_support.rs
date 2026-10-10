//! 桌面测试只使用内存驱动和合成图片，不读取真实应用或用户配置。

use super::driver::NativeDriver;
use super::session::{ComputerSession, DesktopCoordinator};
use anyhow::{bail, Result};
use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

/// 记录调用并依次返回指定响应的驱动替身。
#[derive(Default)]
pub(super) struct FakeDriver {
    pub(super) calls: Mutex<Vec<(String, Value)>>,
    pub(super) replies: Mutex<VecDeque<Value>>,
    pub(super) closed: Mutex<usize>,
}

#[async_trait]
impl NativeDriver for FakeDriver {
    /// 【桌面测试】【原生替身】记录参数，返回预设结果或显式失败。
    /// @param name 为动作；args 为参数
    /// @returns 队列中的响应
    async fn call(&self, name: &str, args: Value) -> Result<Value> {
        self.calls.lock().unwrap().push((name.into(), args));
        let Some(reply) = self.replies.lock().unwrap().pop_front() else {
            bail!("fixture has no more responses");
        };
        if reply.get("transport_failure").is_some() {
            bail!("fixture disconnected after dispatch");
        }
        Ok(reply)
    }

    /// 【桌面测试】【关闭记录】记录会话释放，不操作真实服务。
    /// @returns 无
    async fn close(&self) {
        *self.closed.lock().unwrap() += 1;
    }
}

/// 【桌面测试】【图片夹具】生成明确尺寸的 PNG，供不缩放与边界检查使用。
/// @returns PNG 的 base64
pub(super) fn screenshot_base64() -> String {
    let image = image::DynamicImage::new_rgb8(100, 60);
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    base64::engine::general_purpose::STANDARD.encode(bytes.into_inner())
}

/// 【桌面测试】【状态夹具】构造带原生 token 的窗口状态和可选图片。
/// @param screenshot 为是否携带合成截图
/// @returns 标准 MCP result
pub(super) fn state(screenshot: bool) -> Value {
    let mut content = vec![json!({"type":"text","text":"fixture state"})];
    if screenshot {
        content.push(json!({"type":"image","mimeType":"image/png","data":screenshot_base64()}));
    }
    json!({
        "structuredContent":{
            "pid":42,"window_id":101,"app_name":"Fixture","window_title":"Test window",
            "capture_id":"capture-fixture","screenshot_width":100,"screenshot_height":60,
            "elements":[
                {"element_index":10,"element_token":"native-button-token","role":"button","label":"Apply","enabled":true},
                {"element_index":20,"element_token":"native-field-token","role":"text","label":"Query","value":""}
            ]
        },
        "content":content
    })
}

/// 【桌面测试】【会话夹具】创建拥有独立协调器的会话，测试之间互不影响。
/// @param replies 为按调用顺序返回的响应
/// @returns 会话与可检查的驱动
pub(super) fn session(replies: Vec<Value>) -> (ComputerSession, Arc<FakeDriver>) {
    let driver = Arc::new(FakeDriver::default());
    driver.replies.lock().unwrap().extend(replies);
    (
        ComputerSession::new(driver.clone(), Arc::new(DesktopCoordinator::default())),
        driver,
    )
}

/// 【桌面测试】【观察参数】返回确定的测试进程与窗口。
/// @returns get_app_state 参数
pub(super) fn observe_args() -> Value {
    json!({"action":"get_app_state","app":"42","window_id":101})
}

/// 【桌面测试】【输入参数】从实际工具输出读取快照标识，避免绕过公开契约。
/// @param output 为 get_app_state 文本；action 为输入动作
/// @returns 带有效目标与快照的输入参数
pub(super) fn input_args(output: &str, action: &str) -> Value {
    let id = output
        .lines()
        .find_map(|line| line.strip_prefix("snapshot_id: "))
        .unwrap();
    json!({"action":action,"app":"42","window_id":101,"snapshot_id":id})
}
